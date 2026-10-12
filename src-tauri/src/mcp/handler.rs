//! Brio 的 MCP 工具实现（应用级单 server，按 `browser_id` 寻址多浏览器）。
//!
//! 工具分两层：
//! - 管理层：list/get/create/update/delete/open/close browser + detect_timezone，
//!   直接复用 [`crate::browsers_core`] 与 [`crate::commands::sidecar_ops`]，与前端
//!   IPC 共享同一份逻辑。
//! - 页面操作层：navigate/snapshot/click/type/... 经 [`super::cdp`] 连到目标浏览器
//!   的本机 CDP 端口执行。
//!
//! 错误约定：业务失败返回 `Ok(CallToolResult::error(..))`（消息对 agent 可见），
//! 只有请求根本无法处理时才用 `Err(McpError)`。

use std::sync::Arc;
use std::time::{Duration, Instant};

use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{CallToolResult, ContentBlock};
use rmcp::schemars::{self, JsonSchema};
use rmcp::{tool, tool_handler, tool_router, ErrorData as McpError, ServerHandler};
use serde::Deserialize;
use serde_json::{json, Value};
use tauri::{AppHandle, Manager};

use super::cdp::{self, CdpConn, CdpPool};
use super::snapshot::format_ax_tree;
use crate::browser_supervisor::BrowserSupervisor;
use crate::browsers_core::{self as core, Browser, CreateBrowserInput, UpdateBrowserInput};
use crate::commands::sidecar_ops::{derive_fingerprint_id_inner, detect_timezone_inner};
use crate::db::DbState;

#[derive(Clone)]
pub struct BrioMcp {
    app: AppHandle,
    pool: Arc<CdpPool>,
}

impl BrioMcp {
    pub fn new(app: AppHandle, pool: Arc<CdpPool>) -> Self {
        Self { app, pool }
    }
}

