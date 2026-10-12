//! Browser CRUD 与运行时控制的核心逻辑。
//!
//! 这里的函数接收 `&DbState` / `&BrowserSupervisor` / `&AppHandle`（而非 Tauri
//! 的 `State<>` 包装），从而既能被 `commands::browsers` 的 Tauri command 复用，
//! 也能被内嵌 MCP server 的工具直接调用，避免逻辑两份实现。
//!
//! 统一用 `Result<T, String>`：Tauri command 直接透传，MCP 工具把 `Err(String)`
//! 映射成对用户可见的 tool-level error。

use rusqlite::{params, Row};
use serde::{Deserialize, Serialize};
use tauri::AppHandle;

use crate::browser_supervisor::{BrowserSupervisor, RunningBrowser};
use crate::db::{new_id, now_ts, DbState};
use crate::paths;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Browser {
    pub id: String,
    pub name: String,
    pub proxy_url: String,
    pub timezone: String,
    pub language: String,
    pub fingerprint_id: String,
    pub notes: String,
    pub user_data_dir: String,
    pub last_opened_at: Option<i64>,
    pub created_at: i64,
    pub updated_at: i64,
    /// 当前活跃的远程调试端口；0 表示当前未在跑 / 未启用 CDP。
    #[serde(default)]
    pub cdp_port: u16,
}

#[derive(Debug, Deserialize)]
pub struct CreateBrowserInput {
    pub name: String,
    pub proxy_url: String,
    pub timezone: String,
    pub language: String,
    pub fingerprint_id: String,
    #[serde(default)]
    pub notes: String,
}

/// 刻意不允许更新 `fingerprint_id`：udd 目录按 fingerprint 共享
/// （`browser_data/<fp>/`），改 fp 会让 `user_data_dir` 与实际指纹脱钩，
/// 删除时的引用计数也会算错，留下孤儿目录。要换指纹请新建 browser。
#[derive(Debug, Deserialize)]
pub struct UpdateBrowserInput {
    pub name: Option<String>,
    pub proxy_url: Option<String>,
    pub timezone: Option<String>,
    pub language: Option<String>,
    pub notes: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct OpenBrowserInput {
    #[serde(default)]
    pub open_urls: Vec<String>,
}

pub const BROWSER_COLS: &str = "id, name, proxy_url, timezone, language, fingerprint_id, notes, \
                                user_data_dir, last_opened_at, created_at, updated_at, cdp_port";

fn map_row(row: &Row) -> rusqlite::Result<Browser> {
    let cdp_port: i64 = row.get::<_, i64>("cdp_port").unwrap_or(0);
    Ok(Browser {
        id: row.get("id")?,
        name: row.get("name")?,
        proxy_url: row.get("proxy_url")?,
        timezone: row.get("timezone")?,
        language: row.get("language")?,
        fingerprint_id: row.get("fingerprint_id")?,
        notes: row.get("notes")?,
        user_data_dir: row.get("user_data_dir")?,
        last_opened_at: row.get("last_opened_at")?,
        created_at: row.get("created_at")?,
        updated_at: row.get("updated_at")?,
        cdp_port: cdp_port.clamp(0, 65535) as u16,
    })
}

pub fn list_browsers(db: &DbState) -> Result<Vec<Browser>, String> {
    let conn = db.pool.get().map_err(|e| e.to_string())?;
    let sql = format!("SELECT {BROWSER_COLS} FROM browsers ORDER BY created_at DESC");
    let mut stmt = conn.prepare(&sql).map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], map_row)
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    Ok(rows)
}

pub fn get_browser(db: &DbState, id: &str) -> Result<Browser, String> {
    let conn = db.pool.get().map_err(|e| e.to_string())?;
    let sql = format!("SELECT {BROWSER_COLS} FROM browsers WHERE id = ?1");
    conn.query_row(&sql, params![id], map_row)
        .map_err(|e| e.to_string())
}

pub fn create_browser(
    app: &AppHandle,
    db: &DbState,
    input: CreateBrowserInput,
) -> Result<Browser, String> {
    if input.fingerprint_id.trim().is_empty() {
        return Err("fingerprint_id is required".to_string());
    }
    let id = new_id();
    let now = now_ts();

    // udd 按 fingerprint 共享：同一指纹的所有 browser 行共用一个目录。
    let udd = paths::user_data_dir_for_fingerprint(app, &input.fingerprint_id)
        .map_err(|e| e.to_string())?;
    std::fs::create_dir_all(&udd).map_err(|e| e.to_string())?;
    let udd_str = udd.to_string_lossy().to_string();

    let conn = db.pool.get().map_err(|e| e.to_string())?;
    conn.execute(
        "INSERT INTO browsers (id, name, proxy_url, timezone, language, fingerprint_id,
                               notes, user_data_dir, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
        params![
            id,
            input.name,
            input.proxy_url,
            input.timezone,
            input.language,
            input.fingerprint_id,
            input.notes,
            udd_str,
            now,
            now,
        ],
    )
    .map_err(|e| {
        // INSERT 失败时不留孤儿目录，但 udd 按 fp 共享，只有确认无任何行引用时才删。
        let still_used: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM browsers WHERE fingerprint_id = ?1",
                params![input.fingerprint_id],
                |r| r.get(0),
            )
            .unwrap_or(1);
        if still_used == 0 {
            let _ = std::fs::remove_dir_all(&udd);
        }
        e.to_string()
    })?;

    Ok(Browser {
        id,
        name: input.name,
        proxy_url: input.proxy_url,
        timezone: input.timezone,
        language: input.language,
        fingerprint_id: input.fingerprint_id,
        notes: input.notes,
        user_data_dir: udd_str,
        last_opened_at: None,
        created_at: now,
        updated_at: now,
        cdp_port: 0,
    })
}

