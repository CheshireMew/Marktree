use crate::{
    error::{AppError, AppResult},
    state::{PersistentState, WorkspaceRuntime},
    workspace_guard,
};

mod documents_service;
mod git_service;
mod portability_service;
mod workspaces_service;

pub(crate) struct WorkspaceService<'a> {
    state: &'a PersistentState,
    runtime: &'a WorkspaceRuntime,
    recover_pending: bool,
}

impl<'a> WorkspaceService<'a> {
    pub(crate) fn new(state: &'a PersistentState, runtime: &'a WorkspaceRuntime) -> Self {
        Self {
            state,
            runtime,
            recover_pending: true,
        }
    }

    pub(crate) fn new_read_only(state: &'a PersistentState, runtime: &'a WorkspaceRuntime) -> Self {
        Self {
            state,
            runtime,
            recover_pending: false,
        }
    }

    pub(crate) fn run<T>(
        &self,
        root: &str,
        operation: impl FnOnce() -> AppResult<T>,
    ) -> AppResult<T> {
        if self.recover_pending {
            workspace_guard::with_workspace(self.runtime, self.state, root, operation)
        } else {
            workspace_guard::with_workspace_read_only(self.runtime, self.state, root, operation)
        }
    }

    pub(crate) fn run_two<T>(
        &self,
        left_root: &str,
        right_root: &str,
        operation: impl FnOnce() -> AppResult<T>,
    ) -> AppResult<T> {
        if !self.recover_pending {
            return Err(AppError::Message(
                "A read-only workspace view cannot coordinate a two-workspace operation."
                    .to_owned(),
            ));
        }
        workspace_guard::with_two_workspaces(
            self.runtime,
            self.state,
            left_root,
            right_root,
            operation,
        )
    }

    fn ensure_writable(&self, root: &str) -> AppResult<()> {
        let blocked = self
            .state
            .try_pending_git_operation(root)?
            .is_some_and(|operation| {
                operation.aborting
                    || matches!(
                        operation.phase,
                        crate::types::GitOperationPhase::Commit
                            | crate::types::GitOperationPhase::PreserveWorkingTree
                            | crate::types::GitOperationPhase::Rebase
                            | crate::types::GitOperationPhase::RestoreWorkingTree
                    )
            });
        if blocked {
            Err(AppError::GitOperationPending {
                root: root.to_owned(),
            })
        } else {
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::TempDir;

    use super::*;
    use crate::git;

    #[test]
    fn asset_finish_revalidates_the_owning_document_before_publishing() {
        let workspace = TempDir::new().unwrap();
        let app_data = TempDir::new().unwrap();
        let upload = TempDir::new().unwrap();
        let root = workspace.path().to_string_lossy().into_owned();
        fs::write(workspace.path().join("note.md"), b"# Note").unwrap();
        let source = upload.path().join("photo.png");
        fs::write(&source, b"image bytes").unwrap();
        let state = PersistentState::load(app_data.path()).unwrap();
        let runtime = WorkspaceRuntime::default();
        let service = WorkspaceService::new(&state, &runtime);
        let document_sha256 = service.read_document(&root, "note.md").unwrap().sha256;
        fs::remove_file(workspace.path().join("note.md")).unwrap();

        assert!(service
            .write_asset(
                &root,
                "note.md",
                "photo.png",
                &source,
                None,
                &document_sha256,
            )
            .is_err());
        assert!(!workspace.path().join("assets").exists());
    }

    #[test]
    fn asset_finish_rejects_a_replacement_at_the_same_document_path() {
        let workspace = TempDir::new().unwrap();
        let app_data = TempDir::new().unwrap();
        let upload = TempDir::new().unwrap();
        let root = workspace.path().to_string_lossy().into_owned();
        fs::write(workspace.path().join("note.md"), b"# Original").unwrap();
        let source = upload.path().join("photo.png");
        fs::write(&source, b"image bytes").unwrap();
        let state = PersistentState::load(app_data.path()).unwrap();
        let runtime = WorkspaceRuntime::default();
        let service = WorkspaceService::new(&state, &runtime);
        let document_sha256 = service.read_document(&root, "note.md").unwrap().sha256;
        fs::write(workspace.path().join("note.md"), b"# Replacement").unwrap();

        assert!(matches!(
            service.write_asset(
                &root,
                "note.md",
                "photo.png",
                &source,
                None,
                &document_sha256,
            ),
            Err(AppError::ExternalChange)
        ));
        assert!(!workspace.path().join("assets").exists());
    }

    #[test]
    fn one_workspace_view_uses_one_status_for_entries_and_header() {
        let workspace = TempDir::new().unwrap();
        let app_data = TempDir::new().unwrap();
        let root = workspace.path().to_string_lossy().into_owned();
        fs::write(workspace.path().join("note.md"), b"# Note").unwrap();
        git::initialize_repository(&root).unwrap();
        let state = PersistentState::load(app_data.path()).unwrap();
        let runtime = WorkspaceRuntime::default();

        let view = WorkspaceService::new(&state, &runtime)
            .workspace_view(&root)
            .unwrap();

        let status = view.status.unwrap();
        let file_status = status
            .files
            .iter()
            .find(|file| file.path == "note.md")
            .unwrap();
        let entry_status = view
            .entries
            .iter()
            .find(|entry| entry.path == "note.md")
            .and_then(|entry| entry.git_status.as_ref())
            .unwrap();
        assert_eq!(entry_status.path, file_status.path);
        assert_eq!(entry_status.worktree_status, file_status.worktree_status);
    }
}
