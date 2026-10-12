//! 应用数据/资源路径解析。

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use rusqlite::params;
use tauri::{AppHandle, Manager};

use crate::db::DbState;

/// 应用数据根目录（不存在则创建）：
/// - macOS：`~/Library/Application Support/com.brio.desktop/`
/// - Windows：`%LOCALAPPDATA%\com.brio.desktop\`
pub fn app_data_dir(app: &AppHandle) -> Result<PathBuf> {
    let dir = resolve_app_data_dir(app)?;
    std::fs::create_dir_all(&dir).context("create app_data_dir")?;
    Ok(dir)
}

/// 用 local 而不是 roaming（`app_data_dir()` 在 Windows 上是 `%APPDATA%`）：
/// 浏览器 profile 动辄上 GB，进了漫游配置会在域环境下随登录同步。macOS 上两者是同一个目录。
fn resolve_app_data_dir(app: &AppHandle) -> Result<PathBuf> {
    app.path()
        .app_local_data_dir()
        .context("failed to resolve app_local_data_dir")
}

/// 浏览器 user-data-dir 根目录：`<app_data>/browser_data`
pub fn browser_data_root(app: &AppHandle) -> Result<PathBuf> {
    let p = app_data_dir(app)?.join("browser_data");
    std::fs::create_dir_all(&p)?;
    Ok(p)
}

/// 指定 fingerprint 的 user-data-dir：`<app_data>/browser_data/<fingerprint_id>`
///
/// 同一指纹共用同一磁盘目录：所有引用该 `fingerprint_id` 的 browser 行
/// 指向同一 `browser_data/<fp>/`。删除时由 caller 按引用计数决定是否回收目录。
pub fn user_data_dir_for_fingerprint(app: &AppHandle, fingerprint_id: &str) -> Result<PathBuf> {
    if fingerprint_id.trim().is_empty() {
        anyhow::bail!("fingerprint_id is required");
    }
    Ok(browser_data_root(app)?.join(fingerprint_id))
}

/// 启动时确保所有数据目录存在。
pub fn ensure_data_dirs(app: &AppHandle) -> Result<()> {
    browser_data_root(app)?;
    Ok(())
}

/// 清理旧版本功能（identity 提取 / web2api）留下的数据目录。
///
/// 这些功能已删除，目录不再被任何代码读写；不清理的话会作为死数据
/// 永远占着磁盘（identity 里还是 cookie 快照，留着反而是隐患）。
/// 每次启动幂等执行，目录不存在时是 no-op。
pub fn cleanup_legacy_data(app: &AppHandle) -> Result<()> {
    let root = app_data_dir(app)?;
    for name in ["identity", "downloads"] {
        let p = root.join(name);
        if !p.exists() {
            continue;
        }
        match std::fs::remove_dir_all(&p) {
            Ok(_) => tracing::info!("removed legacy data dir {}", p.display()),
            Err(e) => tracing::warn!("remove legacy dir {} failed: {e}", p.display()),
        }
    }
    Ok(())
}

/// 一次性：把旧 identifier 的数据目录迁到当前 identifier 目录。
///
/// identifier 历史：
/// - `com.identity-gen.desktop`（identity-gen 时期）
/// - `com.brio.app`（更名 brio 后的第一版，名字以 `.app` 结尾在 macOS 上容易和 .app bundle 混淆）
/// - `com.brio.desktop`（当前）
///
/// 行为：
/// - 新目录不存在 → 找第一个仍存在的旧目录 rename 过去
/// - 新目录已存在但旧目录还残留 → 仅 warn，留给用户手动处理（避免覆盖现有数据）
/// - 旧目录都不在 → no-op
///
/// 必须在 `ensure_data_dirs` 之前调用，否则新目录会被先创建，rename 会落空。
pub fn migrate_legacy_app_data_dir(app: &AppHandle) -> Result<()> {
    // 顺序：离当前最近的 legacy 在前，确保选中"最新一份"用户数据
    const LEGACY_IDENTIFIERS: &[&str] = &["com.brio.app", "com.identity-gen.desktop"];

    let new_dir = resolve_app_data_dir(app)?;
    let parent = match new_dir.parent() {
        Some(p) => p.to_path_buf(),
        None => return Ok(()),
    };

    if new_dir.exists() {
        for legacy in LEGACY_IDENTIFIERS {
            let old_dir = parent.join(legacy);
            if old_dir.exists() {
                tracing::warn!(
                    "legacy app data dir still present at {} but new dir {} exists; skipping migration",
                    old_dir.display(),
                    new_dir.display()
                );
            }
        }
        return Ok(());
    }

    for legacy in LEGACY_IDENTIFIERS {
        let old_dir = parent.join(legacy);
        if !old_dir.exists() {
            continue;
        }
        match std::fs::rename(&old_dir, &new_dir) {
            Ok(_) => {
                tracing::info!(
                    "migrated legacy app data dir: {} -> {}",
                    old_dir.display(),
                    new_dir.display()
                );
                return Ok(());
            }
            Err(e) => {
                tracing::warn!(
                    "rename {} -> {} failed: {e}; you may need to move it manually",
                    old_dir.display(),
                    new_dir.display()
                );
                return Ok(());
            }
        }
    }
    Ok(())
}

