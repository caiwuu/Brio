//! BrowserSupervisor：把每个 Browser profile 的 Chromium 实例托管为
//! 一个 Python sidecar 子进程（`launch-browser`），并维护其生命周期。
//!
//! 关键点：
//! - 一个 browser_id 同时只能有一个 sidecar 子进程
//! - sidecar 自然退出 / panic / 被 kill → 通过 stdout/stderr/exit envelope 上报
//! - App 退出时会调用 `shutdown_all_blocking()` 兜底确保 Chromium 不残留
//! - `BrowserStatus` 由 sidecar 的 `stage` 事件驱动，前端拿这个去渲染状态点

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use serde::Serialize;
use serde_json::Value;
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_shell::process::{CommandChild, CommandEvent};
use tauri_plugin_shell::ShellExt;
use tokio::sync::mpsc;

use crate::db::{now_ts, DbState};

pub const EVENT_NAME: &str = "browser-event";

/// 发出 stop 后等 sidecar 自己退出的上限，超时才强杀。
/// 必须比 sidecar 里 `BrowserLauncher.terminate()` 的 5s 超时长：Windows 上它要先请 Chromium
/// 正常退出。提前强杀只杀得掉 PyInstaller onefile 的引导进程，真正的 sidecar 会脱离管理继续跑，
/// 解压出来的临时目录也没人清理。
const STOP_GRACE: Duration = Duration::from_secs(6);

/// 浏览器运行时状态机（与 Python `events.STAGE_*` 对齐）。
#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BrowserStatus {
    Starting,
    Running,
    Stopping,
    Stopped,
    Error,
}

