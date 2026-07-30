use tauri::{AppHandle, Manager, State};

use crate::{
    documents,
    error::AppResult,
    git,
    paths::{canonical_root, resolve_existing_entry},
    state::{PersistentState, WorkspaceRuntime},
    types::{
        AssetPreview, AssetWriteResult, DocumentContent, MoveWorkspaceEntryRequest,
        SaveDocumentRequest, SaveDocumentResult, SaveWorkspaceConfigRequest, TrashEntry,
        WorkspaceConfigSnapshot, WorkspaceEntry, WorkspaceEntryMoveResult,
    },
};

use super::support::{ensure_writable_during_git_operation, with_workspace_lock};

#[tauri::command(async)]
pub fn list_workspace_entries(
    root: String,
    runtime: State<'_, WorkspaceRuntime>,
) -> AppResult<Vec<WorkspaceEntry>> {
    with_workspace_lock(&runtime, &root, || {
        let statuses = if git::has_git_capability(&root) {
            git::repository_status(&root)?.files
        } else {
            Vec::new()
        };
        documents::list_workspace_entries(&root, &statuses)
    })
}

#[tauri::command(async)]
pub fn read_document(
    root: String,
    path: String,
    runtime: State<'_, WorkspaceRuntime>,
) -> AppResult<DocumentContent> {
    with_workspace_lock(&runtime, &root, || documents::read_document(&root, &path))
}

#[tauri::command(async)]
pub fn open_document(
    root: String,
    path: String,
    state: State<'_, PersistentState>,
    runtime: State<'_, WorkspaceRuntime>,
) -> AppResult<DocumentContent> {
    with_workspace_lock(&runtime, &root, || {
        documents::open_document(&root, &path, &state)
    })
}

#[tauri::command(async)]
pub fn read_asset(
    root: String,
    path: String,
    runtime: State<'_, WorkspaceRuntime>,
) -> AppResult<AssetPreview> {
    with_workspace_lock(&runtime, &root, || documents::read_asset(&root, &path))
}

#[tauri::command(async)]
pub fn save_document(
    request: SaveDocumentRequest,
    state: State<'_, PersistentState>,
    runtime: State<'_, WorkspaceRuntime>,
) -> AppResult<SaveDocumentResult> {
    let root = request.root.clone();
    with_workspace_lock(&runtime, &root, || {
        ensure_writable_during_git_operation(&state, &root)?;
        documents::save_document(request, &state)
    })
}

#[tauri::command(async)]
pub fn create_document(
    root: String,
    path: String,
    state: State<'_, PersistentState>,
    runtime: State<'_, WorkspaceRuntime>,
) -> AppResult<DocumentContent> {
    with_workspace_lock(&runtime, &root, || {
        ensure_writable_during_git_operation(&state, &root)?;
        documents::create_document(&root, &path, &state)
    })
}

#[tauri::command(async)]
pub fn read_workspace_config(
    root: String,
    runtime: State<'_, WorkspaceRuntime>,
) -> AppResult<WorkspaceConfigSnapshot> {
    with_workspace_lock(&runtime, &root, || documents::read_workspace_config(&root))
}

#[tauri::command(async)]
pub fn save_workspace_config(
    request: SaveWorkspaceConfigRequest,
    state: State<'_, PersistentState>,
    runtime: State<'_, WorkspaceRuntime>,
) -> AppResult<WorkspaceConfigSnapshot> {
    let root = request.root.clone();
    with_workspace_lock(&runtime, &root, || {
        ensure_writable_during_git_operation(&state, &root)?;
        documents::save_workspace_config(request, &state)
    })
}

#[tauri::command(async)]
pub fn write_asset(
    root: String,
    document_path: String,
    file_name: String,
    base64_data: String,
    assets_dir: Option<String>,
    state: State<'_, PersistentState>,
    runtime: State<'_, WorkspaceRuntime>,
) -> AppResult<AssetWriteResult> {
    with_workspace_lock(&runtime, &root, || {
        ensure_writable_during_git_operation(&state, &root)?;
        documents::write_asset(
            &root,
            &document_path,
            &file_name,
            &base64_data,
            assets_dir.as_deref(),
            &state,
        )
    })
}

#[tauri::command(async)]
pub fn search_documents(
    root: String,
    query: String,
    limit: usize,
    runtime: State<'_, WorkspaceRuntime>,
) -> AppResult<Vec<String>> {
    let key = git::repository_lock_key(&root);
    let generation = runtime.begin_search(&key);
    documents::search_documents(&root, &query, limit.min(500), || {
        runtime.is_search_current(&key, generation)
    })
}

#[tauri::command(async)]
pub fn create_workspace_folder(
    root: String,
    path: String,
    state: State<'_, PersistentState>,
    runtime: State<'_, WorkspaceRuntime>,
) -> AppResult<String> {
    with_workspace_lock(&runtime, &root, || {
        ensure_writable_during_git_operation(&state, &root)?;
        documents::create_folder(&root, &path)
    })
}

#[tauri::command(async)]
pub fn move_workspace_entry(
    request: MoveWorkspaceEntryRequest,
    state: State<'_, PersistentState>,
    runtime: State<'_, WorkspaceRuntime>,
) -> AppResult<WorkspaceEntryMoveResult> {
    let root = request.root.clone();
    with_workspace_lock(&runtime, &root, || {
        ensure_writable_during_git_operation(&state, &root)?;
        documents::move_entry(
            &root,
            &request.source_path,
            &request.destination_path,
            &state,
        )
    })
}

#[tauri::command(async)]
pub fn trash_workspace_entry(
    root: String,
    path: String,
    app: AppHandle,
    state: State<'_, PersistentState>,
    runtime: State<'_, WorkspaceRuntime>,
) -> AppResult<Option<TrashEntry>> {
    with_workspace_lock(&runtime, &root, || {
        ensure_writable_during_git_operation(&state, &root)?;
        documents::trash_entry(&root, &path, &app.path().app_data_dir()?, &state)
    })
}

#[tauri::command(async)]
pub fn list_workspace_trash(app: AppHandle) -> AppResult<Vec<TrashEntry>> {
    documents::list_android_trash(&app.path().app_data_dir()?)
}

#[tauri::command(async)]
pub fn restore_workspace_trash(
    id: String,
    app: AppHandle,
    state: State<'_, PersistentState>,
    runtime: State<'_, WorkspaceRuntime>,
) -> AppResult<TrashEntry> {
    let entries = documents::list_android_trash(&app.path().app_data_dir()?)?;
    let entry = entries
        .iter()
        .find(|entry| entry.id == id)
        .ok_or_else(|| crate::error::AppError::FileNotFound { path: id.clone() })?;
    let root = entry.workspace_root.clone();
    with_workspace_lock(&runtime, &root, || {
        ensure_writable_during_git_operation(&state, &root)?;
        documents::restore_android_trash(&app.path().app_data_dir()?, &id, &state)
    })
}

#[tauri::command(async)]
pub fn empty_workspace_trash(app: AppHandle) -> AppResult<()> {
    documents::empty_android_trash(&app.path().app_data_dir()?)
}

#[tauri::command(async)]
pub fn open_workspace_file_with_system(root: String, path: String) -> AppResult<()> {
    let root_path = canonical_root(&root)?;
    let file_path = resolve_existing_entry(&root_path, &path)?;
    if !file_path.is_file() {
        return Err(crate::error::AppError::InvalidPath(path));
    }
    tauri_plugin_opener::open_path(file_path, None::<&str>)
        .map_err(|error| crate::error::AppError::Message(error.to_string()))
}
