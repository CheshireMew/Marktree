use tauri::{AppHandle, Manager, WebviewWindow};

use crate::{
    error::AppResult,
    git,
    state::{PersistentState, WorkspaceRuntime},
    types::{
        BranchDescriptor, ConflictChoice, CreateWorktreeRequest, DiffMode, DiffResult,
        GitStatusSnapshot, PendingGitOperationSummary, SyncPlan, SyncResult, TextComparison,
        WorkspaceViewSnapshot, WorktreeDescriptor, WorktreeSearchRequest, WorktreeSearchResponse,
    },
    workspace_service::WorkspaceService,
};

use super::support::{
    ensure_git_idle_for_root, ensure_worktree_idle, run_blocking, with_sync_credential,
};

fn in_workspace<T>(
    app: &AppHandle,
    root: &str,
    operation: impl FnOnce(&PersistentState) -> AppResult<T>,
) -> AppResult<T> {
    let state = app.state::<PersistentState>();
    let runtime = app.state::<WorkspaceRuntime>();
    WorkspaceService::new(&state, &runtime).run(root, || operation(&state))
}

fn in_two_workspaces<T>(
    app: &AppHandle,
    left_root: &str,
    right_root: &str,
    operation: impl FnOnce() -> AppResult<T>,
) -> AppResult<T> {
    let state = app.state::<PersistentState>();
    let runtime = app.state::<WorkspaceRuntime>();
    WorkspaceService::new(&state, &runtime).run_two(left_root, right_root, operation)
}

fn view_from_status(
    app: &AppHandle,
    root: &str,
    status: GitStatusSnapshot,
) -> AppResult<WorkspaceViewSnapshot> {
    let state = app.state::<PersistentState>();
    let runtime = app.state::<WorkspaceRuntime>();
    WorkspaceService::new(&state, &runtime).workspace_view_from_status(root, Some(status))
}

#[tauri::command]
pub async fn create_branch(
    root: String,
    name: String,
    start_point: Option<String>,
    checkout: bool,
    app: AppHandle,
) -> AppResult<WorkspaceViewSnapshot> {
    run_blocking(move || {
        in_workspace(&app, &root, |state| {
            ensure_git_idle_for_root(state, &root, None)?;
            let status = git::create_branch(&root, &name, start_point.as_deref(), checkout)?;
            view_from_status(&app, &root, status)
        })
    })
    .await
}

#[tauri::command]
pub async fn checkout_branch(
    root: String,
    name: String,
    app: AppHandle,
) -> AppResult<WorkspaceViewSnapshot> {
    run_blocking(move || {
        in_workspace(&app, &root, |state| {
            ensure_git_idle_for_root(state, &root, None)?;
            let status = git::checkout_branch(&root, &name)?;
            view_from_status(&app, &root, status)
        })
    })
    .await
}

#[tauri::command]
pub async fn delete_branch(
    root: String,
    name: String,
    app: AppHandle,
) -> AppResult<Vec<BranchDescriptor>> {
    run_blocking(move || {
        in_workspace(&app, &root, |state| {
            ensure_git_idle_for_root(state, &root, None)?;
            git::delete_branch(&root, &name)
        })
    })
    .await
}

#[tauri::command]
pub async fn create_worktree(
    request: CreateWorktreeRequest,
    app: AppHandle,
) -> AppResult<WorktreeDescriptor> {
    let root = request.root.clone();
    run_blocking(move || {
        in_workspace(&app, &root, |state| {
            ensure_git_idle_for_root(state, &root, None)?;
            git::create_worktree(request)
        })
    })
    .await
}

#[tauri::command]
pub async fn search_worktrees(
    request: WorktreeSearchRequest,
    window: WebviewWindow,
) -> AppResult<WorktreeSearchResponse> {
    let client_id = window.label().to_owned();
    let app = window.app_handle().clone();
    run_blocking(move || {
        crate::paths::canonical_root(&request.root)?;
        let runtime = app.state::<WorkspaceRuntime>();
        let key = git::repository_lock_key(&request.root);
        let search = runtime.search_session(&key, &client_id);
        let result = git::search_worktrees(
            &request.root,
            &request.query,
            request.limit.min(crate::state::MAX_SEARCH_RESULTS),
            request.path_prefix.as_deref(),
            &request.file_kinds,
            request.modified_after_ms,
            || search.is_current(),
        );
        result
    })
    .await
}

#[tauri::command]
pub async fn stage_paths(
    root: String,
    paths: Vec<String>,
    app: AppHandle,
) -> AppResult<WorkspaceViewSnapshot> {
    run_blocking(move || {
        in_workspace(&app, &root, |state| {
            ensure_worktree_idle(state, &root)?;
            let status = git::stage_paths(&root, &paths)?;
            view_from_status(&app, &root, status)
        })
    })
    .await
}

#[tauri::command]
pub async fn stage_all(root: String, app: AppHandle) -> AppResult<WorkspaceViewSnapshot> {
    run_blocking(move || {
        in_workspace(&app, &root, |state| {
            ensure_worktree_idle(state, &root)?;
            let status = git::stage_all(&root)?;
            view_from_status(&app, &root, status)
        })
    })
    .await
}

