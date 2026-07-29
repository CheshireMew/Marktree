use std::{fs, path::Path};

use notify::{RecursiveMode, Watcher};
use tauri::{AppHandle, Emitter, Manager, State};

use crate::{
    auth,
    error::AppResult,
    git,
    state::{PersistentState, RepositoryRuntime},
    types::{
        LocalStateSnapshot, RepositoryChangedEvent, RepositoryDescriptor, RepositoryForgottenEvent,
        RepositoryWatchErrorEvent,
    },
};

use super::support::{ensure_repository_idle, mobile_repository_path, with_repository_lock};

#[tauri::command(async)]
pub fn get_local_state(state: State<'_, PersistentState>) -> LocalStateSnapshot {
    state.snapshot()
}

#[tauri::command(async)]
pub fn open_repository(
    path: String,
    state: State<'_, PersistentState>,
    runtime: State<'_, RepositoryRuntime>,
) -> AppResult<RepositoryDescriptor> {
    with_repository_lock(&runtime, &path, || git::open_repository(&path, &state))
}

#[tauri::command(async)]
pub fn initialize_repository(
    path: String,
    state: State<'_, PersistentState>,
    runtime: State<'_, RepositoryRuntime>,
) -> AppResult<RepositoryDescriptor> {
    with_repository_lock(&runtime, &path, || {
        git::initialize_repository(&path, &state)
    })
}

#[tauri::command(async)]
pub fn clone_repository(
    remote_url: String,
    path: String,
    credential_id: Option<String>,
    state: State<'_, PersistentState>,
    runtime: State<'_, RepositoryRuntime>,
) -> AppResult<RepositoryDescriptor> {
    let credential = credential_id
        .as_deref()
        .map(auth::load_credential)
        .transpose()?;
    let descriptor = with_repository_lock(&runtime, &path, || {
        git::clone_repository(&remote_url, &path, credential, &state)
    })?;
    if let Some(credential_id) = credential_id {
        state.set_credential_ref(&git::repository_lock_key(&descriptor.root), &credential_id)?;
    }
    Ok(descriptor)
}

#[tauri::command(async)]
pub fn clone_mobile_repository(
    remote_url: String,
    repository_name: String,
    credential_id: Option<String>,
    app: AppHandle,
    state: State<'_, PersistentState>,
    runtime: State<'_, RepositoryRuntime>,
) -> AppResult<RepositoryDescriptor> {
    let path = mobile_repository_path(&app, &repository_name)?;
    let credential = credential_id
        .as_deref()
        .map(auth::load_credential)
        .transpose()?;
    let descriptor = with_repository_lock(&runtime, &path.to_string_lossy(), || {
        git::clone_repository(&remote_url, &path.to_string_lossy(), credential, &state)
    })?;
    if let Some(credential_id) = credential_id {
        state.set_credential_ref(&git::repository_lock_key(&descriptor.root), &credential_id)?;
    }
    Ok(descriptor)
}

#[tauri::command(async)]
pub fn initialize_mobile_repository(
    repository_name: String,
    app: AppHandle,
    state: State<'_, PersistentState>,
    runtime: State<'_, RepositoryRuntime>,
) -> AppResult<RepositoryDescriptor> {
    let path = mobile_repository_path(&app, &repository_name)?;
    with_repository_lock(&runtime, &path.to_string_lossy(), || {
        git::initialize_repository(&path.to_string_lossy(), &state)
    })
}

#[tauri::command(async)]
pub fn forget_repository(
    root: String,
    app: AppHandle,
    state: State<'_, PersistentState>,
    runtime: State<'_, RepositoryRuntime>,
) -> AppResult<()> {
    if app.webview_windows().len() > 1 {
        return Err(crate::error::AppError::Message(
            "Close the additional Marktree windows before removing a repository.".to_owned(),
        ));
    }
    let event = with_repository_lock(&runtime, &root, || {
        let descriptor = git::refresh_repository(&root)?;
        ensure_repository_idle(&state, &descriptor, None)?;
        let worktree_roots = descriptor
            .worktrees
            .iter()
            .map(|worktree| worktree.path.clone())
            .collect::<Vec<_>>();
        state.forget_repository(
            &descriptor.root,
            &worktree_roots,
            &git::repository_lock_key(&root),
        )?;
        Ok(RepositoryForgottenEvent {
            repository_id: descriptor.id,
            worktree_roots,
        })
    })?;
    runtime.forget_worktrees(&event.worktree_roots);
    let _ = app.emit("repository-forgotten", event);
    Ok(())
}

#[tauri::command(async)]
pub fn refresh_repository(
    root: String,
    runtime: State<'_, RepositoryRuntime>,
) -> AppResult<RepositoryDescriptor> {
    with_repository_lock(&runtime, &root, || git::refresh_repository(&root))
}

#[tauri::command(async)]
pub fn watch_repository(
    root: String,
    app: AppHandle,
    runtime: State<'_, RepositoryRuntime>,
) -> AppResult<()> {
    let canonical = fs::canonicalize(&root)?;
    let canonical_string = canonical.to_string_lossy().into_owned();
    if runtime.has_watcher(&canonical_string) {
        return Ok(());
    }
    let event_root = canonical_string.clone();
    let watched_root = canonical.clone();
    let mut watcher = notify::recommended_watcher(move |result: notify::Result<notify::Event>| {
        let event = match result {
            Ok(event) => event,
            Err(error) => {
                let _ = app.emit(
                    "repository-watch-error",
                    RepositoryWatchErrorEvent {
                        root: event_root.clone(),
                        message: error.to_string(),
                    },
                );
                return;
            }
        };
        let has_relevant_path = event
            .paths
            .into_iter()
            .filter(|path| !path.components().any(|part| part.as_os_str() == ".git"))
            .any(|path| path.strip_prefix(&watched_root).is_ok());
        if has_relevant_path {
            let _ = app.emit(
                "repository-changed",
                RepositoryChangedEvent {
                    root: event_root.clone(),
                },
            );
        }
    })
    .map_err(|error| crate::error::AppError::Watch(error.to_string()))?;
    watcher
        .watch(Path::new(&canonical_string), RecursiveMode::Recursive)
        .map_err(|error| crate::error::AppError::Watch(error.to_string()))?;
    runtime.store_watcher(&canonical_string, watcher);
    Ok(())
}
