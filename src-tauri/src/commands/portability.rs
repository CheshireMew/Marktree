use std::path::PathBuf;

use tauri::{AppHandle, Manager};

use super::support::run_blocking;
use crate::{
    android_bridge,
    error::AppResult,
    paths::portable_name_fragment,
    portability,
    state::{PersistentState, WorkspaceRuntime},
    transfer_cache,
    types::{
        AndroidShareImportResult, ImportAndroidShareRequest, PendingAndroidShare,
        WorkspaceArchiveExportResult,
    },
    workspace_service::WorkspaceService,
};

#[tauri::command]
pub async fn take_pending_android_share(app: AppHandle) -> AppResult<Option<PendingAndroidShare>> {
    run_blocking(move || {
        let Some(share) = android_bridge::take_pending_share(&app)? else {
            return Ok(None);
        };
        Ok(Some(portability::describe_incoming_share(share)))
    })
    .await
}

#[tauri::command]
pub async fn import_android_share(
    request: ImportAndroidShareRequest,
    app: AppHandle,
) -> AppResult<AndroidShareImportResult> {
    run_blocking(move || {
        let app_cache_dir = app.path().app_cache_dir()?;
        portability::import_incoming_share(
            request,
            &app.path().app_data_dir()?,
            &app_cache_dir,
            &app.state::<PersistentState>(),
            &app.state::<WorkspaceRuntime>(),
        )
    })
    .await
}

#[tauri::command]
pub async fn export_android_workspace_archive(
    root: String,
    app: AppHandle,
) -> AppResult<WorkspaceArchiveExportResult> {
    run_blocking(move || {
        let state = app.state::<PersistentState>();
        let runtime = app.state::<WorkspaceRuntime>();
        let workspace = WorkspaceService::new(&state, &runtime).inspect_workspace(&root)?;
        let app_cache_dir = app.path().app_cache_dir()?;
        let directory = transfer_cache::workspace_exports(&app_cache_dir);
        let output_path = unique_export_path(&directory, &workspace.name);
        let result = WorkspaceService::new(&state, &runtime)
            .export_workspace_archive(&root, &output_path)?;
        android_bridge::share_file(&app, &output_path, "application/zip", "Share workspace ZIP")?;
        Ok(result)
    })
    .await
}

fn unique_export_path(directory: &std::path::Path, workspace_name: &str) -> PathBuf {
    let name = portable_name_fragment(workspace_name);
    let name = if name.is_empty() { "workspace" } else { &name };
    let timestamp = chrono::Local::now().format("%Y%m%d-%H%M%S");
    for suffix in 1usize.. {
        let file_name = if suffix == 1 {
            format!("{name}-{timestamp}.zip")
        } else {
            format!("{name}-{timestamp}-{suffix}.zip")
        };
        let path = directory.join(file_name);
        if !path.exists() {
            return path;
        }
    }
    unreachable!()
}
