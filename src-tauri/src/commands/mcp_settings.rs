//! MCP server 设置命令：查看/保存配置、重置 token。
//!
//! 保存与重置 token 都会即时应用（先 stop，再按新配置决定是否重启）。

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, State};

use crate::db::DbState;
use crate::mcp::McpServer;
use crate::settings;

/// 返回给前端的 MCP 设置 + 运行状态快照。
#[derive(Debug, Serialize)]
pub struct McpSettingsView {
    pub enabled: bool,
    pub port: u16,
    pub token: String,
    /// `http://127.0.0.1:<port>/mcp`
    pub endpoint: String,
    /// server 当前是否真的在监听。
    pub running: bool,
}

fn view(db: &DbState, server: &McpServer) -> Result<McpSettingsView, String> {
    let cfg = settings::load_mcp_config(db)?;
    Ok(McpSettingsView {
        enabled: cfg.enabled,
        running: server.running_port().is_some(),
        endpoint: cfg.endpoint(),
        port: cfg.port,
        token: cfg.token,
    })
}

#[tauri::command]
pub fn get_mcp_settings(
    db: State<'_, DbState>,
    server: State<'_, McpServer>,
) -> Result<McpSettingsView, String> {
    view(&db, &server)
}

#[derive(Debug, Deserialize)]
pub struct SetMcpSettingsInput {
    pub enabled: bool,
    pub port: u16,
}

#[tauri::command]
pub async fn set_mcp_settings(
    app: AppHandle,
    db: State<'_, DbState>,
    server: State<'_, McpServer>,
    input: SetMcpSettingsInput,
) -> Result<McpSettingsView, String> {
    settings::save_mcp_config(&db, input.enabled, input.port)?;
    let cfg = settings::load_mcp_config(&db)?;
    server.stop().await;
    if cfg.enabled {
        server.start(app, cfg.port, cfg.token.clone()).await?;
    }
    view(&db, &server)
}

#[tauri::command]
pub async fn regenerate_mcp_token(
    app: AppHandle,
    db: State<'_, DbState>,
    server: State<'_, McpServer>,
) -> Result<McpSettingsView, String> {
    settings::regenerate_token(&db)?;
    let cfg = settings::load_mcp_config(&db)?;
    // token 变了：正在跑的话需要用新 token 重启才会生效。
    if server.running_port().is_some() {
        server.stop().await;
        if cfg.enabled {
            server.start(app, cfg.port, cfg.token.clone()).await?;
        }
    }
    view(&db, &server)
}
