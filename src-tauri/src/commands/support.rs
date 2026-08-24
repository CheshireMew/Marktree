use tauri::{AppHandle, Manager};

use crate::{
    auth,
    error::AppResult,
    git,
    paths::{paths_equal, portable_name_fragment},
    state::PersistentState,
    types::{GitCapability, SyncResult, SyncStage},
};

pub(super) async fn run_blocking<T>(
    operation: impl FnOnce() -> AppResult<T> + Send + 'static,
) -> AppResult<T>
where
    T: Send + 'static,
{
    tauri::async_runtime::spawn_blocking(operation)
        .await
        .map_err(|error| {
            crate::error::AppError::Message(format!(
                "The native worker stopped before completing the operation: {error}"
            ))
        })?
}

pub(super) fn with_sync_credential(
    root: &str,
    state: &PersistentState,
    operation: impl FnOnce(Option<crate::types::CredentialRecord>) -> AppResult<SyncResult>,
) -> AppResult<SyncResult> {
    match auth::credential_for_workspace(root, state) {
        Ok(credential) => operation(credential),
        Err(error) => {
            let changed_paths = state
                .try_workspace_changes(root)?
                .into_iter()
                .map(|change| change.path)
                .collect();
            Ok(SyncResult::failure(
                SyncStage::Credential,
                error,
                changed_paths,
            ))
        }
    }
}

pub(super) fn ensure_worktree_idle(state: &PersistentState, root: &str) -> AppResult<()> {
    if state.try_pending_git_operation(root)?.is_some() {
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
        if state.try_pending_git_operation(&worktree.path)?.is_some() {
            return Err(crate::error::AppError::GitOperationPending {
                root: worktree.path.clone(),
            });
        }
    }
    Ok(())
}

pub(super) fn mobile_workspace_path(app: &AppHandle, name: &str) -> AppResult<std::path::PathBuf> {
    let normalized = portable_name_fragment(name);
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blocking_native_work_runs_off_the_async_command_thread() {
        let command_thread = std::thread::current().id();
        let worker_thread =
            tauri::async_runtime::block_on(run_blocking(|| Ok(std::thread::current().id())))
                .unwrap();

        assert_ne!(command_thread, worker_thread);
    }
}