/// 一次性迁移：把每个 browser 行的 user_data_dir 从 `<browser_id>/` 规则
/// 收敛到 `<fingerprint_id>/`，并清理 `browser_data/` 下的孤儿目录。
///
/// 策略：
/// - 同一 fingerprint 的多行 row：保留先到者的旧 udd 作为权威（rename 到 fp 路径），
///   其余 row 直接指向同一 fp 路径，原磁盘目录视为冗余删除。
/// - 期望 `<fp>/` 已存在 → 不覆盖，仅把 row 指向它，旧 udd 删除。
/// - 旧 udd 不存在 → 仅更新 row 字段，等待下次 launch/restore 重建。
/// - 完成后扫描 `browser_data/` 子目录，凡名字不在 (browser ids ∪ fingerprint ids) 集合里的
///   一律删除（清理早期废弃布局残留）。
///
/// 该函数幂等：第二次运行时大多数 row 已经位于 `<fp>/`，跳过即可。
pub fn migrate_udd_to_fingerprint(app: &AppHandle, db: &DbState) -> Result<()> {
    let root = browser_data_root(app)?;
    let conn = db.pool.get().context("get db conn for udd migration")?;

    // (id, fingerprint_id, user_data_dir)
    let rows: Vec<(String, String, String)> = {
        let mut stmt = conn
            .prepare("SELECT id, fingerprint_id, user_data_dir FROM browsers")
            .context("prepare browsers select")?;
        let it = stmt
            .query_map([], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                ))
            })
            .context("query browsers rows")?;
        it.filter_map(|r| r.ok()).collect()
    };

    let mut keep: HashSet<String> = HashSet::new();
    let mut migrated = 0usize;

    for (id, fp, current_udd) in rows.iter() {
        if fp.trim().is_empty() {
            // 防御：旧脏数据，跳过
            if !current_udd.trim().is_empty() {
                if let Some(name) = PathBuf::from(current_udd)
                    .file_name()
                    .and_then(|s| s.to_str())
                {
                    keep.insert(name.to_string());
                }
            }
            continue;
        }
        let target = root.join(fp);
        let target_str = target.to_string_lossy().to_string();
        keep.insert(fp.clone());

        if current_udd == &target_str {
            continue;
        }

        let cur_path = PathBuf::from(current_udd);
        if cur_path.exists() {
            if !target.exists() {
                if let Err(e) = std::fs::rename(&cur_path, &target) {
                    tracing::warn!(
                        "udd migrate: rename {} -> {} failed: {e}; falling back to copy",
                        cur_path.display(),
                        target.display()
                    );
                    // rename 失败（如跨设备），保守起见放弃，留给用户手动处理
                    continue;
                }
                migrated += 1;
            } else {
                // target 已存在（同 fp 的兄弟 row 先迁好了）；当前目录是冗余副本
                if let Err(e) = std::fs::remove_dir_all(&cur_path) {
                    tracing::warn!(
                        "udd migrate: remove redundant {} failed: {e}",
                        cur_path.display()
                    );
                }
            }
        }

        if let Err(e) = conn.execute(
            "UPDATE browsers SET user_data_dir = ?1, updated_at = strftime('%s','now') WHERE id = ?2",
            params![target_str, id],
        ) {
            tracing::warn!("udd migrate: update browsers row {id} failed: {e}");
        }
    }

    // 孤儿清理：扫描 root，凡名字既不是任何当前 fp、也不是任何 row.id 的目录全部删
    // （row.id 只是兜底，避免误删未迁移成功的旧目录；下一轮启动会再尝试）
    let row_ids: HashSet<String> = rows.iter().map(|(id, _, _)| id.clone()).collect();
    if let Ok(entries) = std::fs::read_dir(&root) {
        for ent in entries.flatten() {
            let name = ent.file_name().to_string_lossy().to_string();
            if name.starts_with('.') {
                continue;
            }
            if keep.contains(&name) || row_ids.contains(&name) {
                continue;
            }
            let p = ent.path();
            if p.is_dir() {
                if let Err(e) = std::fs::remove_dir_all(&p) {
                    tracing::warn!("udd migrate: remove orphan {} failed: {e}", p.display());
                } else {
                    tracing::info!("udd migrate: removed orphan {}", p.display());
                }
            }
        }
    }

    if migrated > 0 {
        tracing::info!("udd migrate: moved {migrated} dir(s) to fingerprint layout");
    }
    Ok(())
}

