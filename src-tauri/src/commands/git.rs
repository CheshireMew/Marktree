use tauri::State;

use crate::{
    error::AppResult,
    git,
    state::{PersistentState, WorkspaceRuntime},
    types::{
        BranchDescriptor, ConflictChoice, ConflictRecord, CreateWorktreeRequest, DiffMode,
        DiffResult, GitStatusSnapshot, PendingGitOperation, SyncPlan, SyncResult, TextComparison,
        WorktreeDescriptor, WorktreeSearchResult,
    },
};

use super::support::{
    credential_for_root, ensure_git_idle_for_root, ensure_worktree_idle, with_sync_credential,
    with_two_workspace_locks, with_workspace_lock,
};

#[tauri::command(async)]
pub fn workspace_git_status(
    root: String,
    runtime: State<'_, WorkspaceRuntime>,
) -> AppResult<GitStatusSnapshot> {
    with_workspace_lock(&runtime, &root, || git::repository_status(&root))
}

#[tauri::command(async)]
pub fn list_branches(
    root: String,
    runtime: State<'_, WorkspaceRuntime>,
) -> AppResult<Vec<BranchDescriptor>> {
    with_workspace_lock(&runtime, &root, || git::list_branches(&root))
}

#[tauri::command(async)]
pub fn create_branch(
    root: String,
    name: String,
    start_point: Option<String>,
    checkout: bool,
    state: State<'_, PersistentState>,
    runtime: State<'_, WorkspaceRuntime>,
) -> AppResult<GitStatusSnapshot> {
    with_workspace_lock(&runtime, &root, || {
        ensure_git_idle_for_root(&state, &root, None)?;
        git::create_branch(&root, &name, start_point.as_deref(), checkout)
    })
}

#[tauri::command(async)]
pub fn checkout_branch(
    root: String,
    name: String,
    state: State<'_, PersistentState>,
    runtime: State<'_, WorkspaceRuntime>,
) -> AppResult<GitStatusSnapshot> {
    with_workspace_lock(&runtime, &root, || {
        ensure_git_idle_for_root(&state, &root, None)?;
        git::checkout_branch(&root, &name)
    })
}

#[tauri::command(async)]
pub fn delete_branch(
    root: String,
    name: String,
    state: State<'_, PersistentState>,
    runtime: State<'_, WorkspaceRuntime>,
) -> AppResult<Vec<BranchDescriptor>> {
    with_workspace_lock(&runtime, &root, || {
        ensure_git_idle_for_root(&state, &root, None)?;
        git::delete_branch(&root, &name)
    })
}

#[tauri::command(async)]
pub fn create_worktree(
    request: CreateWorktreeRequest,
    state: State<'_, PersistentState>,
    runtime: State<'_, WorkspaceRuntime>,
) -> AppResult<WorktreeDescriptor> {
    let root = request.root.clone();
    with_workspace_lock(&runtime, &root, || {
        ensure_git_idle_for_root(&state, &root, None)?;
        git::create_worktree(request)
    })
}

#[tauri::command(async)]
pub fn search_worktrees(
    root: String,
    query: String,
    limit: usize,
    runtime: State<'_, WorkspaceRuntime>,
) -> AppResult<Vec<WorktreeSearchResult>> {
    let key = git::repository_lock_key(&root);
    let generation = runtime.begin_search(&key);
    git::search_worktrees(&root, &query, limit.min(500), || {
        runtime.is_search_current(&key, generation)
    })
}

#[tauri::command(async)]
pub fn stage_paths(
    root: String,
    paths: Vec<String>,
    state: State<'_, PersistentState>,
    runtime: State<'_, WorkspaceRuntime>,
) -> AppResult<GitStatusSnapshot> {
    with_workspace_lock(&runtime, &root, || {
        ensure_worktree_idle(&state, &root)?;
        git::stage_paths(&root, &paths)
    })
}

#[tauri::command(async)]
pub fn stage_all(
    root: String,
    state: State<'_, PersistentState>,
    runtime: State<'_, WorkspaceRuntime>,
) -> AppResult<GitStatusSnapshot> {
    with_workspace_lock(&runtime, &root, || {
        ensure_worktree_idle(&state, &root)?;
        git::stage_all(&root)
    })
}

#[tauri::command(async)]
pub fn unstage_paths(
    root: String,
    paths: Vec<String>,
    state: State<'_, PersistentState>,
    runtime: State<'_, WorkspaceRuntime>,
) -> AppResult<GitStatusSnapshot> {
    with_workspace_lock(&runtime, &root, || {
        ensure_worktree_idle(&state, &root)?;
        git::unstage_paths(&root, &paths)
    })
}

#[tauri::command(async)]
pub fn commit(
    root: String,
    message: String,
    state: State<'_, PersistentState>,
    runtime: State<'_, WorkspaceRuntime>,
) -> AppResult<String> {
    with_workspace_lock(&runtime, &root, || {
        ensure_worktree_idle(&state, &root)?;
        git::commit(&root, &message)
    })
}

