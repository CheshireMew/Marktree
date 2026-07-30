use tauri::{AppHandle, Manager};

use crate::{
    auth,
    error::AppResult,
    git,
    paths::paths_equal,
    state::{PersistentState, WorkspaceRuntime},
    types::{GitCapability, GitOperationPhase, SyncResult, SyncStage},
};

pub(super) fn credential_for_root(
    root: &str,
    state: &PersistentState,
) -> AppResult<Option<crate::types::CredentialRecord>> {
    state
        .credential_ref(&git::repository_lock_key(root))
        .as_deref()
        .map(auth::load_credential)
        .transpose()
}

pub(super) fn with_sync_credential(
    root: &str,
    state: &PersistentState,
    operation: impl FnOnce(Option<crate::types::CredentialRecord>) -> AppResult<SyncResult>,
) -> AppResult<SyncResult> {
    match credential_for_root(root, state) {
        Ok(credential) => operation(credential),
        Err(error) => Ok(SyncResult::failure(
            SyncStage::Credential,
            error,
            state
                .workspace_changes(root)
                .into_iter()
                .map(|change| change.path)
                .collect(),
        )),
    }
}

pub(super) fn ensure_writable_during_git_operation(
    state: &PersistentState,
    root: &str,
) -> AppResult<()> {
    let blocked = state.pending_git_operation(root).is_some_and(|operation| {
        operation.aborting
            || matches!(
                operation.phase,
                GitOperationPhase::Commit
                    | GitOperationPhase::PreserveWorkingTree
                    | GitOperationPhase::Rebase
                    | GitOperationPhase::RestoreWorkingTree
            )
    });
    if blocked {
        Err(crate::error::AppError::GitOperationPending {
            root: root.to_owned(),
        })
    } else {
        Ok(())
    }
}

pub(super) fn ensure_worktree_idle(state: &PersistentState, root: &str) -> AppResult<()> {
    if state.pending_git_operation(root).is_some() {
        Err(crate::error::AppError::GitOperationPending {
            root: root.to_owned(),
        })
    } else {
        Ok(())
    }
}

pub(super) fn ensure_git_idle_for_root(
    state: &PersistentState,
    root: &str,
    allowed_root: Option<&str>,
) -> AppResult<()> {
    let capability = git::refresh_repository(root)?;
    ensure_git_idle(state, &capability, allowed_root)
}

pub(super) fn ensure_git_idle(
    state: &PersistentState,
    capability: &GitCapability,
    allowed_root: Option<&str>,
) -> AppResult<()> {
    for worktree in &capability.worktrees {
        if allowed_root.is_some_and(|allowed| paths_equal(allowed, &worktree.path)) {
            continue;
        }
        if state.pending_git_operation(&worktree.path).is_some() {
            return Err(crate::error::AppError::GitOperationPending {
                root: worktree.path.clone(),
            });
        }
    }
    Ok(())
}

pub(super) fn with_workspace_lock<T>(
    runtime: &WorkspaceRuntime,
    root: &str,
    operation: impl FnOnce() -> AppResult<T>,
) -> AppResult<T> {
    let key = git::repository_lock_key(root);
    let mutex = runtime.workspace_mutex(&key);
    let _guard = mutex.lock();
    operation()
}

pub(super) fn with_two_workspace_locks<T>(
    runtime: &WorkspaceRuntime,
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
    let _first_guard = first.lock();
    if keys[0] == keys[1] {
        return operation();
    }
    let second = runtime.workspace_mutex(&keys[1]);
    let _second_guard = second.lock();
    operation()
}

pub(super) fn mobile_workspace_path(app: &AppHandle, name: &str) -> AppResult<std::path::PathBuf> {
    let normalized = name
        .trim()
        .chars()
        .map(|character| {
            if character.is_alphanumeric() || matches!(character, '-' | '_') {
                character
            } else {
                '-'
            }
        })
        .collect::<String>()
        .trim_matches('-')
        .to_owned();
    if normalized.is_empty() {
        return Err(crate::error::AppError::Message(
            "A workspace name is required.".to_owned(),
        ));
    }
    Ok(app
        .path()
        .app_data_dir()?
        .join("workspaces")
        .join(normalized))
}
