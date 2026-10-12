mod browser_supervisor;
mod browsers_core;
mod commands;
mod db;
mod mcp;
mod paths;
mod settings;
#[allow(dead_code)]
mod sidecar;

use browser_supervisor::BrowserSupervisor;
use mcp::McpServer;
use tauri::{Manager, RunEvent};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    let app = tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .manage(BrowserSupervisor::default())
        .manage(McpServer::default())
        .setup(|app| {
            // identifier 历史变迁（com.identity-gen.desktop → com.brio.app → com.brio.desktop），
            // 老用户目录在 migrate_legacy_app_data_dir 里被 rename 过来。
            // 必须先于 ensure_data_dirs，否则新目录会被先建出来，rename 失效。
            if let Err(e) = paths::migrate_legacy_app_data_dir(app.handle()) {
                tracing::warn!("legacy app data dir migration failed: {e:#}");
            }
            paths::ensure_data_dirs(app.handle())?;
            // 清掉已删功能（identity 提取 / web2api）遗留的数据目录
            if let Err(e) = paths::cleanup_legacy_data(app.handle()) {
                tracing::warn!("legacy data cleanup failed: {e:#}");
            }
            let db_state = db::init(app.handle())?;
            // 一次性迁移：udd 目录布局 <browser_id>/ → <fingerprint_id>/
            if let Err(e) = paths::migrate_udd_to_fingerprint(app.handle(), &db_state) {
                tracing::warn!("udd layout migration failed: {e:#}");
            }
            // 读取 MCP 配置（token 缺失会在此惰性生成并持久化）；启用则自动拉起 server。
            let mcp_cfg = settings::load_mcp_config(&db_state).ok();
            app.manage(db_state);
            if let Some(cfg) = mcp_cfg {
                if cfg.enabled {
                    let handle = app.handle().clone();
                    tauri::async_runtime::spawn(async move {
                        let server = handle.state::<McpServer>();
                        if let Err(e) =
                            server.start(handle.clone(), cfg.port, cfg.token.clone()).await
                        {
                            tracing::error!("auto-start mcp server failed: {e}");
                        }
                    });
                }
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::utils::default_chromium_path,
            commands::utils::get_app_paths,
            commands::sidecar_ops::detect_timezone,
            commands::sidecar_ops::derive_fingerprint_id,
            commands::browsers::list_browsers,
            commands::browsers::get_browser,
            commands::browsers::create_browser,
            commands::browsers::update_browser,
            commands::browsers::delete_browser,
            commands::browsers::open_browser,
            commands::browsers::close_browser,
            commands::browsers::list_running_browsers,
            commands::mcp_settings::get_mcp_settings,
            commands::mcp_settings::set_mcp_settings,
            commands::mcp_settings::regenerate_mcp_token,
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application");

    app.run(|app_handle, event| {
        if let RunEvent::ExitRequested { .. } = event {
            // 先停 MCP server（避免退出过程中还有工具调用去开浏览器），再兜底关所有浏览器。
            if let Some(server) = app_handle.try_state::<McpServer>() {
                tauri::async_runtime::block_on(server.stop());
            }
            if let Some(supervisor) = app_handle.try_state::<BrowserSupervisor>() {
                supervisor.shutdown_all_blocking();
            }
        }
    });
}
