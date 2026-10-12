//! 通用 Tauri 命令：路径、Chromium 路径等。

use serde::Serialize;
use tauri::AppHandle;

use crate::paths;

#[tauri::command]
pub fn default_chromium_path(app: AppHandle) -> Result<String, String> {
    paths::bundled_chromium_bin(&app)
        .map(|p| p.to_string_lossy().to_string())
        .map_err(|e| e.to_string())
}

/// 前端拿数据目录用。Settings 页的 "Reveal" 按钮、错误报告里贴路径都走这个。
#[derive(Debug, Serialize)]
pub struct AppPaths {
    /// 应用数据根目录（见 `paths::app_data_dir`）。
    pub data_dir: String,
    /// 浏览器 user-data-dir 的根（`<data>/browser_data`）。
    pub browser_data_dir: String,
}

#[tauri::command]
pub fn get_app_paths(app: AppHandle) -> Result<AppPaths, String> {
    let data = paths::app_data_dir(&app).map_err(|e| e.to_string())?;
    let browser = paths::browser_data_root(&app).map_err(|e| e.to_string())?;
    Ok(AppPaths {
        data_dir: data.to_string_lossy().to_string(),
        browser_data_dir: browser.to_string_lossy().to_string(),
    })
}
