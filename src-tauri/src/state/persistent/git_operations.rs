use crate::{
    error::{AppError, AppResult},
    types::{OperationLogOutcome, PendingGitOperation},
};

use super::PersistentState;

impl PersistentState {
    pub fn begin_git_operation(&self, operation: PendingGitOperation) -> AppResult<()> {
        let root = operation.root.clone();
        let mut conflict = false;
        self.update(|state| {
            if state.pending_git_operations.contains_key(&root) {
                conflict = true;
            } else {
                state
                    .pending_git_operations
                    .insert(root.clone(), operation.clone());
            }
        })?;
        if conflict {
            Err(AppError::Message(
                "This worktree already has an unfinished Git operation.".to_owned(),
            ))
        } else {
            self.append_git_operation_log(&operation, OperationLogOutcome::Started, None);
            Ok(())
        }
    }

    pub fn try_pending_git_operation(&self, root: &str) -> AppResult<Option<PendingGitOperation>> {
        self.read(|state| state.pending_git_operations.get(root).cloned())
    }

    #[cfg(test)]
    pub fn pending_git_operation(&self, root: &str) -> Option<PendingGitOperation> {
        self.try_pending_git_operation(root)
            .expect("test state should be readable")
    }

    pub fn update_git_operation(&self, operation: PendingGitOperation) -> AppResult<()> {
        let root = operation.root.clone();
        let id = operation.id.clone();
        let mut missing = false;
        let mut changed = false;
        self.update(|state| {
            let current = state
                .pending_git_operations
                .get(&root)
                .filter(|current| current.id == id);
            if let Some(current) = current {
                changed =
                    current.phase != operation.phase || current.aborting != operation.aborting;
                state.pending_git_operations.insert(root.clone(), operation);
            } else {
                missing = true;
            }
        })?;
        if missing {
            Err(AppError::Message(
                "The pending Git operation changed before it could be updated.".to_owned(),
            ))
        } else {
            if changed {
                let operation = self
                    .try_pending_git_operation(&root)?
                    .expect("updated operation");
                self.append_git_operation_log(&operation, OperationLogOutcome::Progress, None);
            }
            Ok(())
        }
    }

    pub fn finish_git_operation(
        &self,
        root: &str,
        id: &str,
        outcome: OperationLogOutcome,
        error: Option<&AppError>,
    ) -> AppResult<()> {
        let mut missing = false;
        let mut completed = None;
        self.update(|state| {
            let operation = state
                .pending_git_operations
                .get(root)
                .filter(|current| current.id == id)
                .cloned();
            if let Some(operation) = operation {
                completed = Some(operation);
                state.pending_git_operations.remove(root);
            } else {
                missing = true;
            }
        })?;
        if missing {
            Err(AppError::Message(
                "The pending Git operation changed before it could be finalized.".to_owned(),
            ))
        } else {
            if let Some(operation) = completed {
                self.append_git_operation_log(&operation, outcome, error);
            }
            Ok(())
        }
    }

    pub(crate) fn record_git_operation_failure(
        &self,
        operation: &PendingGitOperation,
        error: &AppError,
    ) {
        self.append_git_operation_log(operation, OperationLogOutcome::Failed, Some(error));
    }
}