#[tauri::command]
pub async fn unstage_paths(
    root: String,
    paths: Vec<String>,
    app: AppHandle,
) -> AppResult<WorkspaceViewSnapshot> {
    run_blocking(move || {
        in_workspace(&app, &root, |state| {
            ensure_worktree_idle(state, &root)?;
            let status = git::unstage_paths(&root, &paths)?;
            view_from_status(&app, &root, status)
        })
    })
    .await
}

#[tauri::command]
pub async fn commit(
    root: String,
    message: String,
    app: AppHandle,
) -> AppResult<WorkspaceViewSnapshot> {
    run_blocking(move || {
        in_workspace(&app, &root, |state| {
            ensure_worktree_idle(state, &root)?;
            git::commit(&root, &message)?;
            let status = git::repository_status(&root)?;
            view_from_status(&app, &root, status)
        })
    })
    .await
}

#[tauri::command]
pub async fn fetch(root: String, app: AppHandle) -> AppResult<()> {
    run_blocking(move || {
        let state = app.state::<PersistentState>();
        let credential = crate::auth::credential_for_workspace(&root, &state)?;
        in_workspace(&app, &root, |state| {
            ensure_git_idle_for_root(state, &root, None)?;
            git::fetch(&root, credential)
        })
    })
    .await
}

#[tauri::command]
pub async fn pull_rebase(root: String, app: AppHandle) -> AppResult<SyncResult> {
    run_blocking(move || {
        let state = app.state::<PersistentState>();
        with_sync_credential(&root, &state, |credential| {
            in_workspace(&app, &root, |state| {
                ensure_git_idle_for_root(state, &root, Some(&root))?;
                git::pull_rebase(&root, credential, state)
            })
        })
    })
    .await
}

#[tauri::command]
pub async fn push(root: String, app: AppHandle) -> AppResult<()> {
    run_blocking(move || {
        let state = app.state::<PersistentState>();
        let credential = crate::auth::credential_for_workspace(&root, &state)?;
        in_workspace(&app, &root, |state| {
            ensure_git_idle_for_root(state, &root, None)?;
            git::push(&root, credential)
        })
    })
    .await
}

#[tauri::command]
pub async fn git_diff(root: String, mode: DiffMode, app: AppHandle) -> AppResult<DiffResult> {
    run_blocking(move || in_workspace(&app, &root, |_| git::diff(&root, mode))).await
}

#[tauri::command]
pub async fn compare_worktrees(
    left_root: String,
    right_root: String,
    path: String,
    app: AppHandle,
) -> AppResult<TextComparison> {
    run_blocking(move || {
        in_two_workspaces(&app, &left_root, &right_root, || {
            git::compare_worktrees(&left_root, &right_root, &path)
        })
    })
    .await
}

#[tauri::command]
pub async fn sync_plan(root: String, app: AppHandle) -> AppResult<SyncPlan> {
    run_blocking(move || {
        let state = app.state::<PersistentState>();
        let runtime = app.state::<WorkspaceRuntime>();
        WorkspaceService::new(&state, &runtime).sync_plan(&root)
    })
    .await
}

#[tauri::command]
pub async fn sync_workspace_changes(root: String, app: AppHandle) -> AppResult<SyncResult> {
    run_blocking(move || {
        let state = app.state::<PersistentState>();
        let runtime = app.state::<WorkspaceRuntime>();
        with_sync_credential(&root, &state, |credential| {
            WorkspaceService::new(&state, &runtime).sync_with_credential(&root, credential)
        })
    })
    .await
}

#[tauri::command]
pub async fn resolve_conflict(
    root: String,
    path: String,
    recovery_id: String,
    choice: ConflictChoice,
    app: AppHandle,
) -> AppResult<()> {
    run_blocking(move || {
        in_workspace(&app, &root, |state| {
            git::resolve_conflict(&root, &path, &recovery_id, choice, state)
        })
    })
    .await
}

#[tauri::command]
pub async fn resolve_conflict_with_content(
    root: String,
    path: String,
    recovery_id: String,
    content: String,
    app: AppHandle,
) -> AppResult<()> {
    run_blocking(move || {
        in_workspace(&app, &root, |state| {
            git::resolve_conflict_with_content(&root, &path, &recovery_id, &content, state)
        })
    })
    .await
}

#[tauri::command]
pub async fn pending_git_operation(
    root: String,
    app: AppHandle,
) -> AppResult<Option<PendingGitOperationSummary>> {
    run_blocking(move || {
        Ok(
            git::pending_git_operation(&root, &app.state::<PersistentState>())?
                .as_ref()
                .map(PendingGitOperationSummary::from),
        )
    })
    .await
}

#[tauri::command]
pub async fn resume_git_operation(root: String, app: AppHandle) -> AppResult<SyncResult> {
    run_blocking(move || {
        let state = app.state::<PersistentState>();
        with_sync_credential(&root, &state, |credential| {
            in_workspace(&app, &root, |state| {
                git::resume_git_operation(&root, credential, state)
            })
        })
    })
    .await
}

#[tauri::command]
pub async fn abort_git_operation(root: String, app: AppHandle) -> AppResult<GitStatusSnapshot> {
    run_blocking(move || in_workspace(&app, &root, |state| git::abort_git_operation(&root, state)))
        .await
}
