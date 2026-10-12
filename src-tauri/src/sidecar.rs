//! 遗留的 Python sidecar 进程管理（当前未接入 `lib.rs` 运行时路径）。
//!
//! 浏览器生命周期已由 [`crate::browser_supervisor::BrowserSupervisor`] 托管；
//! 本模块保留 `spawn_session` 等旧 identity 时代的类型与实现，仅因
//! `#[allow(dead_code)]` 仍编译进 crate，请勿再按此处 API 写新代码。

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tauri::{AppHandle, Emitter};
use tauri_plugin_shell::process::{CommandChild, CommandEvent};
use tauri_plugin_shell::ShellExt;
use tokio::sync::mpsc;

#[derive(Default)]
pub struct SessionRegistry {
    inner: Mutex<HashMap<String, SessionHandle>>,
}

pub struct SessionHandle {
    pub child: Arc<Mutex<Option<CommandChild>>>,
    pub stdin_tx: mpsc::UnboundedSender<String>,
}

impl SessionRegistry {
    pub fn insert(&self, id: String, handle: SessionHandle) {
        self.inner.lock().unwrap().insert(id, handle);
    }

    pub fn remove(&self, id: &str) -> Option<SessionHandle> {
        self.inner.lock().unwrap().remove(id)
    }

    pub fn get_stdin_tx(&self, id: &str) -> Option<mpsc::UnboundedSender<String>> {
        self.inner.lock().unwrap().get(id).map(|h| h.stdin_tx.clone())
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct StartSessionParams {
    pub proxy_url: String,
    /// 旧 identity 提取时代的类型列表；当前无调用方。
    pub types: Vec<String>,
    pub timezone: Option<String>,
    pub fingerprint_id: String,
    pub fingerprint_platform: Option<String>,
    pub fingerprint_brand: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct StartSessionResult {
    pub session_id: String,
    /// 旧 identity 导出目录；当前无调用方。
    pub identity_dir: String,
    pub user_data_dir: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct EnvelopedEvent {
    pub session_id: String,
    pub kind: String, // "stdout" | "stderr" | "exit"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payload: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code: Option<i32>,
}

/// 启动 sidecar，返回 session 句柄；调用方负责把 handle 注册进 registry。
pub fn spawn_session(
    app: &AppHandle,
    session_id: String,
    args: Vec<String>,
) -> Result<SessionHandle> {
    let shell = app.shell();
    let (mut rx, child) = shell
        .sidecar("brio-py")
        .context("create sidecar command (brio-py)")?
        .args(args)
        .spawn()
        .context("spawn brio-py sidecar")?;

    let child = Arc::new(Mutex::new(Some(child)));
    let (stdin_tx, mut stdin_rx) = mpsc::unbounded_channel::<String>();

    // stdin 写入线程：收到字符串就写入 sidecar stdin
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

    // stdout / stderr / 退出事件转发
    {
        let app = app.clone();
        let session_id = session_id.clone();
        let child_for_exit = child.clone();
        tokio::spawn(async move {
            let event_name = "session-event";
            while let Some(event) = rx.recv().await {
                match event {
                    CommandEvent::Stdout(bytes) => {
                        let line = String::from_utf8_lossy(&bytes).to_string();
                        for raw in line.split('\n') {
                            let s = raw.trim();
                            if s.is_empty() {
                                continue;
                            }
                            let payload = serde_json::from_str::<Value>(s).ok();
                            let envelope = EnvelopedEvent {
                                session_id: session_id.clone(),
                                kind: "stdout".to_string(),
                                payload,
                                message: if payload_is_some(s) { None } else { Some(s.to_string()) },
                                code: None,
                            };
                            let _ = app.emit(event_name, envelope);
                        }
                    }
                    CommandEvent::Stderr(bytes) => {
                        let line = String::from_utf8_lossy(&bytes).to_string();
                        let envelope = EnvelopedEvent {
                            session_id: session_id.clone(),
                            kind: "stderr".to_string(),
                            payload: None,
                            message: Some(line),
                            code: None,
                        };
                        let _ = app.emit(event_name, envelope);
                    }
                    CommandEvent::Terminated(payload) => {
                        let envelope = EnvelopedEvent {
                            session_id: session_id.clone(),
                            kind: "exit".to_string(),
                            payload: None,
                            message: None,
                            code: payload.code,
                        };
                        let _ = app.emit(event_name, envelope);
                        // 清掉 child 引用
                        let mut guard = child_for_exit.lock().unwrap();
                        *guard = None;
                        break;
                    }
                    CommandEvent::Error(err) => {
                        let envelope = EnvelopedEvent {
                            session_id: session_id.clone(),
                            kind: "stderr".to_string(),
                            payload: None,
                            message: Some(format!("sidecar error: {err}")),
                            code: None,
                        };
                        let _ = app.emit(event_name, envelope);
                    }
                    _ => {}
                }
            }
        });
    }

    Ok(SessionHandle { child, stdin_tx })
}

fn payload_is_some(s: &str) -> bool {
    serde_json::from_str::<Value>(s).is_ok()
}

/// 主动终止 sidecar：优先发送 stop 命令，给 finalize 一点时间，然后强杀。
pub async fn terminate_session(handle: SessionHandle) -> Result<()> {
    // 1. 通知 sidecar 主动停止
    let _ = handle.stdin_tx.send("{\"cmd\":\"stop\"}\n".to_string());

    // 2. 等几秒让 finalize 完成
    for _ in 0..40 {
        tokio::time::sleep(std::time::Duration::from_millis(150)).await;
        let alive = {
            let guard = handle.child.lock().unwrap();
            guard.is_some()
        };
        if !alive {
            return Ok(());
        }
    }

    // 3. 兜底强杀
    let child_opt = {
        let mut guard = handle.child.lock().unwrap();
        guard.take()
    };
    if let Some(c) = child_opt {
        let _ = c.kill();
    }
    Ok(())
}
