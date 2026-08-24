use tauri::{AppHandle, Manager, WebviewWindow};

use super::support::run_blocking;
use crate::{
    asset_upload::AssetUploadRuntime,
    documents,
    error::AppResult,
    paths::{canonical_root, resolve_existing_entry},
    state::{PersistentState, WorkspaceRuntime},
    types::{
        AssetUploadChunkRequest, AssetUploadTicket, AssetWriteResult, BeginAssetUploadRequest,
        DocumentContent, DocumentSearchRequest, DocumentSearchResponse,
        DuplicateWorkspaceEntryRequest, MoveWorkspaceEntryRequest, SaveDocumentRequest,
        SaveDocumentResult, SaveWorkspaceConfigRequest, TrashEntry, WorkspaceConfigSnapshot,
        WorkspaceEntriesPatch, WorkspaceEntryDuplicateResult, WorkspaceEntryMoveResult,
        WorkspaceFilePreview, WorkspaceViewSnapshot,
    },
    workspace_service::WorkspaceService,
};

#[tauri::command]
pub async fn workspace_view(root: String, app: AppHandle) -> AppResult<WorkspaceViewSnapshot> {
    run_blocking(move || {
        let state = app.state::<PersistentState>();
        let runtime = app.state::<WorkspaceRuntime>();
        WorkspaceService::new(&state, &runtime).workspace_view(&root)
    })
    .await
}

#[tauri::command]
pub async fn list_workspace_directories(root: String, app: AppHandle) -> AppResult<Vec<String>> {
    run_blocking(move || {
        let state = app.state::<PersistentState>();
        let runtime = app.state::<WorkspaceRuntime>();
        WorkspaceService::new(&state, &runtime).list_workspace_directories(&root)
    })
    .await
}

#[tauri::command]
pub async fn workspace_entries_patch(
    root: String,
    paths: Vec<String>,
    app: AppHandle,
) -> AppResult<WorkspaceEntriesPatch> {
    run_blocking(move || {
        let state = app.state::<PersistentState>();
        let runtime = app.state::<WorkspaceRuntime>();
        WorkspaceService::new(&state, &runtime).workspace_entries_patch(&root, &paths)
    })
    .await
}

#[tauri::command]
pub async fn cancel_searches(roots: Vec<String>, window: WebviewWindow) -> AppResult<()> {
    let client_id = window.label().to_owned();
    let runtime = window.state::<WorkspaceRuntime>();
    for root in roots {
        runtime.cancel_search(&crate::git::repository_lock_key(&root), &client_id);
    }
    Ok(())
}

#[tauri::command]
pub async fn read_document(
    root: String,
    path: String,
    app: AppHandle,
) -> AppResult<DocumentContent> {
    run_blocking(move || {
        let state = app.state::<PersistentState>();
        let runtime = app.state::<WorkspaceRuntime>();
        WorkspaceService::new(&state, &runtime).read_document(&root, &path)
    })
    .await
}

#[tauri::command]
pub async fn open_document(
    root: String,
    path: String,
    app: AppHandle,
) -> AppResult<DocumentContent> {
    run_blocking(move || {
        let state = app.state::<PersistentState>();
        let runtime = app.state::<WorkspaceRuntime>();
        WorkspaceService::new(&state, &runtime).open_document(&root, &path)
    })
    .await
}

#[tauri::command]
pub async fn read_workspace_preview(
    root: String,
    path: String,
    app: AppHandle,
) -> AppResult<WorkspaceFilePreview> {
    run_blocking(move || {
        let state = app.state::<PersistentState>();
        let runtime = app.state::<WorkspaceRuntime>();
        let preview =
            WorkspaceService::new(&state, &runtime).read_workspace_preview(&root, &path)?;
        app.asset_protocol_scope()
            .allow_file(&preview.resource_path)?;
        Ok(preview)
    })
    .await
}

#[tauri::command]
pub async fn save_document(
    request: SaveDocumentRequest,
    app: AppHandle,
) -> AppResult<SaveDocumentResult> {
    run_blocking(move || {
        let state = app.state::<PersistentState>();
        let runtime = app.state::<WorkspaceRuntime>();
        WorkspaceService::new(&state, &runtime).save_document(request)
    })
    .await
}

#[tauri::command]
pub async fn create_document(
    root: String,
    path: String,
    app: AppHandle,
) -> AppResult<DocumentContent> {
    run_blocking(move || {
        let state = app.state::<PersistentState>();
        let runtime = app.state::<WorkspaceRuntime>();
        WorkspaceService::new(&state, &runtime).create_document(&root, &path)
    })
    .await
}

#[tauri::command]
pub async fn read_workspace_config(
    root: String,
    app: AppHandle,
) -> AppResult<WorkspaceConfigSnapshot> {
    run_blocking(move || {
        let state = app.state::<PersistentState>();
        let runtime = app.state::<WorkspaceRuntime>();
        WorkspaceService::new(&state, &runtime).read_workspace_config(&root)
    })
    .await
}

#[tauri::command]
pub async fn save_workspace_config(
    request: SaveWorkspaceConfigRequest,
    app: AppHandle,
) -> AppResult<WorkspaceConfigSnapshot> {
    run_blocking(move || {
        let state = app.state::<PersistentState>();
        let runtime = app.state::<WorkspaceRuntime>();
        WorkspaceService::new(&state, &runtime).save_workspace_config(request)
    })
    .await
}

