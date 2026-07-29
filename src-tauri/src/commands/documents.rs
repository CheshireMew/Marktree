use tauri::State;

use crate::{
    documents,
    error::AppResult,
    git,
    state::{PersistentState, RepositoryRuntime},
    types::{
        AssetPreview, AssetWriteResult, DocumentContent, DocumentDescriptor,
        RepositoryConfigSnapshot, SaveDocumentRequest, SaveDocumentResult,
        SaveRepositoryConfigRequest,
    },
};

use super::support::{ensure_writable_during_git_operation, with_repository_lock};

#[tauri::command(async)]
pub fn list_documents(
    root: String,
    statuses: Vec<crate::types::GitFileStatus>,
    runtime: State<'_, RepositoryRuntime>,
) -> AppResult<Vec<DocumentDescriptor>> {
    with_repository_lock(&runtime, &root, || {
        documents::list_documents(&root, &statuses)
    })
}

#[tauri::command(async)]
pub fn read_document(
    root: String,
    path: String,
    runtime: State<'_, RepositoryRuntime>,
) -> AppResult<DocumentContent> {
    with_repository_lock(&runtime, &root, || documents::read_document(&root, &path))
}

#[tauri::command(async)]
pub fn open_document(
    root: String,
    path: String,
    state: State<'_, PersistentState>,
    runtime: State<'_, RepositoryRuntime>,
) -> AppResult<DocumentContent> {
    with_repository_lock(&runtime, &root, || {
        documents::open_document(&root, &path, &state)
    })
}

#[tauri::command(async)]
pub fn read_asset(
    root: String,
    path: String,
    runtime: State<'_, RepositoryRuntime>,
) -> AppResult<AssetPreview> {
    with_repository_lock(&runtime, &root, || documents::read_asset(&root, &path))
}

#[tauri::command(async)]
pub fn save_document(
    request: SaveDocumentRequest,
    state: State<'_, PersistentState>,
    runtime: State<'_, RepositoryRuntime>,
) -> AppResult<SaveDocumentResult> {
    let root = request.root.clone();
    with_repository_lock(&runtime, &root, || {
        ensure_writable_during_git_operation(&state, &root)?;
        documents::save_document(request, &state)
    })
}

#[tauri::command(async)]
pub fn create_document(
    root: String,
    path: String,
    state: State<'_, PersistentState>,
    runtime: State<'_, RepositoryRuntime>,
) -> AppResult<DocumentContent> {
    with_repository_lock(&runtime, &root, || {
        ensure_writable_during_git_operation(&state, &root)?;
        documents::create_document(&root, &path, &state)
    })
}

#[tauri::command(async)]
pub fn read_repository_config(
    root: String,
    runtime: State<'_, RepositoryRuntime>,
) -> AppResult<RepositoryConfigSnapshot> {
    with_repository_lock(&runtime, &root, || documents::read_repository_config(&root))
}

#[tauri::command(async)]
pub fn save_repository_config(
    request: SaveRepositoryConfigRequest,
    state: State<'_, PersistentState>,
    runtime: State<'_, RepositoryRuntime>,
) -> AppResult<RepositoryConfigSnapshot> {
    let root = request.root.clone();
    with_repository_lock(&runtime, &root, || {
        ensure_writable_during_git_operation(&state, &root)?;
        documents::save_repository_config(request, &state)
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
    runtime: State<'_, RepositoryRuntime>,
) -> AppResult<AssetWriteResult> {
    with_repository_lock(&runtime, &root, || {
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
    runtime: State<'_, RepositoryRuntime>,
) -> AppResult<Vec<String>> {
    let key = git::repository_lock_key(&root);
    let generation = runtime.begin_search(&key);
    documents::search_documents(&root, &query, limit.min(500), || {
        runtime.is_search_current(&key, generation)
    })
}
