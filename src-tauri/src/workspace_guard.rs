use crate::{
    error::AppResult,
    git,
    paths::paths_equal,
    state::{PersistentState, WorkspaceRuntime},
    workspace_operation,
};

pub(crate) fn with_workspace<T>(
    runtime: &WorkspaceRuntime,
    state: &PersistentState,
    root: &str,
    operation: impl FnOnce() -> AppResult<T>,
) -> AppResult<T> {
    let key = git::repository_lock_key(root);
    let mutex = runtime.workspace_mutex(&key);
    let _runtime_guard = mutex.lock();
    let _process_guard = state.lock_workspace(&key)?;
    workspace_operation::recover_pending_for_root(root, state)?;
    operation()
}

pub(crate) fn with_workspace_read_only<T>(
    runtime: &WorkspaceRuntime,
    state: &PersistentState,
    root: &str,
    operation: impl FnOnce() -> AppResult<T>,
) -> AppResult<T> {
    let key = git::repository_lock_key(root);
    let mutex = runtime.workspace_mutex(&key);
    let _runtime_guard = mutex.lock();
    let _process_guard = state.lock_workspace_read_only(&key)?;
    operation()
}

pub(crate) fn with_two_workspaces<T>(
    runtime: &WorkspaceRuntime,
    state: &PersistentState,
    left_root: &str,
    right_root: &str,
    operation: impl FnOnce() -> AppResult<T>,
) -> AppResult<T> {
    let mut keys = [
        git::repository_lock_key(left_root),
        git::repository_lock_key(right_root),
    ];
    keys.sort();

    let first = runtime.workspace_mutex(&keys[0]);
    let _first_runtime_guard = first.lock();
    let _first_process_guard = state.lock_workspace(&keys[0])?;

    if keys[0] == keys[1] {
        workspace_operation::recover_pending_for_root(left_root, state)?;
        if !paths_equal(left_root, right_root) {
            workspace_operation::recover_pending_for_root(right_root, state)?;
        }
        return operation();
    }

    let second = runtime.workspace_mutex(&keys[1]);
    let _second_runtime_guard = second.lock();
    let _second_process_guard = state.lock_workspace(&keys[1])?;
    workspace_operation::recover_pending_for_root(left_root, state)?;
    workspace_operation::recover_pending_for_root(right_root, state)?;
    operation()
}