#[tauri::command]
pub async fn begin_asset_upload(
    request: BeginAssetUploadRequest,
    app: AppHandle,
) -> AppResult<AssetUploadTicket> {
    run_blocking(move || {
        let state = app.state::<PersistentState>();
        let runtime = app.state::<WorkspaceRuntime>();
        let document = WorkspaceService::new(&state, &runtime)
            .read_document(&request.root, &request.document_path)?;
        app.state::<AssetUploadRuntime>()
            .begin(request, document.sha256)
    })
    .await
}

#[tauri::command]
pub async fn append_asset_upload(
    request: AssetUploadChunkRequest,
    app: AppHandle,
) -> AppResult<()> {
    run_blocking(move || app.state::<AssetUploadRuntime>().append(request)).await
}

#[tauri::command]
pub async fn finish_asset_upload(upload_id: String, app: AppHandle) -> AppResult<AssetWriteResult> {
    run_blocking(move || {
        let uploads = app.state::<AssetUploadRuntime>();
        let upload = uploads.completed(&upload_id)?;
        let state = app.state::<PersistentState>();
        let runtime = app.state::<WorkspaceRuntime>();
        let result = WorkspaceService::new(&state, &runtime).write_asset(
            &upload.root,
            &upload.document_path,
            &upload.file_name,
            &upload.source_path,
            upload.assets_dir.as_deref(),
            &upload.document_sha256,
        )?;
        uploads.finish(&upload_id);
        Ok(result)
    })
    .await
}

#[tauri::command]
pub async fn abort_asset_upload(upload_id: String, app: AppHandle) -> AppResult<()> {
    run_blocking(move || {
        app.state::<AssetUploadRuntime>().abort(&upload_id);
        Ok(())
    })
    .await
}

#[tauri::command]
pub async fn search_documents(
    request: DocumentSearchRequest,
    window: WebviewWindow,
) -> AppResult<DocumentSearchResponse> {
    let client_id = window.label().to_owned();
    let app = window.app_handle().clone();
    run_blocking(move || {
        let state = app.state::<PersistentState>();
        let runtime = app.state::<WorkspaceRuntime>();
        WorkspaceService::new(&state, &runtime).search_documents_with_filters(request, &client_id)
    })
    .await
}

#[tauri::command]
pub async fn create_workspace_folder(
    root: String,
    path: String,
    app: AppHandle,
) -> AppResult<String> {
    run_blocking(move || {
        let state = app.state::<PersistentState>();
        let runtime = app.state::<WorkspaceRuntime>();
        WorkspaceService::new(&state, &runtime).create_folder(&root, &path)
    })
    .await
}

#[tauri::command]
pub async fn move_workspace_entry(
    request: MoveWorkspaceEntryRequest,
    app: AppHandle,
) -> AppResult<WorkspaceEntryMoveResult> {
    run_blocking(move || {
        let state = app.state::<PersistentState>();
        let runtime = app.state::<WorkspaceRuntime>();
        WorkspaceService::new(&state, &runtime).move_entry(
            &request.root,
            &request.source_path,
            &request.destination_path,
        )
    })
    .await
}

#[tauri::command]
pub async fn duplicate_workspace_entry(
    request: DuplicateWorkspaceEntryRequest,
    app: AppHandle,
) -> AppResult<WorkspaceEntryDuplicateResult> {
    run_blocking(move || {
        let state = app.state::<PersistentState>();
        let runtime = app.state::<WorkspaceRuntime>();
        WorkspaceService::new(&state, &runtime).duplicate_entry(
            &request.root,
            &request.source_path,
            &request.destination_path,
        )
    })
    .await
}

#[tauri::command]
pub async fn trash_workspace_entry(
    root: String,
    path: String,
    app: AppHandle,
) -> AppResult<Option<TrashEntry>> {
    run_blocking(move || {
        let app_data_dir = app.path().app_data_dir()?;
        let state = app.state::<PersistentState>();
        let runtime = app.state::<WorkspaceRuntime>();
        WorkspaceService::new(&state, &runtime).trash_entry(&root, &path, &app_data_dir)
    })
    .await
}

#[tauri::command]
pub async fn list_workspace_trash(app: AppHandle) -> AppResult<Vec<TrashEntry>> {
    run_blocking(move || documents::list_android_trash(&app.path().app_data_dir()?)).await
}

#[tauri::command]
pub async fn restore_workspace_trash(id: String, app: AppHandle) -> AppResult<TrashEntry> {
    run_blocking(move || {
        let app_data_dir = app.path().app_data_dir()?;
        let state = app.state::<PersistentState>();
        let runtime = app.state::<WorkspaceRuntime>();
        WorkspaceService::new(&state, &runtime).restore_trash(&app_data_dir, &id)
    })
    .await
}

#[tauri::command]
pub async fn empty_workspace_trash(app: AppHandle) -> AppResult<()> {
    run_blocking(move || documents::empty_android_trash(&app.path().app_data_dir()?)).await
}

#[tauri::command]
pub async fn open_workspace_file_with_system(
    root: String,
    path: String,
    app: AppHandle,
) -> AppResult<()> {
    run_blocking(move || {
        let state = app.state::<PersistentState>();
        let runtime = app.state::<WorkspaceRuntime>();
        WorkspaceService::new(&state, &runtime).run(&root, || {
            let root_path = canonical_root(&root)?;
            let relative = crate::paths::normalize_content_relative(&path)?;
            let file_path = resolve_existing_entry(&root_path, &relative)?;
            if !file_path.is_file() {
                return Err(crate::error::AppError::InvalidPath(path));
            }
            tauri_plugin_opener::open_path(file_path, None::<&str>)
                .map_err(|error| crate::error::AppError::Message(error.to_string()))
        })
    })
    .await
}
