use std::{fs, path::Path, sync::Arc};

use notify::{RecursiveMode, Watcher};
use parking_lot::RwLock;
use tauri::{AppHandle, Emitter, Manager, WebviewWindow};

use crate::{
    content_policy::{is_observable_workspace_path, VERSIONED_WORKSPACE_CONFIG},
    documents,
    error::AppResult,
    git,
    state::{PersistentState, WorkspaceRuntime},
    types::{
        GitBaselinePreview, OperationLogEntry, StartupState, WorkspaceChangedEvent,
        WorkspaceDescriptor, WorkspaceForgottenEvent, WorkspaceRefreshSnapshot,
        WorkspaceWatchErrorEvent,
    },
    workspace,
    workspace_service::WorkspaceService,
};

use super::support::{ensure_git_idle, mobile_workspace_path, run_blocking};

#[tauri::command]
pub async fn get_startup_state(app: AppHandle) -> AppResult<StartupState> {
    run_blocking(move || app.state::<PersistentState>().startup_state()).await
}

#[tauri::command]
pub async fn read_operation_log(limit: usize, app: AppHandle) -> AppResult<Vec<OperationLogEntry>> {
    run_blocking(move || app.state::<PersistentState>().operation_log(limit)).await
}

#[tauri::command]
pub async fn open_workspace(path: String, app: AppHandle) -> AppResult<WorkspaceDescriptor> {
    run_blocking(move || {
        let state = app.state::<PersistentState>();
        let runtime = app.state::<WorkspaceRuntime>();
        WorkspaceService::new(&state, &runtime).open_workspace(&path)
    })
    .await
}

#[tauri::command]
pub async fn create_workspace(path: String, app: AppHandle) -> AppResult<WorkspaceDescriptor> {
    run_blocking(move || {
        let state = app.state::<PersistentState>();
        let runtime = app.state::<WorkspaceRuntime>();
        WorkspaceService::new(&state, &runtime).create_workspace(&path)
    })
    .await
}

#[tauri::command]
pub async fn clone_git_workspace(
    remote_url: String,
    path: String,
    credential_id: Option<String>,
    app: AppHandle,
) -> AppResult<WorkspaceDescriptor> {
    run_blocking(move || {
        let state = app.state::<PersistentState>();
        let runtime = app.state::<WorkspaceRuntime>();
        WorkspaceService::new(&state, &runtime).clone_workspace(
            &remote_url,
            &path,
            credential_id.as_deref(),
        )
    })
    .await
}

#[tauri::command]
pub async fn clone_mobile_git_workspace(
    remote_url: String,
    workspace_name: String,
    credential_id: Option<String>,
    app: AppHandle,
) -> AppResult<WorkspaceDescriptor> {
    run_blocking(move || {
        let path = mobile_workspace_path(&app, &workspace_name)?;
        let state = app.state::<PersistentState>();
        let runtime = app.state::<WorkspaceRuntime>();
        WorkspaceService::new(&state, &runtime).clone_workspace(
            &remote_url,
            &path.to_string_lossy(),
            credential_id.as_deref(),
        )
    })
    .await
}

#[tauri::command]
pub async fn create_mobile_workspace(
    workspace_name: String,
    app: AppHandle,
) -> AppResult<WorkspaceDescriptor> {
    run_blocking(move || {
        let path = mobile_workspace_path(&app, &workspace_name)?;
        let state = app.state::<PersistentState>();
        let runtime = app.state::<WorkspaceRuntime>();
        WorkspaceService::new(&state, &runtime).create_workspace(&path.to_string_lossy())
    })
    .await
}

#[tauri::command]
pub async fn preview_workspace_git_baseline(
    root: String,
    app: AppHandle,
) -> AppResult<GitBaselinePreview> {
    run_blocking(move || {
        let state = app.state::<PersistentState>();
        let runtime = app.state::<WorkspaceRuntime>();
        WorkspaceService::new(&state, &runtime).preview_git_baseline(&root)
    })
    .await
}

#[tauri::command]
pub async fn enable_workspace_git(root: String, app: AppHandle) -> AppResult<WorkspaceDescriptor> {
    run_blocking(move || {
        let state = app.state::<PersistentState>();
        let runtime = app.state::<WorkspaceRuntime>();
        WorkspaceService::new(&state, &runtime).enable_git(&root)
    })
    .await
}

