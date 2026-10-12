//! 轻量 Chrome DevTools Protocol 客户端。
//!
//! 设计：
//! - 每个 browser（按 `browser_id`）维护一条到 browser-level WebSocket 的长连接，
//!   通过 Target domain 的 flatten sessions 复用同一连接操作多个 tab。
//! - 命令走 JSON-RPC：全局单调递增 `id` + `pending` oneshot 表做请求/响应匹配。
//! - 事件（无 `id` 的消息）一律忽略；需要「等待」的场景（页面加载 / wait_for）
//!   改用 `Runtime.evaluate` 轮询实现，从而无需维护复杂的事件订阅。
//! - 连接断开 → 标记 closed 并 drain 所有 pending；连接池下次取用时自动重连。

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicI64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::sync::{oneshot, mpsc};
use tokio_tungstenite::tungstenite::Message;

const CALL_TIMEOUT: Duration = Duration::from_secs(30);
/// 轮询用的短超时。导航过程中 execution context 会被销毁，
/// 用默认 30s 超时循环等待会把工具卡死数分钟。
const POLL_CALL_TIMEOUT: Duration = Duration::from_secs(2);
const HTTP_TIMEOUT: Duration = Duration::from_secs(2);

/// 某个 tab（target）最近一次 a11y snapshot 产生的 ref → backendDOMNodeId 映射。
#[derive(Default, Clone)]
pub struct SnapshotRefs {
    pub map: HashMap<String, i64>,
}

pub struct CdpConn {
    pub port: u16,
    cmd_tx: mpsc::UnboundedSender<Message>,
    pending: Arc<Mutex<HashMap<i64, oneshot::Sender<Result<Value, String>>>>>,
    next_id: AtomicI64,
    /// targetId → sessionId（flatten attach 后缓存）
    sessions: Mutex<HashMap<String, String>>,
    /// targetId → 最近 snapshot 的 ref 表
    snapshots: Mutex<HashMap<String, SnapshotRefs>>,
    closed: Arc<AtomicBool>,
}

impl CdpConn {
    /// 连接到指定端口的 browser-level WS。
    pub async fn connect(port: u16) -> Result<Arc<Self>, String> {
        let version = http_get_json(port, "/json/version").await?;
        let ws_url = version
            .get("webSocketDebuggerUrl")
            .and_then(|v| v.as_str())
            .ok_or_else(|| "CDP /json/version missing webSocketDebuggerUrl".to_string())?
            .to_string();

        let (ws_stream, _resp) = tokio_tungstenite::connect_async(ws_url.as_str())
            .await
            .map_err(|e| format!("CDP websocket connect failed: {e}"))?;

        let (mut write, mut read) = ws_stream.split();
        let (cmd_tx, mut cmd_rx) = mpsc::unbounded_channel::<Message>();
        let pending: Arc<Mutex<HashMap<i64, oneshot::Sender<Result<Value, String>>>>> =
            Arc::new(Mutex::new(HashMap::new()));
        let closed = Arc::new(AtomicBool::new(false));

        // 写任务：把命令写入 WS
        {
            let closed = closed.clone();
            tokio::spawn(async move {
                while let Some(msg) = cmd_rx.recv().await {
                    if write.send(msg).await.is_err() {
                        break;
                    }
                }
                closed.store(true, Ordering::SeqCst);
                let _ = write.close().await;
            });
        }

        // 读任务：分派响应，忽略事件
        {
            let pending = pending.clone();
            let closed = closed.clone();
            tokio::spawn(async move {
                while let Some(item) = read.next().await {
                    match item {
                        Ok(Message::Text(txt)) => dispatch(&pending, &txt),
                        Ok(Message::Binary(_)) | Ok(Message::Ping(_)) | Ok(Message::Pong(_)) => {}
                        Ok(Message::Close(_)) | Err(_) => break,
                        Ok(_) => {}
                    }
                }
                closed.store(true, Ordering::SeqCst);
                // drain 所有等待中的调用
                let mut guard = pending.lock().unwrap();
                for (_, tx) in guard.drain() {
                    let _ = tx.send(Err("CDP connection closed".to_string()));
                }
            });
        }

        Ok(Arc::new(Self {
            port,
            cmd_tx,
            pending,
            next_id: AtomicI64::new(1),
            sessions: Mutex::new(HashMap::new()),
            snapshots: Mutex::new(HashMap::new()),
            closed,
        }))
    }

    pub fn is_closed(&self) -> bool {
        self.closed.load(Ordering::SeqCst)
    }

