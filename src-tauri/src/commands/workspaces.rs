use std::{fs, path::Path};

use notify::{RecursiveMode, Watcher};
use tauri::{AppHandle, Emitter, Manager, State};

use crate::{
    auth,
    error::AppResult,
    git,
    state::{PersistentState, WorkspaceRuntime},
    types::{
        GitBaselinePreview, LocalStateSnapshot, WorkspaceChangedEvent, WorkspaceDescriptor,
        WorkspaceForgottenEvent, WorkspaceWatchErrorEvent,
    },
    workspace,
};

use super::support::{ensure_git_idle, mobile_workspace_path, with_workspace_lock};

#[tauri::command(async)]
pub fn get_local_state(state: State<'_, PersistentState>) -> LocalStateSnapshot {
    state.snapshot()
}

#[tauri::command(async)]
pub fn open_workspace(
    path: String,
    state: State<'_, PersistentState>,
    runtime: State<'_, WorkspaceRuntime>,
) -> AppResult<WorkspaceDescriptor> {
    with_workspace_lock(&runtime, &path, || workspace::open_workspace(&path, &state))
}

#[tauri::command(async)]
pub fn create_workspace(
    path: String,
    state: State<'_, PersistentState>,
    runtime: State<'_, WorkspaceRuntime>,
) -> AppResult<WorkspaceDescriptor> {
    with_workspace_lock(&runtime, &path, || {
        workspace::create_workspace(&path, &state)
    })
}

#[tauri::command(async)]
pub fn clone_git_workspace(
    remote_url: String,
    path: String,
    credential_id: Option<String>,
    state: State<'_, PersistentState>,
    runtime: State<'_, WorkspaceRuntime>,
) -> AppResult<WorkspaceDescriptor> {
    let credential = credential_id
        .as_deref()
        .map(auth::load_credential)
        .transpose()?;
    let descriptor = with_workspace_lock(&runtime, &path, || {
        workspace::clone_workspace(&remote_url, &path, credential, &state)
    })?;
    if let Some(credential_id) = credential_id {
        state.set_credential_ref(&git::repository_lock_key(&descriptor.root), &credential_id)?;
    }
    Ok(descriptor)
}

#[tauri::command(async)]
pub fn clone_mobile_git_workspace(
    remote_url: String,
    workspace_name: String,
    credential_id: Option<String>,
    app: AppHandle,
    state: State<'_, PersistentState>,
    runtime: State<'_, WorkspaceRuntime>,
) -> AppResult<WorkspaceDescriptor> {
    let path = mobile_workspace_path(&app, &workspace_name)?;
    let credential = credential_id
        .as_deref()
        .map(auth::load_credential)
        .transpose()?;
    let descriptor = with_workspace_lock(&runtime, &path.to_string_lossy(), || {
        workspace::clone_workspace(&remote_url, &path.to_string_lossy(), credential, &state)
    })?;
    if let Some(credential_id) = credential_id {
        state.set_credential_ref(&git::repository_lock_key(&descriptor.root), &credential_id)?;
    }
    Ok(descriptor)
}

#[tauri::command(async)]
pub fn create_mobile_workspace(
    workspace_name: String,
    app: AppHandle,
    state: State<'_, PersistentState>,
    runtime: State<'_, WorkspaceRuntime>,
) -> AppResult<WorkspaceDescriptor> {
    let path = mobile_workspace_path(&app, &workspace_name)?;
    with_workspace_lock(&runtime, &path.to_string_lossy(), || {
        workspace::create_workspace(&path.to_string_lossy(), &state)
    })
}

#[tauri::command(async)]
pub fn preview_workspace_git_baseline(
    root: String,
    runtime: State<'_, WorkspaceRuntime>,
) -> AppResult<GitBaselinePreview> {
    with_workspace_lock(&runtime, &root, || workspace::preview_git_baseline(&root))
}

#[tauri::command(async)]
pub fn enable_workspace_git(
    root: String,
    runtime: State<'_, WorkspaceRuntime>,
) -> AppResult<WorkspaceDescriptor> {
    with_workspace_lock(&runtime, &root, || workspace::enable_git(&root))
}

#[tauri::command(async)]
pub fn forget_workspace(
    root: String,
    app: AppHandle,
    state: State<'_, PersistentState>,
    runtime: State<'_, WorkspaceRuntime>,
) -> AppResult<()> {
    if app.webview_windows().len() > 1 {
        return Err(crate::error::AppError::Message(
            "Close the additional Marktree windows before removing a workspace.".to_owned(),
        ));
    }
    let event = with_workspace_lock(&runtime, &root, || {
        let descriptor = workspace::refresh_workspace(&root)?;
        let roots = if let Some(capability) = &descriptor.git {
            ensure_git_idle(&state, capability, None)?;
            capability
                .worktrees
                .iter()
                .map(|worktree| worktree.path.clone())
                .collect::<Vec<_>>()
        } else {
            vec![descriptor.root.clone()]
        };
        state.forget_workspace(&descriptor.root, &roots, &git::repository_lock_key(&root))?;
        Ok(WorkspaceForgottenEvent {
            workspace_id: descriptor.id,
            roots,
        })
    })?;
    runtime.forget_roots(&event.roots);
    let _ = app.emit("workspace-forgotten", event);
    Ok(())
}

#[tauri::command(async)]
pub fn refresh_workspace(
    root: String,
    runtime: State<'_, WorkspaceRuntime>,
) -> AppResult<WorkspaceDescriptor> {
    with_workspace_lock(&runtime, &root, || workspace::refresh_workspace(&root))
}

#[tauri::command(async)]
pub fn watch_workspace(
    root: String,
    app: AppHandle,
    runtime: State<'_, WorkspaceRuntime>,
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
                    "workspace-watch-error",
                    WorkspaceWatchErrorEvent {
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
            .filter(|path| {
                !path.components().any(|part| {
                    matches!(
                        part.as_os_str().to_string_lossy().as_ref(),
                        ".git" | ".marktree"
                    )
                })
            })
            .any(|path| path.strip_prefix(&watched_root).is_ok());
        if has_relevant_path {
            let _ = app.emit(
                "workspace-changed",
                WorkspaceChangedEvent {
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
