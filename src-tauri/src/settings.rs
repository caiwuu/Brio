//! 全局应用设置（`app_settings` KV 表）与内嵌 MCP server 配置。
//!
//! MCP server 的开关 / 端口 / 访问 token 都存在这里。token 在首次加载时
//! 惰性生成并持久化，之后保持稳定（重启不变），供 MCP client 长期配置。

use serde::{Deserialize, Serialize};

use crate::db::{now_ts, DbState};

pub const KEY_MCP_ENABLED: &str = "mcp.enabled";
pub const KEY_MCP_PORT: &str = "mcp.port";
pub const KEY_MCP_TOKEN: &str = "mcp.token";

pub const DEFAULT_MCP_PORT: u16 = 9333;

/// 内嵌 MCP server 的当前配置。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpConfig {
    pub enabled: bool,
    pub port: u16,
    pub token: String,
}

impl McpConfig {
    /// `http://127.0.0.1:<port>/mcp`
    pub fn endpoint(&self) -> String {
        format!("http://127.0.0.1:{}/mcp", self.port)
    }
}

/// 读单个 KV。不存在返回 None。
pub fn get_setting(db: &DbState, key: &str) -> Result<Option<String>, String> {
    let conn = db.pool.get().map_err(|e| e.to_string())?;
    conn.query_row(
        "SELECT value FROM app_settings WHERE key = ?1",
        rusqlite::params![key],
        |r| r.get::<_, String>(0),
    )
    .map(Some)
    .or_else(|e| match e {
        rusqlite::Error::QueryReturnedNoRows => Ok(None),
        other => Err(other.to_string()),
    })
}

/// 写单个 KV（upsert）。
pub fn set_setting(db: &DbState, key: &str, value: &str) -> Result<(), String> {
    let conn = db.pool.get().map_err(|e| e.to_string())?;
    conn.execute(
        "INSERT INTO app_settings (key, value, updated_at) VALUES (?1, ?2, ?3)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = excluded.updated_at",
        rusqlite::params![key, value, now_ts()],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

/// 生成一个新的访问 token（两段无连字符的 uuid v4 拼接，约 256 bit 熵）。
pub fn generate_token() -> String {
    format!(
        "{}{}",
        uuid::Uuid::new_v4().simple(),
        uuid::Uuid::new_v4().simple()
    )
}

/// 加载 MCP 配置。token 缺失时惰性生成并持久化。
pub fn load_mcp_config(db: &DbState) -> Result<McpConfig, String> {
    let enabled = get_setting(db, KEY_MCP_ENABLED)?
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false);

    let port = get_setting(db, KEY_MCP_PORT)?
        .and_then(|v| v.parse::<u16>().ok())
        .filter(|p| *p != 0)
        .unwrap_or(DEFAULT_MCP_PORT);

    let token = match get_setting(db, KEY_MCP_TOKEN)? {
        Some(t) if !t.trim().is_empty() => t,
        _ => {
            let t = generate_token();
            set_setting(db, KEY_MCP_TOKEN, &t)?;
            t
        }
    };

    Ok(McpConfig {
        enabled,
        port,
        token,
    })
}

/// 持久化 enabled / port（token 不动）。
pub fn save_mcp_config(db: &DbState, enabled: bool, port: u16) -> Result<(), String> {
    let port = if port == 0 { DEFAULT_MCP_PORT } else { port };
    set_setting(db, KEY_MCP_ENABLED, if enabled { "1" } else { "0" })?;
    set_setting(db, KEY_MCP_PORT, &port.to_string())?;
    Ok(())
}

/// 重新生成并持久化 token，返回新值。
pub fn regenerate_token(db: &DbState) -> Result<String, String> {
    let t = generate_token();
    set_setting(db, KEY_MCP_TOKEN, &t)?;
    Ok(t)
}