pub fn update_browser(
    db: &DbState,
    id: &str,
    input: UpdateBrowserInput,
) -> Result<Browser, String> {
    let conn = db.pool.get().map_err(|e| e.to_string())?;
    let now = now_ts();

    conn.execute(
        "UPDATE browsers
            SET name           = COALESCE(?2, name),
                proxy_url      = COALESCE(?3, proxy_url),
                timezone       = COALESCE(?4, timezone),
                language       = COALESCE(?5, language),
                notes          = COALESCE(?6, notes),
                updated_at     = ?7
          WHERE id = ?1",
        params![
            id,
            input.name,
            input.proxy_url,
            input.timezone,
            input.language,
            input.notes,
            now,
        ],
    )
    .map_err(|e| e.to_string())?;

    get_browser(db, id)
}

pub fn delete_browser(
    db: &DbState,
    supervisor: &BrowserSupervisor,
    id: &str,
) -> Result<(), String> {
    if supervisor.is_running(id) {
        return Err("browser is currently running, close it first".to_string());
    }
    let conn = db.pool.get().map_err(|e| e.to_string())?;
    let row: Option<(String, String)> = conn
        .query_row(
            "SELECT user_data_dir, fingerprint_id FROM browsers WHERE id = ?1",
            params![id],
            |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)),
        )
        .ok();

    conn.execute("DELETE FROM browsers WHERE id = ?1", params![id])
        .map_err(|e| e.to_string())?;

    // 引用计数：仅当没有任何 browser 行还引用该 fingerprint 时，才删共享 udd。
    if let Some((dir, fp)) = row {
        if !dir.trim().is_empty() && !fp.trim().is_empty() {
            let still_used: i64 = conn
                .query_row(
                    "SELECT COUNT(*) FROM browsers WHERE fingerprint_id = ?1",
                    params![fp],
                    |r| r.get(0),
                )
                .unwrap_or(0);
            if still_used == 0 {
                if let Err(e) = std::fs::remove_dir_all(&dir) {
                    tracing::warn!(
                        "delete_browser: remove udd {dir} failed: {e}; will be swept on next launch"
                    );
                }
            }
        }
    }
    Ok(())
}

/// 组装 `launch-browser` 参数并调用 supervisor 启动。返回初始运行快照
/// （此时 CDP 端口通常尚未就绪，调用方如需等待就绪应自行轮询）。
pub fn open_browser(
    app: &AppHandle,
    db: &DbState,
    supervisor: &BrowserSupervisor,
    id: &str,
    input: Option<OpenBrowserInput>,
) -> Result<RunningBrowser, String> {
    if supervisor.is_running(id) {
        return Err("browser is already running".to_string());
    }

    let browser = get_browser(db, id)?;

    let chromium_bin = paths::bundled_chromium_bin(app)
        .map_err(|e| e.to_string())?
        .to_string_lossy()
        .to_string();

    let mut args: Vec<String> = vec![
        "launch-browser".into(),
        "--fingerprint-id".into(),
        browser.fingerprint_id.clone(),
        "--chromium-bin".into(),
        chromium_bin,
        "--user-data-dir".into(),
        browser.user_data_dir.clone(),
        "--language".into(),
        browser.language.clone(),
    ];
    if !browser.proxy_url.is_empty() {
        args.push("--proxy-url".into());
        args.push(browser.proxy_url.clone());
    }
    if !browser.timezone.is_empty() {
        args.push("--timezone".into());
        args.push(browser.timezone.clone());
    }
    if let Some(input) = input.as_ref() {
        for url in &input.open_urls {
            if !url.is_empty() {
                args.push("--open-url".into());
                args.push(url.clone());
            }
        }
    }

    let snapshot = supervisor
        .start(app, id.to_string(), browser.fingerprint_id.clone(), args)
        .map_err(|e| e.to_string())?;

    // 更新 last_opened_at（容忍失败，不阻断启动）
    if let Ok(conn) = db.pool.get() {
        let _ = conn.execute(
            "UPDATE browsers SET last_opened_at = ?2, updated_at = ?2 WHERE id = ?1",
            params![id, now_ts()],
        );
    }

    Ok(snapshot)
}

pub async fn close_browser(supervisor: &BrowserSupervisor, id: &str) -> Result<(), String> {
    supervisor.stop(id).await.map_err(|e| e.to_string())
}

pub fn list_running(supervisor: &BrowserSupervisor) -> Vec<RunningBrowser> {
    supervisor.list_running()
}