// ---------------------------------------------------------------------------
// 工具参数结构
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize, JsonSchema)]
pub struct BrowserIdArgs {
    /// 目标浏览器的 id（来自 list_browsers）。
    pub browser_id: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct CreateBrowserArgs {
    /// 浏览器展示名。
    pub name: String,
    /// 上游代理，如 `socks5://user:pass@host:port` 或 `http://host:port`；无代理留空。
    #[serde(default)]
    pub proxy_url: Option<String>,
    /// IANA 时区，如 `Asia/Shanghai`；留空且配了代理时会自动探测。
    #[serde(default)]
    pub timezone: Option<String>,
    /// 界面语言，默认 `en-US`。
    #[serde(default)]
    pub language: Option<String>,
    /// 备注。
    #[serde(default)]
    pub notes: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct UpdateBrowserArgs {
    pub browser_id: String,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub proxy_url: Option<String>,
    #[serde(default)]
    pub timezone: Option<String>,
    #[serde(default)]
    pub language: Option<String>,
    #[serde(default)]
    pub notes: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct OpenBrowserArgs {
    pub browser_id: String,
    /// 启动后要打开的 URL 列表（可选）。
    #[serde(default)]
    pub open_urls: Option<Vec<String>>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct DetectTimezoneArgs {
    /// 用于探测出口时区的代理；留空则探测本机直连出口。
    #[serde(default)]
    pub proxy_url: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct TabArgs {
    pub browser_id: String,
    /// 目标标签页 id（来自 list_tabs）；留空时用第一个页面标签。
    #[serde(default)]
    pub tab_id: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct CloseTabArgs {
    pub browser_id: String,
    /// 要关闭的标签页 id。
    pub tab_id: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct NewTabArgs {
    pub browser_id: String,
    /// 新标签页初始 URL，默认 `about:blank`。
    #[serde(default)]
    pub url: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct NavigateArgs {
    pub browser_id: String,
    /// 要导航到的 URL。
    pub url: String,
    #[serde(default)]
    pub tab_id: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ClickArgs {
    pub browser_id: String,
    /// 元素 ref（来自 snapshot，如 `e5`）。
    pub r#ref: String,
    #[serde(default)]
    pub tab_id: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct TypeTextArgs {
    pub browser_id: String,
    /// 目标元素 ref（来自 snapshot）。
    pub r#ref: String,
    /// 要输入的文本。
    pub text: String,
    /// 输入后是否按回车提交。
    #[serde(default)]
    pub submit: Option<bool>,
    #[serde(default)]
    pub tab_id: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct PressKeyArgs {
    pub browser_id: String,
    /// 键名，如 `Enter` / `Tab` / `Escape` / `ArrowDown` / `a`。
    pub key: String,
    #[serde(default)]
    pub tab_id: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ScrollArgs {
    pub browser_id: String,
    /// 方向：`down`（默认）或 `up`。
    #[serde(default)]
    pub direction: Option<String>,
    /// 滚动像素，默认 600。
    #[serde(default)]
    pub amount: Option<f64>,
    #[serde(default)]
    pub tab_id: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct WaitForArgs {
    pub browser_id: String,
    /// 等待页面出现该文本。
    #[serde(default)]
    pub text: Option<String>,
    /// 等待页面上该文本消失。
    #[serde(default)]
    pub text_gone: Option<String>,
    /// 固定等待秒数（给定时忽略 text/text_gone）。
    #[serde(default)]
    pub time: Option<f64>,
    #[serde(default)]
    pub tab_id: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ScreenshotArgs {
    pub browser_id: String,
    /// 是否整页截图（默认仅可视区域）。
    #[serde(default)]
    pub full_page: Option<bool>,
    #[serde(default)]
    pub tab_id: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct EvaluateArgs {
    pub browser_id: String,
    /// 要在页面上下文执行的 JavaScript 表达式（支持返回 Promise）。
    pub expression: String,
    #[serde(default)]
    pub tab_id: Option<String>,
}

// ---------------------------------------------------------------------------
// 工具实现
// ---------------------------------------------------------------------------

#[tool_router]
impl BrioMcp {
    // ---- 管理层 ----

    #[tool(description = "列出所有指纹浏览器 profile（含运行状态与 CDP endpoint）。")]
    async fn list_browsers(&self) -> Result<CallToolResult, McpError> {
        let db = self.app.state::<DbState>();
        let list = match core::list_browsers(&db) {
            Ok(v) => v,
            Err(e) => return Ok(tool_err(e)),
        };
        let supervisor = self.app.state::<BrowserSupervisor>();
        let arr: Vec<Value> = list
            .iter()
            .map(|b| {
                let mut v = masked(b);
                if let Some(r) = supervisor.get_running(&b.id) {
                    v["running"] = json!(true);
                    v["status"] = serde_json::to_value(r.status).unwrap_or(Value::Null);
                    v["cdp_endpoint"] = json!(r.cdp_endpoint);
                } else {
                    v["running"] = json!(false);
                }
                v
            })
            .collect();
        Ok(json_ok(json!(arr)))
    }

    #[tool(description = "按 id 获取单个浏览器 profile 的详情。")]
    async fn get_browser(
        &self,
        Parameters(a): Parameters<BrowserIdArgs>,
    ) -> Result<CallToolResult, McpError> {
        let db = self.app.state::<DbState>();
        Ok(match core::get_browser(&db, &a.browser_id) {
            Ok(b) => json_ok(masked(&b)),
            Err(e) => tool_err(e),
        })
    }

    #[tool(
        description = "新建一个指纹浏览器 profile。fingerprint_id 由 (proxy, timezone, language) 自动派生；timezone 留空且配置了代理时会自动探测。"
    )]
    async fn create_browser(
        &self,
        Parameters(a): Parameters<CreateBrowserArgs>,
    ) -> Result<CallToolResult, McpError> {
        let proxy = a.proxy_url.unwrap_or_default();
        let language = a
            .language
            .filter(|s| !s.trim().is_empty())
            .unwrap_or_else(|| "en-US".to_string());

        // timezone：给定优先；否则若有代理则探测。
        let mut timezone = a.timezone.unwrap_or_default();
        if timezone.trim().is_empty() && !proxy.trim().is_empty() {
            if let Ok(res) = detect_timezone_inner(&self.app, Some(proxy.clone())).await {
                timezone = res.timezone;
            }
        }

        let fingerprint_id = match derive_fingerprint_id_inner(
            &self.app,
            Some(proxy.clone()),
            Some(timezone.clone()),
            Some(language.clone()),
        )
        .await
        {
            Ok(fp) => fp,
            Err(e) => return Ok(tool_err(format!("derive fingerprint failed: {e}"))),
        };

        let input = CreateBrowserInput {
            name: a.name,
            proxy_url: proxy,
            timezone,
            language,
            fingerprint_id,
            notes: a.notes.unwrap_or_default(),
        };
        let db = self.app.state::<DbState>();
        Ok(match core::create_browser(&self.app, &db, input) {
            Ok(b) => json_ok(masked(&b)),
            Err(e) => tool_err(e),
        })
    }

    #[tool(description = "更新浏览器 profile 的字段（不可改 fingerprint_id，需换指纹请新建）。")]
    async fn update_browser(
        &self,
        Parameters(a): Parameters<UpdateBrowserArgs>,
    ) -> Result<CallToolResult, McpError> {
        let input = UpdateBrowserInput {
            name: a.name,
            proxy_url: a.proxy_url,
            timezone: a.timezone,
            language: a.language,
            notes: a.notes,
        };
        let db = self.app.state::<DbState>();
        Ok(match core::update_browser(&db, &a.browser_id, input) {
            Ok(b) => json_ok(masked(&b)),
            Err(e) => tool_err(e),
        })
    }

    #[tool(description = "删除浏览器 profile（必须先关闭）。")]
    async fn delete_browser(
        &self,
        Parameters(a): Parameters<BrowserIdArgs>,
    ) -> Result<CallToolResult, McpError> {
        let db = self.app.state::<DbState>();
        let supervisor = self.app.state::<BrowserSupervisor>();
        Ok(match core::delete_browser(&db, &supervisor, &a.browser_id) {
            Ok(()) => text_ok(format!("deleted browser {}", a.browser_id)),
            Err(e) => tool_err(e),
        })
    }

    #[tool(
        description = "启动一个浏览器 profile 并等待其 CDP 就绪（最多 60s）。返回运行状态与 CDP endpoint。"
    )]
    async fn open_browser(
        &self,
        Parameters(a): Parameters<OpenBrowserArgs>,
    ) -> Result<CallToolResult, McpError> {
        let open = a.open_urls.map(|urls| core::OpenBrowserInput { open_urls: urls });
        {
            let db = self.app.state::<DbState>();
            let supervisor = self.app.state::<BrowserSupervisor>();
            if let Err(e) = core::open_browser(&self.app, &db, &supervisor, &a.browser_id, open) {
                return Ok(tool_err(e));
            }
        }

        // 轮询等待 cdp 就绪或出错。
        for _ in 0..120 {
            tokio::time::sleep(Duration::from_millis(500)).await;
            let running = self.app.state::<BrowserSupervisor>().get_running(&a.browser_id);
            match running {
                None => {
                    return Ok(tool_err(
                        "browser exited before becoming ready (check proxy / fingerprint)"
                            .to_string(),
                    ));
                }
                Some(r) => {
                    if r.cdp_port != 0 && cdp::probe_ready(r.cdp_port).await {
                        return Ok(json_ok(json!({
                            "browser_id": r.browser_id,
                            "status": r.status,
                            "cdp_port": r.cdp_port,
                            "cdp_endpoint": r.cdp_endpoint,
                        })));
                    }
                    if matches!(r.status, crate::browser_supervisor::BrowserStatus::Error) {
                        return Ok(tool_err("browser reported an error while starting".to_string()));
                    }
                }
            }
        }
        Ok(tool_err("timed out waiting for CDP to become ready".to_string()))
    }

    #[tool(description = "关闭一个正在运行的浏览器 profile。")]
    async fn close_browser(
        &self,
        Parameters(a): Parameters<BrowserIdArgs>,
    ) -> Result<CallToolResult, McpError> {
        let res = {
            let supervisor = self.app.state::<BrowserSupervisor>();
            core::close_browser(&supervisor, &a.browser_id).await
        };
        self.pool.drop_conn(&a.browser_id);
        Ok(match res {
            Ok(()) => text_ok(format!("closed browser {}", a.browser_id)),
            Err(e) => tool_err(e),
        })
    }

    #[tool(description = "通过给定代理探测出口时区（用于新建 profile 前确定 timezone）。")]
    async fn detect_timezone(
        &self,
        Parameters(a): Parameters<DetectTimezoneArgs>,
    ) -> Result<CallToolResult, McpError> {
        Ok(match detect_timezone_inner(&self.app, a.proxy_url).await {
            Ok(r) => json_ok(json!({ "timezone": r.timezone, "detected": r.detected })),
            Err(e) => tool_err(e),
        })
    }

    // ---- 页面操作层 ----

    #[tool(description = "列出某浏览器当前打开的所有标签页（id / title / url）。")]
    async fn list_tabs(
        &self,
        Parameters(a): Parameters<BrowserIdArgs>,
    ) -> Result<CallToolResult, McpError> {
        let port = match self.running_port(&a.browser_id) {
            Ok(p) => p,
            Err(e) => return Ok(tool_err(e)),
        };
        Ok(match cdp::list_page_targets(port).await {
            Ok(tabs) => {
                let arr: Vec<Value> = tabs
                    .into_iter()
                    .map(|(id, title, url)| json!({ "tab_id": id, "title": title, "url": url }))
                    .collect();
                json_ok(json!(arr))
            }
            Err(e) => tool_err(e),
        })
    }

    #[tool(description = "在某浏览器新开一个标签页，返回其 tab_id。")]
    async fn new_tab(
        &self,
        Parameters(a): Parameters<NewTabArgs>,
    ) -> Result<CallToolResult, McpError> {
        let conn = match self.conn_for(&a.browser_id).await {
            Ok(c) => c,
            Err(e) => return Ok(tool_err(e)),
        };
        let url = a.url.unwrap_or_else(|| "about:blank".to_string());
        Ok(
            match conn
                .call("Target.createTarget", json!({ "url": url }), None)
                .await
            {
                Ok(v) => {
                    let id = v.get("targetId").and_then(|x| x.as_str()).unwrap_or("");
                    json_ok(json!({ "tab_id": id }))
                }
                Err(e) => tool_err(e),
            },
        )
    }

    #[tool(description = "关闭某浏览器的指定标签页。")]
    async fn close_tab(
        &self,
        Parameters(a): Parameters<CloseTabArgs>,
    ) -> Result<CallToolResult, McpError> {
        let conn = match self.conn_for(&a.browser_id).await {
            Ok(c) => c,
            Err(e) => return Ok(tool_err(e)),
        };
        Ok(
            match conn
                .call("Target.closeTarget", json!({ "targetId": a.tab_id }), None)
                .await
            {
                Ok(_) => text_ok(format!("closed tab {}", a.tab_id)),
                Err(e) => tool_err(e),
            },
        )
    }

    #[tool(description = "在指定标签页导航到某 URL，并等待页面加载完成。")]
    async fn navigate(
        &self,
        Parameters(a): Parameters<NavigateArgs>,
    ) -> Result<CallToolResult, McpError> {
        let (conn, session, _target) =
            match self.session_for(&a.browser_id, a.tab_id.clone()).await {
                Ok(v) => v,
                Err(e) => return Ok(tool_err(e)),
            };
        if let Err(e) = conn
            .call("Page.navigate", json!({ "url": a.url }), Some(&session))
            .await
        {
            return Ok(tool_err(e));
        }
        // 最多等 8s：complete / interactive 即可返回。页面若一直 loading
        // （代理慢、验证页）也不阻塞工具。
        let deadline = Instant::now() + Duration::from_secs(8);
        let mut ready = String::new();
        while Instant::now() < deadline {
            tokio::time::sleep(Duration::from_millis(200)).await;
            if let Ok(v) = conn
                .call_poll(
                    "Runtime.evaluate",
                    json!({ "expression": "document.readyState", "returnByValue": true }),
                    Some(&session),
                )
                .await
            {
                if let Some(s) = js_string(&v) {
                    ready = s;
                    if ready == "complete" || ready == "interactive" {
                        break;
                    }
                }
            }
        }
        let title = conn
            .call_poll(
                "Runtime.evaluate",
                json!({ "expression": "document.title", "returnByValue": true }),
                Some(&session),
            )
            .await
            .ok()
            .and_then(|v| js_string(&v))
            .unwrap_or_default();
        Ok(json_ok(json!({
            "ok": true,
            "url": a.url,
            "title": title,
            "readyState": ready,
        })))
    }

    #[tool(
        description = "抓取当前页面的可访问性快照（带 ref 的元素树），供后续 click / type 用 ref 定位。"
    )]
    async fn snapshot(
        &self,
        Parameters(a): Parameters<TabArgs>,
    ) -> Result<CallToolResult, McpError> {
        let (conn, session, target) = match self.session_for(&a.browser_id, a.tab_id).await {
            Ok(v) => v,
            Err(e) => return Ok(tool_err(e)),
        };
        let _ = conn.call("Accessibility.enable", json!({}), Some(&session)).await;
        let res = match conn
            .call("Accessibility.getFullAXTree", json!({}), Some(&session))
            .await
        {
            Ok(v) => v,
            Err(e) => return Ok(tool_err(e)),
        };
        let (text, refs) = format_ax_tree(&res);
        conn.set_snapshot(&target, refs);
        Ok(text_ok(text))
    }

    #[tool(description = "点击 snapshot 中某个 ref 指向的元素（走真实鼠标事件）。")]
    async fn click(
        &self,
        Parameters(a): Parameters<ClickArgs>,
    ) -> Result<CallToolResult, McpError> {
        let (conn, session, target) =
            match self.session_for(&a.browser_id, a.tab_id.clone()).await {
                Ok(v) => v,
                Err(e) => return Ok(tool_err(e)),
            };
        let backend_id = match conn.resolve_ref(&target, &a.r#ref) {
            Some(b) => b,
            None => {
                return Ok(tool_err(format!(
                    "ref `{}` not found; call snapshot again to refresh refs",
                    a.r#ref
                )))
            }
        };
        Ok(match self.click_backend(&conn, &session, backend_id).await {
            Ok(()) => text_ok(format!("clicked {}", a.r#ref)),
            Err(e) => tool_err(e),
        })
    }

    #[tool(description = "聚焦某个 ref 元素并输入文本（真实输入事件）；submit=true 时输入后回车。")]
    async fn type_text(
        &self,
        Parameters(a): Parameters<TypeTextArgs>,
    ) -> Result<CallToolResult, McpError> {
        let (conn, session, target) =
            match self.session_for(&a.browser_id, a.tab_id.clone()).await {
                Ok(v) => v,
                Err(e) => return Ok(tool_err(e)),
            };
        let backend_id = match conn.resolve_ref(&target, &a.r#ref) {
            Some(b) => b,
            None => {
                return Ok(tool_err(format!(
                    "ref `{}` not found; call snapshot again to refresh refs",
                    a.r#ref
                )))
            }
        };
        // 聚焦：优先 DOM.focus，失败则点击一次兜底。
        let _ = conn
            .call("DOM.scrollIntoViewIfNeeded", json!({ "backendNodeId": backend_id }), Some(&session))
            .await;
        if conn
            .call("DOM.focus", json!({ "backendNodeId": backend_id }), Some(&session))
            .await
            .is_err()
        {
            let _ = self.click_backend(&conn, &session, backend_id).await;
        }
        if let Err(e) = conn
            .call("Input.insertText", json!({ "text": a.text }), Some(&session))
            .await
        {
            return Ok(tool_err(e));
        }
        if a.submit.unwrap_or(false) {
            let _ = dispatch_key(&conn, &session, "Enter").await;
        }
        Ok(text_ok(format!("typed into {}", a.r#ref)))
    }

    #[tool(description = "在当前标签页按下一个键（Enter / Tab / Escape / 方向键 / 单字符等）。")]
    async fn press_key(
        &self,
        Parameters(a): Parameters<PressKeyArgs>,
    ) -> Result<CallToolResult, McpError> {
        let (conn, session, _target) = match self.session_for(&a.browser_id, a.tab_id).await {
            Ok(v) => v,
            Err(e) => return Ok(tool_err(e)),
        };
        Ok(match dispatch_key(&conn, &session, &a.key).await {
            Ok(()) => text_ok(format!("pressed {}", a.key)),
            Err(e) => tool_err(e),
        })
    }

    #[tool(description = "在当前标签页滚动页面（真实滚轮事件）。")]
    async fn scroll(
        &self,
        Parameters(a): Parameters<ScrollArgs>,
    ) -> Result<CallToolResult, McpError> {
        let (conn, session, _target) =
            match self.session_for(&a.browser_id, a.tab_id.clone()).await {
                Ok(v) => v,
                Err(e) => return Ok(tool_err(e)),
            };
        let amount = a.amount.unwrap_or(600.0).abs();
        let delta_y = if a.direction.as_deref() == Some("up") {
            -amount
        } else {
            amount
        };
        // 取视口中心作为滚轮落点。
        let (cx, cy) = conn
            .call(
                "Runtime.evaluate",
                json!({ "expression": "[Math.floor(innerWidth/2), Math.floor(innerHeight/2)]", "returnByValue": true }),
                Some(&session),
            )
            .await
            .ok()
            .and_then(|v| {
                let arr = v.get("result")?.get("value")?.as_array()?.clone();
                Some((arr.first()?.as_f64()? , arr.get(1)?.as_f64()?))
            })
            .unwrap_or((200.0, 200.0));
        Ok(
            match conn
                .call(
                    "Input.dispatchMouseEvent",
                    json!({ "type": "mouseWheel", "x": cx, "y": cy, "deltaX": 0, "deltaY": delta_y }),
                    Some(&session),
                )
                .await
            {
                Ok(_) => text_ok(format!("scrolled {}", if delta_y < 0.0 { "up" } else { "down" })),
                Err(e) => tool_err(e),
            },
        )
    }

    #[tool(description = "等待页面出现/消失某段文本，或固定等待若干秒。")]
    async fn wait_for(
        &self,
        Parameters(a): Parameters<WaitForArgs>,
    ) -> Result<CallToolResult, McpError> {
        let (conn, session, _target) = match self.session_for(&a.browser_id, a.tab_id).await {
            Ok(v) => v,
            Err(e) => return Ok(tool_err(e)),
        };
        if let Some(secs) = a.time {
            tokio::time::sleep(Duration::from_secs_f64(secs.clamp(0.0, 120.0))).await;
            return Ok(text_ok("waited".to_string()));
        }
        let (needle, want_present) = match (a.text, a.text_gone) {
            (Some(t), _) if !t.is_empty() => (t, true),
            (_, Some(t)) if !t.is_empty() => (t, false),
            _ => return Ok(tool_err("provide one of: text, text_gone, time".to_string())),
        };
        let escaped = needle.replace('\\', "\\\\").replace('`', "\\`");
        for _ in 0..40 {
            let expr = format!(
                "(document.body ? document.body.innerText : '').includes(`{escaped}`)"
            );
            let present = conn
                .call_poll(
                    "Runtime.evaluate",
                    json!({ "expression": expr, "returnByValue": true }),
                    Some(&session),
                )
                .await
                .ok()
                .and_then(|v| js_bool(&v))
                .unwrap_or(false);
            if present == want_present {
                return Ok(text_ok("condition met".to_string()));
            }
            tokio::time::sleep(Duration::from_millis(500)).await;
        }
        Ok(tool_err("wait_for timed out".to_string()))
    }

    #[tool(description = "对当前标签页截图，返回 PNG 图像。")]
    async fn screenshot(
        &self,
        Parameters(a): Parameters<ScreenshotArgs>,
    ) -> Result<CallToolResult, McpError> {
        let (conn, session, _target) =
            match self.session_for(&a.browser_id, a.tab_id.clone()).await {
                Ok(v) => v,
                Err(e) => return Ok(tool_err(e)),
            };
        let mut params = json!({ "format": "png" });
        if a.full_page.unwrap_or(false) {
            if let Ok(m) = conn
                .call("Page.getLayoutMetrics", json!({}), Some(&session))
                .await
            {
                let size = m
                    .get("cssContentSize")
                    .or_else(|| m.get("contentSize"))
                    .cloned()
                    .unwrap_or(Value::Null);
                let w = size.get("width").and_then(|x| x.as_f64()).unwrap_or(0.0);
                let h = size.get("height").and_then(|x| x.as_f64()).unwrap_or(0.0);
                if w > 0.0 && h > 0.0 {
                    params["clip"] = json!({ "x": 0, "y": 0, "width": w, "height": h, "scale": 1 });
                    params["captureBeyondViewport"] = json!(true);
                }
            }
        }
        Ok(
            match conn.call("Page.captureScreenshot", params, Some(&session)).await {
                Ok(v) => {
                    let data = v.get("data").and_then(|x| x.as_str()).unwrap_or("");
                    if data.is_empty() {
                        tool_err("screenshot returned empty data".to_string())
                    } else {
                        CallToolResult::success(vec![ContentBlock::image(
                            data.to_string(),
                            "image/png".to_string(),
                        )])
                    }
                }
                Err(e) => tool_err(e),
            },
        )
    }

    #[tool(description = "在当前标签页的页面上下文执行 JavaScript 表达式并返回结果（支持 Promise）。")]
    async fn evaluate(
        &self,
        Parameters(a): Parameters<EvaluateArgs>,
    ) -> Result<CallToolResult, McpError> {
        let (conn, session, _target) = match self.session_for(&a.browser_id, a.tab_id).await {
            Ok(v) => v,
            Err(e) => return Ok(tool_err(e)),
        };
        Ok(
            match conn
                .call(
                    "Runtime.evaluate",
                    json!({ "expression": a.expression, "returnByValue": true, "awaitPromise": true }),
                    Some(&session),
                )
                .await
            {
                Ok(v) => {
                    if let Some(exc) = v.get("exceptionDetails") {
                        let text = exc
                            .get("exception")
                            .and_then(|e| e.get("description"))
                            .and_then(|d| d.as_str())
                            .or_else(|| exc.get("text").and_then(|t| t.as_str()))
                            .unwrap_or("evaluation error");
                        tool_err(text.to_string())
                    } else {
                        let value = v
                            .get("result")
                            .and_then(|r| r.get("value"))
                            .cloned()
                            .unwrap_or(Value::Null);
                        json_ok(value)
                    }
                }
                Err(e) => tool_err(e),
            },
        )
    }
}

// ---------------------------------------------------------------------------
// 内部辅助
// ---------------------------------------------------------------------------

impl BrioMcp {
    /// 取某 browser 当前的 CDP 端口；未运行/未就绪返回可读错误。
    fn running_port(&self, browser_id: &str) -> Result<u16, String> {
        let running = self
            .app
            .state::<BrowserSupervisor>()
            .get_running(browser_id)
            .ok_or_else(|| "browser is not running; open it first".to_string())?;
        if running.cdp_port == 0 {
            return Err("browser CDP port is not ready yet".to_string());
        }
        Ok(running.cdp_port)
    }

    /// 取（或建立）某 browser 的 CDP 连接。
    async fn conn_for(&self, browser_id: &str) -> Result<Arc<CdpConn>, String> {
        let port = self.running_port(browser_id)?;
        self.pool.get(browser_id, port).await
    }

    /// 解析出连接 + 目标 tab 的 session + targetId。
    async fn session_for(
        &self,
        browser_id: &str,
        tab_id: Option<String>,
    ) -> Result<(Arc<CdpConn>, String, String), String> {
        let port = self.running_port(browser_id)?;
        let conn = self.pool.get(browser_id, port).await?;
        let target = match tab_id.filter(|s| !s.trim().is_empty()) {
            Some(t) => t,
            None => cdp::list_page_targets(port)
                .await?
                .into_iter()
                .next()
                .map(|(id, _, _)| id)
                .ok_or_else(|| "no open page/tab found".to_string())?,
        };
        let session = conn.attach(&target).await?;
        Ok((conn, session, target))
    }

    /// 通过 backendNodeId 拿盒模型中心点，派发真实的按下/抬起鼠标事件。
    async fn click_backend(
        &self,
        conn: &CdpConn,
        session: &str,
        backend_id: i64,
    ) -> Result<(), String> {
        let _ = conn
            .call("DOM.scrollIntoViewIfNeeded", json!({ "backendNodeId": backend_id }), Some(session))
            .await;
        let model = conn
            .call("DOM.getBoxModel", json!({ "backendNodeId": backend_id }), Some(session))
            .await?;
        let content = model
            .get("model")
            .and_then(|m| m.get("content"))
            .and_then(|c| c.as_array())
            .ok_or_else(|| "element has no box model (not visible?)".to_string())?;
        if content.len() < 6 {
            return Err("unexpected box model".to_string());
        }
        let x = (content[0].as_f64().unwrap_or(0.0) + content[4].as_f64().unwrap_or(0.0)) / 2.0;
        let y = (content[1].as_f64().unwrap_or(0.0) + content[5].as_f64().unwrap_or(0.0)) / 2.0;
        conn.call(
            "Input.dispatchMouseEvent",
            json!({ "type": "mousePressed", "x": x, "y": y, "button": "left", "buttons": 1, "clickCount": 1 }),
            Some(session),
        )
        .await?;
        conn.call(
            "Input.dispatchMouseEvent",
            json!({ "type": "mouseReleased", "x": x, "y": y, "button": "left", "buttons": 1, "clickCount": 1 }),
            Some(session),
        )
        .await?;
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// 自由函数辅助
// ---------------------------------------------------------------------------

fn json_ok(v: Value) -> CallToolResult {
    let text = serde_json::to_string_pretty(&v).unwrap_or_else(|_| v.to_string());
    CallToolResult::success(vec![ContentBlock::text(text)])
}

fn text_ok(s: impl Into<String>) -> CallToolResult {
    CallToolResult::success(vec![ContentBlock::text(s.into())])
}

fn tool_err(s: impl Into<String>) -> CallToolResult {
    CallToolResult::error(vec![ContentBlock::text(s.into())])
}

/// `call()` 已剥掉 CDP 外层 `result`，这里兼容 `{result:{value}}` 与直接 `{value}`。
fn js_value(v: &Value) -> Option<&Value> {
    v.get("result")
        .and_then(|r| r.get("value"))
        .or_else(|| v.get("value"))
}

fn js_string(v: &Value) -> Option<String> {
    js_value(v)?.as_str().map(|s| s.to_string())
}

fn js_bool(v: &Value) -> Option<bool> {
    js_value(v)?.as_bool()
}

/// 序列化 Browser 并对 proxy_url 打码。
fn masked(b: &Browser) -> Value {
    let mut v = serde_json::to_value(b).unwrap_or(Value::Null);
    if let Some(p) = v.get_mut("proxy_url") {
        if let Some(s) = p.as_str() {
            *p = Value::String(mask_proxy(s));
        }
    }
    v
}

/// `scheme://user:pass@host:port` → `scheme://***:***@host:port`，无凭证则原样返回。
fn mask_proxy(url: &str) -> String {
    if url.is_empty() {
        return String::new();
    }
    let Some((scheme, rest)) = url.split_once("://") else {
        return url.to_string();
    };
    if let Some((auth, host)) = rest.split_once('@') {
        if !auth.is_empty() {
            return format!("{scheme}://***:***@{host}");
        }
    }
    url.to_string()
}

/// 常见键名 → (key, code, windowsVirtualKeyCode, text)。
fn key_def(name: &str) -> (String, String, i64, Option<String>) {
    match name {
        "Enter" => ("Enter".into(), "Enter".into(), 13, Some("\r".into())),
        "Tab" => ("Tab".into(), "Tab".into(), 9, None),
        "Escape" | "Esc" => ("Escape".into(), "Escape".into(), 27, None),
        "Backspace" => ("Backspace".into(), "Backspace".into(), 8, None),
        "Delete" => ("Delete".into(), "Delete".into(), 46, None),
        "ArrowDown" => ("ArrowDown".into(), "ArrowDown".into(), 40, None),
        "ArrowUp" => ("ArrowUp".into(), "ArrowUp".into(), 38, None),
        "ArrowLeft" => ("ArrowLeft".into(), "ArrowLeft".into(), 37, None),
        "ArrowRight" => ("ArrowRight".into(), "ArrowRight".into(), 39, None),
        "PageDown" => ("PageDown".into(), "PageDown".into(), 34, None),
        "PageUp" => ("PageUp".into(), "PageUp".into(), 33, None),
        "Home" => ("Home".into(), "Home".into(), 36, None),
        "End" => ("End".into(), "End".into(), 35, None),
        other => {
            // 单字符：作为可打印字符输入。
            let text = other.to_string();
            let code = if other.len() == 1 {
                let c = other.chars().next().unwrap();
                if c.is_ascii_alphabetic() {
                    format!("Key{}", c.to_ascii_uppercase())
                } else if c.is_ascii_digit() {
                    format!("Digit{c}")
                } else {
                    String::new()
                }
            } else {
                String::new()
            };
            let vk = if other.len() == 1 {
                other.chars().next().unwrap().to_ascii_uppercase() as i64
            } else {
                0
            };
            (other.to_string(), code, vk, Some(text))
        }
    }
}

/// 派发一次真实按键（rawKeyDown + keyUp）。
async fn dispatch_key(conn: &CdpConn, session: &str, name: &str) -> Result<(), String> {
    let (key, code, vk, text) = key_def(name);
    let mut down = json!({
        "type": if text.is_some() { "keyDown" } else { "rawKeyDown" },
        "key": key,
        "code": code,
        "windowsVirtualKeyCode": vk,
        "nativeVirtualKeyCode": vk,
    });
    if let Some(t) = text.as_ref() {
        down["text"] = json!(t);
    }
    conn.call("Input.dispatchKeyEvent", down, Some(session)).await?;
    let up = json!({
        "type": "keyUp",
        "key": key,
        "code": code,
        "windowsVirtualKeyCode": vk,
        "nativeVirtualKeyCode": vk,
    });
    conn.call("Input.dispatchKeyEvent", up, Some(session)).await?;
    Ok(())
}

#[tool_handler(
    name = "brio",
    version = "0.1.0",
    instructions = "Brio 指纹浏览器的 MCP server。用 list_browsers 查看/定位浏览器，open_browser 启动后用 navigate/snapshot/click/type_text 等按 browser_id 操作页面。所有页面工具都需要先 open_browser 让目标浏览器进入运行态。"
)]
impl ServerHandler for BrioMcp {}

#[cfg(test)]
mod tests {
    use super::*;

    const EXPECTED_TOOLS: &[&str] = &[
        // 管理层
        "list_browsers",
        "get_browser",
        "create_browser",
        "update_browser",
        "delete_browser",
        "open_browser",
        "close_browser",
        "detect_timezone",
        // 页面操作层
        "list_tabs",
        "new_tab",
        "close_tab",
        "navigate",
        "snapshot",
        "click",
        "type_text",
        "press_key",
        "scroll",
        "wait_for",
        "screenshot",
        "evaluate",
    ];

    #[test]
    fn all_tools_registered() {
        let router = BrioMcp::tool_router();
        for name in EXPECTED_TOOLS {
            assert!(router.has_route(name), "missing tool `{name}`");
        }
        assert_eq!(
            router.list_all().len(),
            EXPECTED_TOOLS.len(),
            "unexpected number of registered tools"
        );
    }

    #[test]
    fn mask_proxy_hides_credentials() {
        assert_eq!(
            mask_proxy("socks5://user:pass@host:1080"),
            "socks5://***:***@host:1080"
        );
        assert_eq!(mask_proxy("http://host:8080"), "http://host:8080");
        assert_eq!(mask_proxy(""), "");
    }

    #[test]
    fn key_def_maps_named_and_char_keys() {
        let (key, code, vk, text) = key_def("Enter");
        assert_eq!(key, "Enter");
        assert_eq!(code, "Enter");
        assert_eq!(vk, 13);
        assert_eq!(text.as_deref(), Some("\r"));

        let (key, code, _vk, text) = key_def("a");
        assert_eq!(key, "a");
        assert_eq!(code, "KeyA");
        assert_eq!(text.as_deref(), Some("a"));
    }
}