    /// 发一条 CDP 命令并等待结果（默认超时）。
    pub async fn call(
        &self,
        method: &str,
        params: Value,
        session_id: Option<&str>,
    ) -> Result<Value, String> {
        self.call_timeout(method, params, session_id, CALL_TIMEOUT)
            .await
    }

    /// 轮询场景（readyState / wait_for）：失败要快，不能每次卡满 30s。
    pub async fn call_poll(
        &self,
        method: &str,
        params: Value,
        session_id: Option<&str>,
    ) -> Result<Value, String> {
        self.call_timeout(method, params, session_id, POLL_CALL_TIMEOUT)
            .await
    }

    pub async fn call_timeout(
        &self,
        method: &str,
        params: Value,
        session_id: Option<&str>,
        timeout: Duration,
    ) -> Result<Value, String> {
        if self.is_closed() {
            return Err("CDP connection closed".to_string());
        }
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let mut msg = json!({ "id": id, "method": method, "params": params });
        if let Some(s) = session_id {
            msg["sessionId"] = Value::String(s.to_string());
        }
        let (tx, rx) = oneshot::channel();
        self.pending.lock().unwrap().insert(id, tx);

        if self.cmd_tx.send(Message::text(msg.to_string())).is_err() {
            self.pending.lock().unwrap().remove(&id);
            return Err("CDP connection closed".to_string());
        }

        match tokio::time::timeout(timeout, rx).await {
            Ok(Ok(res)) => res,
            Ok(Err(_)) => Err("CDP response channel dropped".to_string()),
            Err(_) => {
                self.pending.lock().unwrap().remove(&id);
                Err(format!("CDP call `{method}` timed out"))
            }
        }
    }

    /// attach 到某个 target（flatten），返回 sessionId；结果缓存复用。
    /// attach 后顺带 enable 常用 domain（失败不致命）。
    pub async fn attach(&self, target_id: &str) -> Result<String, String> {
        if let Some(sid) = self.sessions.lock().unwrap().get(target_id).cloned() {
            return Ok(sid);
        }
        let res = self
            .call(
                "Target.attachToTarget",
                json!({ "targetId": target_id, "flatten": true }),
                None,
            )
            .await?;
        let sid = res
            .get("sessionId")
            .and_then(|v| v.as_str())
            .ok_or_else(|| "Target.attachToTarget missing sessionId".to_string())?
            .to_string();
        self.sessions
            .lock()
            .unwrap()
            .insert(target_id.to_string(), sid.clone());

        for m in ["Page.enable", "DOM.enable", "Runtime.enable"] {
            let _ = self.call(m, json!({}), Some(&sid)).await;
        }
        Ok(sid)
    }

    pub fn set_snapshot(&self, target_id: &str, refs: SnapshotRefs) {
        self.snapshots
            .lock()
            .unwrap()
            .insert(target_id.to_string(), refs);
    }

    /// 把 ref（如 "e5"）解析成 backendDOMNodeId；未命中说明 snapshot 已过期。
    pub fn resolve_ref(&self, target_id: &str, r: &str) -> Option<i64> {
        self.snapshots
            .lock()
            .unwrap()
            .get(target_id)
            .and_then(|s| s.map.get(r).copied())
    }
}

fn dispatch(
    pending: &Arc<Mutex<HashMap<i64, oneshot::Sender<Result<Value, String>>>>>,
    txt: &str,
) {
    let Ok(v) = serde_json::from_str::<Value>(txt) else {
        return;
    };
    // 只关心带 id 的响应；事件（无 id）忽略。
    let Some(id) = v.get("id").and_then(|i| i.as_i64()) else {
        return;
    };
    let tx = pending.lock().unwrap().remove(&id);
    let Some(tx) = tx else { return };
    if let Some(err) = v.get("error") {
        let msg = err
            .get("message")
            .and_then(|m| m.as_str())
            .unwrap_or("unknown CDP error");
        let _ = tx.send(Err(format!("CDP error: {msg}")));
    } else {
        let _ = tx.send(Ok(v.get("result").cloned().unwrap_or(Value::Null)));
    }
}

