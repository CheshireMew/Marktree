use crate::{
    error::{AppError, AppResult},
    types::{OperationLogCategory, OperationLogOutcome, WorkspaceChange},
    workspace_operation::{
        PendingWorkspaceOperation, WorkspaceChangeIntent, WorkspaceOperationPhase,
    },
};

use super::{LocalState, PersistentState};

impl PersistentState {
    pub(crate) fn begin_workspace_operation(
        &self,
        operation: PendingWorkspaceOperation,
    ) -> AppResult<()> {
        let root = operation.root.clone();
        let mut conflict = false;
        self.update(|state| {
            if state.pending_workspace_operations.contains_key(&root) {
                conflict = true;
            } else {
                state
                    .pending_workspace_operations
                    .insert(root.clone(), operation.clone());
            }
        })?;
        if conflict {
            Err(AppError::Message(format!(
                "A workspace file operation is already pending for '{root}'."
            )))
        } else {
            self.append_workspace_operation_log(
                &operation,
                OperationLogCategory::Workspace,
                OperationLogOutcome::Started,
                None,
            );
            Ok(())
        }
    }

    pub(crate) fn try_pending_workspace_operation(
        &self,
        root: &str,
    ) -> AppResult<Option<PendingWorkspaceOperation>> {
        self.read(|state| state.pending_workspace_operations.get(root).cloned())
    }

    #[cfg(test)]
    pub(crate) fn pending_workspace_operation(
        &self,
        root: &str,
    ) -> Option<PendingWorkspaceOperation> {
        self.try_pending_workspace_operation(root)
            .expect("test state should be readable")
    }

    pub(crate) fn pending_workspace_operations(&self) -> AppResult<Vec<PendingWorkspaceOperation>> {
        self.read(|state| {
            state
                .pending_workspace_operations
                .values()
                .cloned()
                .collect()
        })
    }

    pub(crate) fn update_workspace_operation_phase(
        &self,
        root: &str,
        id: &str,
        phase: WorkspaceOperationPhase,
    ) -> AppResult<()> {
        let mut missing = false;
        self.update(
            |state| match state.pending_workspace_operations.get_mut(root) {
                Some(operation) if operation.id == id => operation.phase = phase,
                _ => missing = true,
            },
        )?;
        if missing {
            Err(AppError::Message(
                "The pending workspace operation changed before its phase could be saved."
                    .to_owned(),
            ))
        } else {
            if let Some(operation) = self.try_pending_workspace_operation(root)? {
                self.append_workspace_operation_log(
                    &operation,
                    OperationLogCategory::Workspace,
                    OperationLogOutcome::Progress,
                    None,
                );
            }
            Ok(())
        }
    }

    pub(crate) fn complete_workspace_operation(&self, root: &str, id: &str) -> AppResult<()> {
        let mut missing = false;
        let mut completed = None;
        self.update(|state| {
            let Some(operation) = state.pending_workspace_operations.get(root) else {
                missing = true;
                return;
            };
            if operation.id != id {
                missing = true;
                return;
            }
            let operation = operation.clone();
            completed = Some(operation.clone());
            if operation.track_changes {
                apply_workspace_changes(state, root, &operation.changes);
            }
            state.pending_workspace_operations.remove(root);
        })?;
        if missing {
            Err(AppError::Message(
                "The pending workspace operation changed before it could be completed.".to_owned(),
            ))
        } else {
            if let Some(operation) = completed {
                self.append_workspace_operation_log(
                    &operation,
                    OperationLogCategory::Workspace,
                    OperationLogOutcome::Succeeded,
                    None,
                );
            }
            Ok(())
        }
    }

    pub(crate) fn cancel_workspace_operation(&self, root: &str, id: &str) -> AppResult<()> {
        let mut missing = false;
        let mut cancelled = None;
        self.update(|state| {
            let operation = state
                .pending_workspace_operations
                .get(root)
                .filter(|operation| operation.id == id)
                .cloned();
            if let Some(operation) = operation {
                cancelled = Some(operation);
                state.pending_workspace_operations.remove(root);
            } else {
                missing = true;
            }
        })?;
        if missing {
            Err(AppError::Message(
                "The pending workspace operation changed before it could be cancelled.".to_owned(),
            ))
        } else {
            if let Some(operation) = cancelled {
                self.append_workspace_operation_log(
                    &operation,
                    OperationLogCategory::Workspace,
                    OperationLogOutcome::Cancelled,
                    None,
                );
            }
            Ok(())
        }
    }

    pub(crate) fn fail_workspace_operation(
        &self,
        root: &str,
        id: &str,
        error: &AppError,
    ) -> AppResult<()> {
        let mut missing = false;
        let mut failed = None;
        self.update(|state| {
            let operation = state
                .pending_workspace_operations
                .get(root)
                .filter(|operation| operation.id == id)
                .cloned();
            if let Some(operation) = operation {
                failed = Some(operation);
                state.pending_workspace_operations.remove(root);
            } else {
                missing = true;
            }
        })?;
        if missing {
            Err(AppError::Message(
                "The pending workspace operation changed before it could be failed.".to_owned(),
            ))
        } else {
            if let Some(operation) = failed {
                self.append_workspace_operation_log(
                    &operation,
                    OperationLogCategory::Workspace,
                    OperationLogOutcome::Failed,
                    Some(error),
                );
            }
            Ok(())
        }
    }

    pub(crate) fn record_workspace_operation_failure(
        &self,
        operation: &PendingWorkspaceOperation,
        error: &AppError,
        recovery: bool,
    ) {
        self.append_workspace_operation_log(
            operation,
            if recovery {
                OperationLogCategory::Recovery
            } else {
                OperationLogCategory::Workspace
            },
            OperationLogOutcome::Failed,
            Some(error),
        );
    }

    #[cfg(test)]
    pub(crate) fn seed_workspace_changes(&self, root: &str, changes: &[WorkspaceChangeIntent]) {
        self.update(|state| apply_workspace_changes(state, root, changes))
            .expect("test workspace changes should persist");
    }

    pub fn try_workspace_changes(&self, root: &str) -> AppResult<Vec<WorkspaceChange>> {
        self.read(|state| {
            state
                .workspace_changes
                .get(root)
                .map(|changes| changes.values().cloned().collect())
                .unwrap_or_default()
        })
    }

    #[cfg(test)]
    pub fn workspace_changes(&self, root: &str) -> Vec<WorkspaceChange> {
        self.try_workspace_changes(root)
            .expect("test state should be readable")
    }

    pub fn clear_workspace_changes(
        &self,
        root: &str,
        completed: &[WorkspaceChange],
    ) -> AppResult<()> {
        self.update(|state| {
            if let Some(current) = state.workspace_changes.get_mut(root) {
                for change in completed {
                    let unchanged = current.get(&change.path).is_some_and(|candidate| {
                        candidate.generation == change.generation
                            && candidate.operation == change.operation
                            && candidate.version == change.version
                    });
                    if unchanged {
                        current.remove(&change.path);
                    }
                }
                if current.is_empty() {
                    state.workspace_changes.remove(root);
                }
            }
        })
    }
}

fn apply_workspace_changes(state: &mut LocalState, root: &str, changes: &[WorkspaceChangeIntent]) {
    for intent in changes {
        let generation = state.next_generation;
        state.next_generation = state.next_generation.saturating_add(1);
        let change = WorkspaceChange {
            path: intent.path.clone(),
            generation,
            operation: intent.operation.clone(),
            version: intent.version.clone(),
        };
        state
            .workspace_changes
            .entry(root.to_owned())
            .or_default()
            .insert(intent.path.clone(), change);
    }
}