/// 解析内置 Chromium 可执行文件路径。
///
/// 优先级：
/// 1. 环境变量 `CHROMIUM_BIN`（绝对路径，方便本地调试或自定义）
/// 2. 打包态：
///    - macOS：`<app>.app/Contents/Resources/{resources/}chromium/Chromium.app/...`
///    - Windows：`<安装目录>\chromium\chrome.exe`（tauri.windows-release.conf.json 打进安装包）
/// 3. dev 态：`CARGO_MANIFEST_DIR/../brower/{mac_armx64/Chromium.app/...,win_x64/chrome.exe}`
///    —— `CARGO_MANIFEST_DIR` 在编译期固定为 `<project>/src-tauri`，
///    这样无论 `cargo run` 把可执行文件放到哪里都能找到工程内的 Chromium。
pub fn bundled_chromium_bin(app: &AppHandle) -> Result<PathBuf> {
    if let Ok(p) = std::env::var("CHROMIUM_BIN") {
        let pb = PathBuf::from(&p);
        if pb.exists() {
            return Ok(pb);
        }
        tracing::warn!("CHROMIUM_BIN={p} but file not found, falling back");
    }

    // resource_dir 仅在 bundle 态可用，dev 态可能失败，做 best-effort。
    let res_dir = app.path().resource_dir().ok();

    // 编译期烘焙的项目根目录，用于 dev fallback
    const PROJECT_DIR: &str = env!("CARGO_MANIFEST_DIR"); // <project>/src-tauri
    let project_root = PathBuf::from(PROJECT_DIR)
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_default();

    let candidates = chromium_candidates(res_dir.as_deref(), &project_root);
    for c in candidates.iter() {
        if c.exists() {
            return Ok(c.clone());
        }
    }
    anyhow::bail!(
        "chromium binary not found (dev: run scripts/fetch_chromium.py, or set CHROMIUM_BIN to override). tried: {:?}",
        candidates
            .iter()
            .map(|p| p.display().to_string())
            .collect::<Vec<_>>()
    )
}

#[cfg(target_os = "macos")]
fn chromium_candidates(res_dir: Option<&Path>, project_root: &Path) -> Vec<PathBuf> {
    let mut candidates: Vec<PathBuf> = Vec::new();
    if let Some(res) = res_dir {
        candidates.extend([
            // 打包态（含/不含 _up_/ 前缀）
            res.join("resources/chromium/Chromium.app/Contents/MacOS/Chromium"),
            res.join("chromium/Chromium.app/Contents/MacOS/Chromium"),
            // 兜底：旧的相对路径行为
            res.join("../../../brower/mac_armx64/Chromium.app/Contents/MacOS/Chromium"),
        ]);
    }
    // dev 态：直接走工程根的 brower/mac_armx64（始终存在的最可靠路径）
    candidates.push(project_root.join("brower/mac_armx64/Chromium.app/Contents/MacOS/Chromium"));
    candidates
}

#[cfg(target_os = "windows")]
fn chromium_candidates(res_dir: Option<&Path>, project_root: &Path) -> Vec<PathBuf> {
    let mut candidates: Vec<PathBuf> = Vec::new();
    if let Some(res) = res_dir {
        // 打包态：resource_dir 就是安装目录，bundle.resources 把 brower/win_x64/ 放到 chromium/
        candidates.push(res.join("chromium").join("chrome.exe"));
    }
    // dev 态：工程根的 brower/win_x64（scripts/fetch_chromium.py 下载到这里）
    candidates.push(
        project_root
            .join("brower")
            .join("win_x64")
            .join("chrome.exe"),
    );
    candidates
}

/// 上游 fingerprint-chromium 只发布 macOS / Windows 版，其它平台只能靠 `CHROMIUM_BIN`。
#[cfg(not(any(target_os = "macos", target_os = "windows")))]
fn chromium_candidates(_res_dir: Option<&Path>, _project_root: &Path) -> Vec<PathBuf> {
    Vec::new()
}
