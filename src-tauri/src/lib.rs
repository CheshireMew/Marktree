mod auth;
mod commands;
mod content_policy;
mod documents;
mod error;
mod file_version;
mod git;
mod paths;
mod state;
mod types;

include!(concat!(env!("OUT_DIR"), "/ipc_handler.rs"));

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    use tauri::Manager;

    let builder = tauri::Builder::default();
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
            app.manage(state::PersistentState::load(&app_data_dir)?);
            app.manage(state::RepositoryRuntime::default());
            Ok(())
        })
        .invoke_handler(marktree_invoke_handler!())
        .run(tauri::generate_context!())
        .expect("error while running Marktree");
}