/// 极简 HTTP GET，仅用于 CDP 本机 `/json/*` 端点。
///
/// Chromium 的 DevTools HTTP 会带 `Content-Length`，但**不会**因
/// `Connection: close` 断开；`read_to_end` 会永远阻塞。必须按长度读完就停。
async fn http_get_json(port: u16, path: &str) -> Result<Value, String> {
    let fut = async {
        let mut stream = TcpStream::connect(("127.0.0.1", port))
            .await
            .map_err(|e| format!("connect CDP http 127.0.0.1:{port} failed: {e}"))?;
        let req = format!(
            "GET {path} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nAccept: application/json\r\nConnection: close\r\n\r\n"
        );
        stream
            .write_all(req.as_bytes())
            .await
            .map_err(|e| e.to_string())?;

        let mut buf = Vec::new();
        let mut tmp = [0u8; 4096];
        loop {
            let n = stream.read(&mut tmp).await.map_err(|e| e.to_string())?;
            if n == 0 {
                break;
            }
            buf.extend_from_slice(&tmp[..n]);
            if let Some(sep) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
                let body_start = sep + 4;
                if let Some(len) = http_content_length(&buf[..sep]) {
                    if buf.len().saturating_sub(body_start) >= len {
                        return parse_cdp_json(&buf[body_start..body_start + len], path);
                    }
                }
            }
        }
        let sep = buf
            .windows(4)
            .position(|w| w == b"\r\n\r\n")
            .ok_or_else(|| "malformed CDP http response".to_string())?;
        parse_cdp_json(&buf[sep + 4..], path)
    };
    tokio::time::timeout(HTTP_TIMEOUT, fut)
        .await
        .map_err(|_| format!("CDP http {path} timed out"))?
}

fn http_content_length(header_bytes: &[u8]) -> Option<usize> {
    let headers = std::str::from_utf8(header_bytes).ok()?;
    for line in headers.split("\r\n") {
        let Some((k, v)) = line.split_once(':') else {
            continue;
        };
        if k.eq_ignore_ascii_case("content-length") {
            return v.trim().parse().ok();
        }
    }
    None
}

fn parse_cdp_json(body: &[u8], path: &str) -> Result<Value, String> {
    serde_json::from_slice(body).map_err(|e| format!("parse CDP json ({path}): {e}"))
}

/// Chromium 是否已经在该端口接受 DevTools HTTP（`/json/version` 可解析）。
pub async fn probe_ready(port: u16) -> bool {
    http_get_json(port, "/json/version").await.is_ok()
}

/// 列出某端口浏览器的所有 page target（走 HTTP `/json/list`）。
/// 返回 (targetId, title, url) 列表。
pub async fn list_page_targets(port: u16) -> Result<Vec<(String, String, String)>, String> {
    let v = http_get_json(port, "/json/list").await?;
    let arr = v.as_array().cloned().unwrap_or_default();
    let mut out = Vec::new();
    for t in arr {
        if t.get("type").and_then(|x| x.as_str()) != Some("page") {
            continue;
        }
        let id = t.get("id").and_then(|x| x.as_str()).unwrap_or("").to_string();
        if id.is_empty() {
            continue;
        }
        let title = t
            .get("title")
            .and_then(|x| x.as_str())
            .unwrap_or("")
            .to_string();
        let url = t.get("url").and_then(|x| x.as_str()).unwrap_or("").to_string();
        out.push((id, title, url));
    }
    Ok(out)
}

/// CDP 连接池：按 browser_id 复用连接。
#[derive(Default)]
pub struct CdpPool {
    conns: Mutex<HashMap<String, Arc<CdpConn>>>,
}

impl CdpPool {
    /// 取某 browser 的连接；无有效连接则按给定端口新建。
    pub async fn get(&self, browser_id: &str, port: u16) -> Result<Arc<CdpConn>, String> {
        if port == 0 {
            return Err("browser has no active CDP port (is it running?)".to_string());
        }
        // 已有且未断、端口一致 → 直接复用
        {
            let guard = self.conns.lock().unwrap();
            if let Some(c) = guard.get(browser_id) {
                if !c.is_closed() && c.port == port {
                    return Ok(c.clone());
                }
            }
        }
        let conn = CdpConn::connect(port).await?;
        self.conns
            .lock()
            .unwrap()
            .insert(browser_id.to_string(), conn.clone());
        Ok(conn)
    }

    /// 主动丢弃某 browser 的连接（浏览器关闭时调用）。
    pub fn drop_conn(&self, browser_id: &str) {
        self.conns.lock().unwrap().remove(browser_id);
    }
}

#[cfg(test)]
mod tests {
    use super::http_content_length;

    #[test]
    fn content_length_skips_status_line() {
        let h = b"HTTP/1.1 200 OK\r\nContent-Length: 459\r\nContent-Type: application/json";
        assert_eq!(http_content_length(h), Some(459));
    }
}