#[tauri::command(async)]
pub fn fetch(
    root: String,
    state: State<'_, PersistentState>,
    runtime: State<'_, WorkspaceRuntime>,
) -> AppResult<GitStatusSnapshot> {
    let credential = credential_for_root(&root, &state)?;
    with_workspace_lock(&runtime, &root, || {
        ensure_git_idle_for_root(&state, &root, None)?;
        git::fetch(&root, credential)
    })
}

#[tauri::command(async)]
pub fn pull_rebase(
    root: String,
    state: State<'_, PersistentState>,
    runtime: State<'_, WorkspaceRuntime>,
) -> AppResult<SyncResult> {
    with_sync_credential(&root, &state, |credential| {
        with_workspace_lock(&runtime, &root, || {
            ensure_git_idle_for_root(&state, &root, Some(&root))?;
            git::pull_rebase(&root, credential, &state)
        })
    })
}

#[tauri::command(async)]
pub fn push(
    root: String,
    state: State<'_, PersistentState>,
    runtime: State<'_, WorkspaceRuntime>,
) -> AppResult<GitStatusSnapshot> {
    let credential = credential_for_root(&root, &state)?;
    with_workspace_lock(&runtime, &root, || {
        ensure_git_idle_for_root(&state, &root, None)?;
        git::push(&root, credential)
    })
}

#[tauri::command(async)]
pub fn git_diff(
    root: String,
    mode: DiffMode,
    runtime: State<'_, WorkspaceRuntime>,
) -> AppResult<DiffResult> {
    with_workspace_lock(&runtime, &root, || git::diff(&root, mode))
}

#[tauri::command(async)]
pub fn compare_worktrees(
    left_root: String,
    right_root: String,
    path: String,
    runtime: State<'_, WorkspaceRuntime>,
) -> AppResult<TextComparison> {
    with_two_workspace_locks(&runtime, &left_root, &right_root, || {
        git::compare_worktrees(&left_root, &right_root, &path)
    })
}

#[tauri::command(async)]
pub fn sync_plan(
    root: String,
    state: State<'_, PersistentState>,
    runtime: State<'_, WorkspaceRuntime>,
) -> AppResult<SyncPlan> {
    with_workspace_lock(&runtime, &root, || git::sync_plan(&root, &state))
}

#[tauri::command(async)]
pub fn sync_workspace_changes(
    root: String,
    state: State<'_, PersistentState>,
    runtime: State<'_, WorkspaceRuntime>,
) -> AppResult<SyncResult> {
    with_sync_credential(&root, &state, |credential| {
        with_workspace_lock(&runtime, &root, || {
            ensure_git_idle_for_root(&state, &root, Some(&root))?;
            git::sync_workspace_changes(&root, credential, &state)
        })
    })
}

#[tauri::command(async)]
pub fn resolve_conflict(
    root: String,
    path: String,
    recovery_id: String,
    choice: ConflictChoice,
    state: State<'_, PersistentState>,
    runtime: State<'_, WorkspaceRuntime>,
) -> AppResult<()> {
    with_workspace_lock(&runtime, &root, || {
        git::resolve_conflict(&root, &path, &recovery_id, choice, &state)
    })
}

#[tauri::command(async)]
pub fn resolve_conflict_with_content(
    root: String,
    path: String,
    recovery_id: String,
    content: String,
    state: State<'_, PersistentState>,
    runtime: State<'_, WorkspaceRuntime>,
) -> AppResult<()> {
    with_workspace_lock(&runtime, &root, || {
        git::resolve_conflict_with_content(&root, &path, &recovery_id, &content, &state)
    })
}

#[tauri::command(async)]
pub fn pending_conflicts(
    root: String,
    state: State<'_, PersistentState>,
    runtime: State<'_, WorkspaceRuntime>,
) -> AppResult<Vec<ConflictRecord>> {
    with_workspace_lock(&runtime, &root, || git::pending_conflicts(&root, &state))
}

#[tauri::command(async)]
pub fn pending_git_operation(
    root: String,
    state: State<'_, PersistentState>,
) -> Option<PendingGitOperation> {
    git::pending_git_operation(&root, &state)
}

#[tauri::command(async)]
pub fn resume_git_operation(
    root: String,
    state: State<'_, PersistentState>,
    runtime: State<'_, WorkspaceRuntime>,
) -> AppResult<SyncResult> {
    with_sync_credential(&root, &state, |credential| {
        with_workspace_lock(&runtime, &root, || {
            git::resume_git_operation(&root, credential, &state)
        })
    })
}

#[tauri::command(async)]
pub fn abort_git_operation(
    root: String,
    state: State<'_, PersistentState>,
    runtime: State<'_, WorkspaceRuntime>,
) -> AppResult<GitStatusSnapshot> {
    with_workspace_lock(&runtime, &root, || git::abort_git_operation(&root, &state))
}
