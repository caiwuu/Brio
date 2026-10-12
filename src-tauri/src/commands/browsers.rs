//! Browser CRUD 与运行时控制的 Tauri command 薄壳。
//!
//! 具体逻辑在 [`crate::browsers_core`]，这里只做 `State<>` 解包后转调，
//! 使同一份逻辑能同时服务前端 IPC 和内嵌 MCP server。

use tauri::{AppHandle, State};

use crate::browser_supervisor::{BrowserSupervisor, RunningBrowser};
use crate::browsers_core as core;
use crate::db::DbState;

pub use crate::browsers_core::{
    Browser, CreateBrowserInput, OpenBrowserInput, UpdateBrowserInput,
};

#[tauri::command]
pub fn list_browsers(db: State<'_, DbState>) -> Result<Vec<Browser>, String> {
    core::list_browsers(&db)
}

#[tauri::command]
pub fn get_browser(db: State<'_, DbState>, id: String) -> Result<Browser, String> {
    core::get_browser(&db, &id)
}

#[tauri::command]
pub fn create_browser(
    app: AppHandle,
    db: State<'_, DbState>,
    input: CreateBrowserInput,
) -> Result<Browser, String> {
    core::create_browser(&app, &db, input)
}

#[tauri::command]
pub fn update_browser(
    db: State<'_, DbState>,
    id: String,
    input: UpdateBrowserInput,
) -> Result<Browser, String> {
    core::update_browser(&db, &id, input)
}

#[tauri::command]
pub async fn open_browser(
    app: AppHandle,
    db: State<'_, DbState>,
    supervisor: State<'_, BrowserSupervisor>,
    id: String,
    input: Option<OpenBrowserInput>,
) -> Result<RunningBrowser, String> {
    core::open_browser(&app, &db, &supervisor, &id, input)
}

#[tauri::command]
pub async fn close_browser(
    supervisor: State<'_, BrowserSupervisor>,
    id: String,
) -> Result<(), String> {
    core::close_browser(&supervisor, &id).await
}

#[tauri::command]
pub fn list_running_browsers(supervisor: State<'_, BrowserSupervisor>) -> Vec<RunningBrowser> {
    core::list_running(&supervisor)
}

#[tauri::command]
pub fn delete_browser(
    db: State<'_, DbState>,
    supervisor: State<'_, BrowserSupervisor>,
    id: String,
) -> Result<(), String> {
    core::delete_browser(&db, &supervisor, &id)
}