impl BrowserStatus {
    fn from_stage(stage: &str) -> Option<Self> {
        match stage {
            "preparing" | "detect_tz" | "proxy_check" | "forwarder_started"
            | "browser_starting" => Some(Self::Starting),
            "browser_running" => Some(Self::Running),
            "done" => Some(Self::Stopped),
            "error" => Some(Self::Error),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct RunningBrowser {
    pub browser_id: String,
    pub status: BrowserStatus,
    pub started_at: i64,
    pub pid: Option<u32>,
    /// 远程调试端口；0 表示未启用 / 未就绪。
    #[serde(default)]
    pub cdp_port: u16,
    /// `http://127.0.0.1:<cdp_port>`，没有则空串。
    #[serde(default)]
    pub cdp_endpoint: String,
    /// 浏览器指纹 ID。同一时刻同一 fingerprint 只允许一个 running 实例。
    #[serde(default)]
    pub fingerprint_id: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct BrowserEnvelope {
    pub browser_id: String,
    pub kind: String, // "stdout" | "stderr" | "exit" | "status"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<BrowserStatus>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payload: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pid: Option<u32>,
}

struct Handle {
    child: Arc<Mutex<Option<CommandChild>>>,
    stdin_tx: mpsc::UnboundedSender<String>,
    info: Arc<Mutex<RunningBrowser>>,
}

#[derive(Default)]
pub struct BrowserSupervisor {
    inner: Mutex<HashMap<String, Handle>>,
    /// fingerprint_id → browser_id 反向索引，用于强制"同一指纹同时只能跑一个实例"。
    fingerprint_index: Mutex<HashMap<String, String>>,
}

impl BrowserSupervisor {
    pub fn list_running(&self) -> Vec<RunningBrowser> {
        let map = self.inner.lock().unwrap();
        map.values()
            .map(|h| h.info.lock().unwrap().clone())
            .collect()
    }

    pub fn is_running(&self, browser_id: &str) -> bool {
        self.inner.lock().unwrap().contains_key(browser_id)
    }

    /// 取某个 browser 的当前运行快照（含 CDP 端口/endpoint）；未在跑返回 None。
    pub fn get_running(&self, browser_id: &str) -> Option<RunningBrowser> {
        self.inner
            .lock()
            .unwrap()
            .get(browser_id)
            .map(|h| h.info.lock().unwrap().clone())
    }

    /// 启动一个 sidecar；若 browser_id 已有进程或 fingerprint_id 已被占用会报错。
    /// `fingerprint_id` 是浏览器的唯一标识：同一时刻同一指纹只允许一个 running 实例。
    pub fn start(
        &self,
        app: &AppHandle,
        browser_id: String,
        fingerprint_id: String,
        args: Vec<String>,
    ) -> Result<RunningBrowser> {
        if fingerprint_id.trim().is_empty() {
            anyhow::bail!("fingerprint_id is required to start a browser");
        }
        {
            let map = self.inner.lock().unwrap();
            if map.contains_key(&browser_id) {
                anyhow::bail!("browser {browser_id} is already running");
            }
            let fp = self.fingerprint_index.lock().unwrap();
            if let Some(other) = fp.get(&fingerprint_id) {
                anyhow::bail!(
                    "fingerprint {fingerprint_id} is already in use by browser {other}; close it first"
                );
            }
        }

        let shell = app.shell();
        let (mut rx, child) = shell
            .sidecar("brio-py")
            .context("create sidecar (brio-py)")?
            .args(args)
            .spawn()
            .context("spawn brio-py launch-browser")?;

        let child = Arc::new(Mutex::new(Some(child)));
        let (stdin_tx, mut stdin_rx) = mpsc::unbounded_channel::<String>();

        let info = Arc::new(Mutex::new(RunningBrowser {
            browser_id: browser_id.clone(),
            status: BrowserStatus::Starting,
            started_at: now_ts(),
            pid: None,
            cdp_port: 0,
            cdp_endpoint: String::new(),
            fingerprint_id: fingerprint_id.clone(),
        }));

        // stdin 转发线程
        {
            let child = child.clone();
            tokio::spawn(async move {
                while let Some(line) = stdin_rx.recv().await {
                    let mut guard = child.lock().unwrap();
                    if let Some(c) = guard.as_mut() {
                        if let Err(e) = c.write(line.as_bytes()) {
                            tracing::warn!("sidecar stdin write failed: {e}");
                            break;
                        }
                    }
                }
            });
        }

        // stdout/stderr/exit 转发线程
        {
            let app = app.clone();
            let bid = browser_id.clone();
            let info = info.clone();
            let child_for_exit = child.clone();
            tokio::spawn(async move {
                while let Some(event) = rx.recv().await {
                    match event {
                        CommandEvent::Stdout(bytes) => {
                            let line = String::from_utf8_lossy(&bytes).to_string();
                            for raw in line.split('\n') {
                                let s = raw.trim();
                                if s.is_empty() {
                                    continue;
                                }
                                let payload =
                                    serde_json::from_str::<Value>(s).ok();
                                // 根据 event/stage 更新本地状态
                                let mut new_status = None;
                                if let Some(p) = payload.as_ref() {
                                    let evt = p.get("event").and_then(|e| e.as_str());
                                    if evt == Some("stage") {
                                        if let Some(stage) = p
                                            .get("stage")
                                            .and_then(|s| s.as_str())
                                        {
                                            if let Some(st) =
                                                BrowserStatus::from_stage(stage)
                                            {
                                                new_status = Some(st);
                                            }
                                            if stage == "browser_running" {
                                                if let Some(pid) = p
                                                    .get("pid")
                                                    .and_then(|p| p.as_u64())
                                                {
                                                    info.lock()
                                                        .unwrap()
                                                        .pid = Some(pid as u32);
                                                }
                                            }
                                        }
                                    } else if evt == Some("cdp_port") {
                                        if let Some(port) = p
                                            .get("port")
                                            .and_then(|v| v.as_u64())
                                        {
                                            let port = port as u16;
                                            let endpoint = format!(
                                                "http://127.0.0.1:{port}"
                                            );
                                            {
                                                let mut g = info.lock().unwrap();
                                                g.cdp_port = port;
                                                g.cdp_endpoint = endpoint.clone();
                                            }
                                            // 回写 DB（best-effort，失败仅记日志）
                                            if let Some(db) = app.try_state::<DbState>() {
                                                if let Ok(conn) = db.pool.get() {
                                                    if let Err(e) = conn.execute(
                                                        "UPDATE browsers SET cdp_port = ?1, updated_at = ?2 WHERE id = ?3",
                                                        rusqlite::params![
                                                            port as i64,
                                                            now_ts(),
                                                            bid
                                                        ],
                                                    ) {
                                                        tracing::warn!(
                                                            "persist cdp_port failed: {e}"
                                                        );
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                                if let Some(st) = new_status {
                                    info.lock().unwrap().status = st;
                                }
                                let snapshot = info.lock().unwrap().clone();
                                let env = BrowserEnvelope {
                                    browser_id: bid.clone(),
                                    kind: "stdout".to_string(),
                                    status: Some(snapshot.status),
                                    payload,
                                    message: None,
                                    code: None,
                                    pid: snapshot.pid,
                                };
                                let _ = app.emit(EVENT_NAME, env);
                            }
                        }
                        CommandEvent::Stderr(bytes) => {
                            let line = String::from_utf8_lossy(&bytes).to_string();
                            let env = BrowserEnvelope {
                                browser_id: bid.clone(),
                                kind: "stderr".to_string(),
                                status: None,
                                payload: None,
                                message: Some(line),
                                code: None,
                                pid: None,
                            };
                            let _ = app.emit(EVENT_NAME, env);
                        }
                        CommandEvent::Terminated(payload) => {
                            info.lock().unwrap().status = BrowserStatus::Stopped;
                            let env = BrowserEnvelope {
                                browser_id: bid.clone(),
                                kind: "exit".to_string(),
                                status: Some(BrowserStatus::Stopped),
                                payload: None,
                                message: None,
                                code: payload.code,
                                pid: None,
                            };
                            let _ = app.emit(EVENT_NAME, env);
                            // 清掉 DB 里的 cdp_port（避免上次端口残留）
                            if let Some(db) = app.try_state::<DbState>() {
                                if let Ok(conn) = db.pool.get() {
                                    let _ = conn.execute(
                                        "UPDATE browsers SET cdp_port = 0, updated_at = ?1 WHERE id = ?2",
                                        rusqlite::params![now_ts(), bid],
                                    );
                                }
                            }
                            // 移除自己 + 释放 fingerprint 索引
                            child_for_exit.lock().unwrap().take();
                            if let Some(state) =
                                app.try_state::<BrowserSupervisor>()
                            {
                                state.inner.lock().unwrap().remove(&bid);
                                state
                                    .fingerprint_index
                                    .lock()
                                    .unwrap()
                                    .retain(|_, v| v != &bid);
                            }
                            break;
                        }
                        CommandEvent::Error(err) => {
                            let env = BrowserEnvelope {
                                browser_id: bid.clone(),
                                kind: "stderr".to_string(),
                                status: None,
                                payload: None,
                                message: Some(format!("sidecar error: {err}")),
                                code: None,
                                pid: None,
                            };
                            let _ = app.emit(EVENT_NAME, env);
                        }
                        _ => {}
                    }
                }
            });
        }

        let snapshot = info.lock().unwrap().clone();
        self.fingerprint_index
            .lock()
            .unwrap()
            .insert(fingerprint_id, browser_id.clone());
        self.inner.lock().unwrap().insert(
            browser_id,
            Handle {
                child,
                stdin_tx,
                info,
            },
        );
        Ok(snapshot)
    }

    /// 优雅停 + 兜底强杀。
    pub async fn stop(&self, browser_id: &str) -> Result<()> {
        let handle = {
            let mut map = self.inner.lock().unwrap();
            map.remove(browser_id)
        };
        let Some(handle) = handle else {
            // 兜底：即使 inner 已无该项，也清掉残留的 fingerprint 索引
            self.fingerprint_index
                .lock()
                .unwrap()
                .retain(|_, v| v != browser_id);
            return Ok(());
        };
        // 立即释放 fingerprint，下一次 launch 可以马上拿到
        self.fingerprint_index
            .lock()
            .unwrap()
            .retain(|_, v| v != browser_id);
        handle.info.lock().unwrap().status = BrowserStatus::Stopping;
        let _ = handle.stdin_tx.send("{\"cmd\":\"stop\"}\n".to_string());

        let deadline = Instant::now() + STOP_GRACE;
        while Instant::now() < deadline {
            tokio::time::sleep(Duration::from_millis(150)).await;
            let alive = handle.child.lock().unwrap().is_some();
            if !alive {
                return Ok(());
            }
        }
        let child_opt = handle.child.lock().unwrap().take();
        if let Some(c) = child_opt {
            let _ = c.kill();
        }
        Ok(())
    }

    /// App 退出时同步关闭所有 sidecar（保证 Chromium 不残留）。
    /// 先一起发 stop 再一起等，多个浏览器的等待时间不叠加。
    pub fn shutdown_all_blocking(&self) {
        let handles: Vec<_> = {
            let mut map = self.inner.lock().unwrap();
            map.drain().collect()
        };
        self.fingerprint_index.lock().unwrap().clear();
        for (id, handle) in handles.iter() {
            tracing::info!("shutdown browser {id}");
            let _ = handle.stdin_tx.send("{\"cmd\":\"stop\"}\n".to_string());
        }
        // child 由 Terminated 事件在 async runtime 线程上置空，这里阻塞主线程轮询不会卡住它
        let deadline = Instant::now() + STOP_GRACE;
        while Instant::now() < deadline
            && handles
                .iter()
                .any(|(_, h)| h.child.lock().unwrap().is_some())
        {
            std::thread::sleep(Duration::from_millis(100));
        }
        for (id, handle) in handles {
            let child_opt = handle.child.lock().unwrap().take();
            if let Some(c) = child_opt {
                tracing::warn!("browser {id} did not exit within {STOP_GRACE:?}, killing sidecar");
                let _ = c.kill();
            }
        }
    }
}
