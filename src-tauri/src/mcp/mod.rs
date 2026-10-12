//! 内嵌 MCP server：把浏览器管理 + 页面操作（CDP）能力暴露给外部 MCP client。
//!
//! - 传输：rmcp streamable HTTP，挂载在 `http://127.0.0.1:<port>/mcp`
//! - 鉴权：`Authorization: Bearer <token>`，token 持久化在 app_settings
//!   （见 [`crate::settings`]），设置界面可查看/重置
//! - 生命周期：[`McpServer`] 作为 tauri managed state 持有运行句柄，
//!   设置变更时 stop 后按新配置重启

mod cdp;
mod handler;
mod snapshot;

use std::sync::{Arc, Mutex};

use axum::extract::Request;
use axum::http::{header, StatusCode};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use rmcp::transport::streamable_http_server::session::local::LocalSessionManager;
use rmcp::transport::streamable_http_server::{StreamableHttpServerConfig, StreamableHttpService};
use tauri::AppHandle;
use tokio::sync::oneshot;

use self::cdp::CdpPool;
use self::handler::BrioMcp;

/// 内嵌 MCP server 的运行句柄（tauri managed state）。
#[derive(Default)]
pub struct McpServer {
    inner: Mutex<Option<Running>>,
}

struct Running {
    port: u16,
    /// send（或 drop）即让 serve 任务退出。
    cancel: oneshot::Sender<()>,
}

impl McpServer {
    pub fn running_port(&self) -> Option<u16> {
        self.inner.lock().unwrap().as_ref().map(|r| r.port)
    }

    /// 启动 server（已在跑则先停旧实例）。bind 失败（端口被占等）
    /// 直接返回错误，让调用方（设置界面）立刻可见。
    pub async fn start(&self, app: AppHandle, port: u16, token: String) -> Result<(), String> {
        self.stop().await;

        let listener = tokio::net::TcpListener::bind(("127.0.0.1", port))
            .await
            .map_err(|e| format!("bind 127.0.0.1:{port} failed: {e}"))?;

        let token = Arc::new(token);
        let pool = Arc::new(CdpPool::default());
        let service = StreamableHttpService::new(
            move || Ok(BrioMcp::new(app.clone(), pool.clone())),
            Arc::new(LocalSessionManager::default()),
            StreamableHttpServerConfig::default(),
        );
        let router = axum::Router::new()
            .nest_service("/mcp", service)
            .layer(middleware::from_fn(move |req: Request, next: Next| {
                let token = token.clone();
                async move { require_bearer(token, req, next).await }
            }));

        let (cancel_tx, cancel_rx) = oneshot::channel::<()>();
        tauri::async_runtime::spawn(async move {
            tracing::info!("MCP server listening on http://127.0.0.1:{port}/mcp");
            let serve = async move { axum::serve(listener, router).await };
            tokio::select! {
                res = serve => {
                    if let Err(e) = res {
                        tracing::error!("MCP server exited abnormally: {e}");
                    }
                }
                _ = cancel_rx => {
                    tracing::info!("MCP server on port {port} shut down");
                }
            }
        });

        *self.inner.lock().unwrap() = Some(Running {
            port,
            cancel: cancel_tx,
        });
        Ok(())
    }

    /// 停掉当前 server；没在跑则 no-op。
    pub async fn stop(&self) {
        let prev = self.inner.lock().unwrap().take();
        if let Some(r) = prev {
            let _ = r.cancel.send(());
        }
    }
}

async fn require_bearer(token: Arc<String>, req: Request, next: Next) -> Response {
    let ok = req
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .is_some_and(|t| t == token.as_str());
    if ok {
        next.run(req).await
    } else {
        (StatusCode::UNAUTHORIZED, "missing or invalid bearer token").into_response()
    }
}
