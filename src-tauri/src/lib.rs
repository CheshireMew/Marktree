#![cfg_attr(target_env = "msvc", allow(linker_messages))]

mod android_bridge;
mod archive;
mod asset_upload;
mod auth;
mod cli;
mod commands;
mod content_policy;
mod documents;
mod error;
mod file_version;
mod git;
mod operation_log;
mod paths;
mod portability;
mod process_lock;
mod state;
mod transfer_cache;
mod types;
mod workspace;
mod workspace_guard;
mod workspace_operation;
mod workspace_service;

include!(concat!(env!("OUT_DIR"), "/ipc_handler.rs"));

pub fn run_cli() -> i32 {
    cli::run()
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    use tauri::Manager;

    let builder = tauri::Builder::default();
    #[cfg(target_os = "android")]
    let builder = builder.plugin(marktree_android_bridge::init());
    #[cfg(desktop)]
    let builder = builder.plugin(tauri_plugin_single_instance::init(|app, _, _| {
        if let Some(window) = app.get_webview_window("main") {
            let _ = window.unminimize();
            let _ = window.show();
            let _ = window.set_focus();
        }
    }));

    builder
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_os::init())
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let app_data_dir = app.path().app_data_dir()?;
            let app_cache_dir = app.path().app_cache_dir()?;
            transfer_cache::prepare(&app_cache_dir)?;
            archive::cleanup_stale_import_staging(&app_data_dir)?;
            app.manage(asset_upload::AssetUploadRuntime::new(&app_cache_dir)?);
            let persistent_state = state::PersistentState::load(&app_data_dir)?;
            workspace_operation::recover_pending_operations(&persistent_state);
            app.manage(persistent_state);
            app.manage(state::WorkspaceRuntime::default());
            Ok(())
        })
        .invoke_handler(marktree_invoke_handler!())
        .run(tauri::generate_context!())
        .expect("error while running Marktree");
}