#[tauri::command]
pub async fn forget_workspace(root: String, app: AppHandle) -> AppResult<()> {
    run_blocking(move || {
        if app.webview_windows().len() > 1 {
            return Err(crate::error::AppError::Message(
                "Close the additional Marktree windows before removing a workspace.".to_owned(),
            ));
        }
        let state = app.state::<PersistentState>();
        let runtime = app.state::<WorkspaceRuntime>();
        let repository_key = git::repository_lock_key(&root);
        let event = WorkspaceService::new(&state, &runtime).run(&root, || {
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
        runtime.forget_roots(&event.roots, &repository_key);
        let _ = app.emit("workspace-forgotten", event);
        Ok(())
    })
    .await
}

#[tauri::command]
pub async fn refresh_workspace_view(
    workspace_root: String,
    content_root: String,
    app: AppHandle,
) -> AppResult<WorkspaceRefreshSnapshot> {
    run_blocking(move || {
        let state = app.state::<PersistentState>();
        let runtime = app.state::<WorkspaceRuntime>();
        WorkspaceService::new(&state, &runtime)
            .refresh_workspace_view(&workspace_root, &content_root)
    })
    .await
}

#[tauri::command]
pub async fn watch_workspace(root: String, window: WebviewWindow) -> AppResult<()> {
    run_blocking(move || {
        let app = window.app_handle().clone();
        let state = app.state::<PersistentState>();
        let runtime = app.state::<WorkspaceRuntime>();
        WorkspaceService::new(&state, &runtime).run(&root, || Ok(()))?;
        let canonical = fs::canonicalize(&root)?;
        let canonical_string = canonical.to_string_lossy().into_owned();
        let client_id = window.label().to_owned();
        if runtime.has_watcher(&canonical_string, &client_id) {
            return Ok(());
        }
        let event_root = canonical_string.clone();
        let watched_root = canonical.clone();
        let ignore_set = Arc::new(RwLock::new(
            documents::read_workspace_config(&event_root)
                .and_then(|snapshot| documents::build_ignore_set(&snapshot.config.ignore_rules))
                .unwrap_or_else(|_| documents::build_ignore_set(&[]).expect("empty ignore rules")),
        ));
        let event_ignore_set = Arc::clone(&ignore_set);
        let mut watcher =
            notify::recommended_watcher(move |result: notify::Result<notify::Event>| {
                let event = match result {
                    Ok(event) => event,
                    Err(error) => {
                        let _ = window.emit(
                            "workspace-watch-error",
                            WorkspaceWatchErrorEvent {
                                root: event_root.clone(),
                                message: error.to_string(),
                            },
                        );
                        return;
                    }
                };
                let mut paths = event
                    .paths
                    .into_iter()
                    .filter_map(|path| {
                        let relative = path
                            .strip_prefix(&watched_root)
                            .ok()
                            .map(crate::paths::path_to_slashes)?;
                        if relative == VERSIONED_WORKSPACE_CONFIG {
                            if let Ok(updated) = documents::read_workspace_config(&event_root)
                                .and_then(|snapshot| {
                                    documents::build_ignore_set(&snapshot.config.ignore_rules)
                                })
                            {
                                *event_ignore_set.write() = updated;
                            }
                            return Some(relative);
                        }
                        is_observable_workspace_path(
                            &relative,
                            path.is_dir(),
                            &event_ignore_set.read(),
                        )
                        .then_some(relative)
                    })
                    .collect::<Vec<_>>();
                paths.sort();
                paths.dedup();
                if !paths.is_empty() {
                    let _ = window.emit(
                        "workspace-changed",
                        WorkspaceChangedEvent {
                            root: event_root.clone(),
                            paths,
                        },
                    );
                }
            })
            .map_err(|error| crate::error::AppError::Watch(error.to_string()))?;
        watcher
            .watch(Path::new(&canonical_string), RecursiveMode::Recursive)
            .map_err(|error| crate::error::AppError::Watch(error.to_string()))?;
        runtime.store_watcher(&canonical_string, &client_id, watcher);
        Ok(())
    })
    .await
}

#[tauri::command]
pub async fn unwatch_workspace(root: String, window: WebviewWindow) -> AppResult<()> {
    run_blocking(move || {
        let canonical = fs::canonicalize(&root)
            .map(|path| path.to_string_lossy().into_owned())
            .unwrap_or(root);
        let runtime = window.state::<WorkspaceRuntime>();
        runtime.remove_watcher(&canonical, window.label());
        runtime.cancel_search(&git::repository_lock_key(&canonical), window.label());
        Ok(())
    })
    .await
}
