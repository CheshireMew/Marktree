use crate::{
    auth,
    error::{AppError, AppResult},
    git,
    types::{CredentialRecord, SyncPlan, SyncResult, WorkspaceChange},
};

use super::WorkspaceService;

impl WorkspaceService<'_> {
    pub(crate) fn workspace_changes(&self, root: &str) -> AppResult<Vec<WorkspaceChange>> {
        self.run(root, || self.state.try_workspace_changes(root))
    }

    pub(crate) fn sync_plan(&self, root: &str) -> AppResult<SyncPlan> {
        self.run(root, || git::sync_plan(root, self.state))
    }

    pub(crate) fn sync(&self, root: &str) -> AppResult<SyncResult> {
        let credential = auth::credential_for_workspace(root, self.state)?;
        self.sync_with_credential(root, credential)
    }

    pub(crate) fn sync_with_credential(
        &self,
        root: &str,
        credential: Option<CredentialRecord>,
    ) -> AppResult<SyncResult> {
        self.run(root, || {
            let capability = git::refresh_repository(root)?;
            for worktree in &capability.worktrees {
                if crate::paths::paths_equal(root, &worktree.path) {
                    continue;
                }
                if self
                    .state
                    .try_pending_git_operation(&worktree.path)?
                    .is_some()
                {
                    return Err(AppError::GitOperationPending {
                        root: worktree.path.clone(),
                    });
                }
            }
            git::sync_workspace_changes(root, credential, self.state)
        })
    }
}
