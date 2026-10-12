//! SQLite 连接池 + 迁移。
//!
//! 数据库文件位于 `app_data_dir/data.sqlite3`。所有迁移以单文件 SQL 形式
//! 内嵌进二进制（`include_str!`），按文件名顺序执行，记录在
//! `schema_migrations` 表中以保证幂等。

use std::path::PathBuf;

use anyhow::{Context, Result};
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use rusqlite::params;
use tauri::AppHandle;

use crate::paths;

pub type DbPool = Pool<SqliteConnectionManager>;

pub struct DbState {
    pub pool: DbPool,
}

/// 已知迁移列表（顺序很重要，新增时只追加在末尾）。
const MIGRATIONS: &[(&str, &str)] = &[
    ("0001_init.sql", include_str!("migrations/0001_init.sql")),
    (
        "0002_seed_providers.sql",
        include_str!("migrations/0002_seed_providers.sql"),
    ),
    (
        "0003_browsers_cdp.sql",
        include_str!("migrations/0003_browsers_cdp.sql"),
    ),
    (
        "0004_mcp_servers.sql",
        include_str!("migrations/0004_mcp_servers.sql"),
    ),
    (
        "0005_mcp_servers_transport.sql",
        include_str!("migrations/0005_mcp_servers_transport.sql"),
    ),
    (
        "0006_model_providers.sql",
        include_str!("migrations/0006_model_providers.sql"),
    ),
    (
        "0007_provider_redesign.sql",
        include_str!("migrations/0007_provider_redesign.sql"),
    ),
    (
        "0008_agents_model_provider.sql",
        include_str!("migrations/0008_agents_model_provider.sql"),
    ),
    (
        "0009_browsers_is_ephemeral.sql",
        include_str!("migrations/0009_browsers_is_ephemeral.sql"),
    ),
    (
        "0010_chat_v2.sql",
        include_str!("migrations/0010_chat_v2.sql"),
    ),
    (
        "0011_mcp_server_tools.sql",
        include_str!("migrations/0011_mcp_server_tools.sql"),
    ),
    (
        "0012_chat_fork.sql",
        include_str!("migrations/0012_chat_fork.sql"),
    ),
    (
        "0013_skills.sql",
        include_str!("migrations/0013_skills.sql"),
    ),
    (
        "0014_credentials.sql",
        include_str!("migrations/0014_credentials.sql"),
    ),
    (
        "0015_credentials_secret_plain.sql",
        include_str!("migrations/0015_credentials_secret_plain.sql"),
    ),
    (
        "0016_drop_proxies.sql",
        include_str!("migrations/0016_drop_proxies.sql"),
    ),
    (
        "0017_rename_identity_providers.sql",
        include_str!("migrations/0017_rename_identity_providers.sql"),
    ),
    (
        "0018_identity_freezes.sql",
        include_str!("migrations/0018_identity_freezes.sql"),
    ),
    (
        "0019_identity_multi_providers.sql",
        include_str!("migrations/0019_identity_multi_providers.sql"),
    ),
    (
        "0020_app_settings.sql",
        include_str!("migrations/0020_app_settings.sql"),
    ),
    (
        "0021_drop_agents.sql",
        include_str!("migrations/0021_drop_agents.sql"),
    ),
    (
        "0022_pure_fingerprint_browser.sql",
        include_str!("migrations/0022_pure_fingerprint_browser.sql"),
    ),
    (
        "0023_drop_proxy_credentials.sql",
        include_str!("migrations/0023_drop_proxy_credentials.sql"),
    ),
    (
        "0024_mcp_settings.sql",
        include_str!("migrations/0024_mcp_settings.sql"),
    ),
];

pub fn init(app: &AppHandle) -> Result<DbState> {
    let path = db_path(app)?;

    // 1. 先用一个临时连接打开 WAL（这会拿短暂的独占锁，必须在池外做完）。
    {
        let conn = rusqlite::Connection::open(&path).context("open sqlite for setup")?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
    }

    // 2. 再建池；with_init 只设置每个连接局部的 PRAGMA（foreign_keys 不会触发文件锁）。
    let manager = SqliteConnectionManager::file(&path).with_init(|c| {
        c.pragma_update(None, "foreign_keys", "ON")?;
        Ok(())
    });
    let pool = Pool::builder()
        .max_size(8)
        .build(manager)
        .context("build sqlite pool")?;

    {
        let conn = pool.get().context("get conn for migrations")?;
        let applied = run_migrations(&conn)?;
        // 迁移可能 DROP TABLE / 批量 DELETE；SQLite 不会自动收缩文件，
        // 只在真的执行过新迁移时 VACUUM 一次，把空间还给文件系统。
        if applied > 0 {
            if let Err(e) = conn.execute_batch("VACUUM") {
                tracing::warn!("post-migration VACUUM failed: {e}");
            } else {
                tracing::info!("post-migration VACUUM done ({applied} migration(s) applied)");
            }
        }
    }

    Ok(DbState { pool })
}

pub fn db_path(app: &AppHandle) -> Result<PathBuf> {
    let dir = paths::app_data_dir(app)?;
    Ok(dir.join("data.sqlite3"))
}

/// 返回本次实际执行的迁移条数。
fn run_migrations(conn: &rusqlite::Connection) -> Result<usize> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS schema_migrations (
            name TEXT PRIMARY KEY,
            applied_at INTEGER NOT NULL
        );",
    )?;

    let mut applied = 0usize;
    for (name, sql) in MIGRATIONS {
        let already: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM schema_migrations WHERE name = ?1",
                params![name],
                |r| r.get(0),
            )
            .unwrap_or(0);
        if already > 0 {
            continue;
        }
        tracing::info!("applying migration: {name}");
        conn.execute_batch(sql)
            .with_context(|| format!("apply migration {name}"))?;
        conn.execute(
            "INSERT INTO schema_migrations(name, applied_at) VALUES (?1, ?2)",
            params![name, now_ts()],
        )?;
        applied += 1;
    }
    Ok(applied)
}

pub fn now_ts() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

pub fn new_id() -> String {
    ulid::Ulid::new().to_string()
}
