use std::{collections::BTreeSet, path::Path};

use crate::{
    documents,
    error::{AppError, AppResult},
    git,
    types::{
        AssetWriteResult, DocumentContent, DocumentSearchRequest, DocumentSearchResponse,
        GitStatusSnapshot, PendingGitOperationSummary, SaveDocumentRequest, SaveDocumentResult,
        SaveWorkspaceConfigRequest, TrashEntry, WorkspaceConfigSnapshot, WorkspaceDescriptor,
        WorkspaceEntriesPatch, WorkspaceEntry, WorkspaceEntryDuplicateResult,
        WorkspaceEntryMoveResult, WorkspaceFilePreview, WorkspaceRefreshSnapshot,
        WorkspaceViewSnapshot,
    },
    workspace,
};

use super::WorkspaceService;

impl WorkspaceService<'_> {
    pub(crate) fn list_workspace_directories(&self, root: &str) -> AppResult<Vec<String>> {
        self.run(root, || documents::list_workspace_directories(root))
    }

    pub(crate) fn list_workspace_entries(&self, root: &str) -> AppResult<Vec<WorkspaceEntry>> {
        self.run(root, || {
            let statuses = if Path::new(root).join(".git").exists() {
                git::repository_status(root)?.files
            } else {
                Vec::new()
            };
            documents::list_workspace_entries(root, &statuses)
        })
    }

    pub(crate) fn workspace_view(&self, root: &str) -> AppResult<WorkspaceViewSnapshot> {
        self.run(root, || {
            let status = Path::new(root)
                .join(".git")
                .exists()
                .then(|| git::repository_status(root))
                .transpose()?;
            self.workspace_view_from_status(root, status)
        })
    }

    pub(crate) fn workspace_entries_patch(
        &self,
        root: &str,
        paths: &[String],
    ) -> AppResult<WorkspaceEntriesPatch> {
        self.run(root, || {
            let status = Path::new(root)
                .join(".git")
                .exists()
                .then(|| git::repository_status(root))
                .transpose()?;
            documents::workspace_entries_patch(root, paths, status)
        })
    }

    pub(crate) fn refresh_workspace_view(
        &self,
        workspace_root: &str,
        content_root: &str,
    ) -> AppResult<WorkspaceRefreshSnapshot> {
        self.run(workspace_root, || {
            let descriptor = workspace::refresh_workspace(workspace_root)?;
            let status = status_for_content_root(&descriptor, content_root)?;
            let view = self.workspace_view_from_status(content_root, status)?;
            Ok(WorkspaceRefreshSnapshot {
                workspace: descriptor,
                view,
            })
        })
    }

    pub(crate) fn read_document(&self, root: &str, path: &str) -> AppResult<DocumentContent> {
        self.run(root, || documents::read_document(root, path))
    }

    pub(crate) fn read_workspace_preview(
        &self,
        root: &str,
        path: &str,
    ) -> AppResult<WorkspaceFilePreview> {
        self.run(root, || documents::read_workspace_preview(root, path))
    }

    pub(crate) fn read_workspace_config(&self, root: &str) -> AppResult<WorkspaceConfigSnapshot> {
        self.run(root, || documents::read_workspace_config(root))
    }

    pub(crate) fn save_workspace_config(
        &self,
        request: SaveWorkspaceConfigRequest,
    ) -> AppResult<WorkspaceConfigSnapshot> {
        let root = request.root.clone();
        self.run(&root, || {
            self.ensure_writable(&root)?;
            documents::save_workspace_config(request, self.state)
        })
    }

    pub(crate) fn trash_entry(
        &self,
        root: &str,
        path: &str,
        app_data_dir: &Path,
    ) -> AppResult<Option<TrashEntry>> {
        self.run(root, || {
            self.ensure_writable(root)?;
            documents::trash_entry(root, path, app_data_dir, self.state)
        })
    }

    pub(crate) fn restore_trash(&self, app_data_dir: &Path, id: &str) -> AppResult<TrashEntry> {
        let entries = documents::list_android_trash(app_data_dir)?;
        let entry =
            entries
                .iter()
                .find(|entry| entry.id == id)
                .ok_or_else(|| AppError::FileNotFound {
                    path: id.to_owned(),
                })?;
        let root = entry.workspace_root.clone();
        self.run(&root, || {
            self.ensure_writable(&root)?;
            documents::restore_android_trash(app_data_dir, id, self.state)
        })
    }

    pub(crate) fn open_document(&self, root: &str, path: &str) -> AppResult<DocumentContent> {
        self.run(root, || documents::open_document(root, path, self.state))
    }

    pub(crate) fn save_document(
        &self,
        request: SaveDocumentRequest,
    ) -> AppResult<SaveDocumentResult> {
        let root = request.root.clone();
        self.run(&root, || {
            self.ensure_writable(&root)?;
            documents::save_document(request, self.state)
        })
    }

    pub(crate) fn save_documents(
        &self,
        requests: Vec<SaveDocumentRequest>,
    ) -> AppResult<Vec<SaveDocumentResult>> {
        let root = one_batch_root(&requests)?;
        let mut paths = BTreeSet::new();
        for request in &requests {
            let normalized = crate::paths::normalize_content_relative(&request.path)?;
            if !paths.insert(normalized.clone()) {
                return Err(AppError::Message(format!(
                    "A document batch cannot write the same path twice: {normalized}"
                )));
            }
        }
        self.run(&root, || {
            self.ensure_writable(&root)?;
            for request in &requests {
                documents::validate_save_document(request)?;
            }
            requests
                .into_iter()
                .map(|request| documents::save_document(request, self.state))
                .collect()
        })
    }

    pub(crate) fn create_document(&self, root: &str, path: &str) -> AppResult<DocumentContent> {
        self.run(root, || {
            self.ensure_writable(root)?;
            documents::create_document(root, path, self.state)
        })
    }

    pub(crate) fn import_file_from_path(
        &self,
        root: &str,
        path: &str,
        source_path: &Path,
    ) -> AppResult<String> {
        self.run(root, || {
            self.ensure_writable(root)?;
            documents::import_file_from_path(root, path, source_path, self.state)
        })
    }

    pub(crate) fn write_asset(
        &self,
        root: &str,
        document_path: &str,
        file_name: &str,
        source_path: &Path,
        assets_dir: Option<&str>,
        expected_document_sha256: &str,
    ) -> AppResult<AssetWriteResult> {
        self.run(root, || {
            self.ensure_writable(root)?;
            let document = documents::read_document(root, document_path)?;
            if document.sha256 != expected_document_sha256 {
                return Err(AppError::ExternalChange);
            }
            documents::write_asset(
                root,
                document_path,
                file_name,
                source_path,
                assets_dir,
                self.state,
            )
        })
    }

    pub(crate) fn search_documents(
        &self,
        root: &str,
        query: &str,
        limit: usize,
        client_id: &str,
    ) -> AppResult<DocumentSearchResponse> {
        crate::paths::canonical_root(root)?;
        let key = git::repository_lock_key(root);
        let search = self.runtime.search_session(&key, client_id);
        documents::search_documents(
            root,
            query,
            limit.min(crate::state::MAX_SEARCH_RESULTS),
            || search.is_current(),
        )
    }

    pub(crate) fn search_documents_with_filters(
        &self,
        request: DocumentSearchRequest,
        client_id: &str,
    ) -> AppResult<DocumentSearchResponse> {
        crate::paths::canonical_root(&request.root)?;
        let key = git::repository_lock_key(&request.root);
        let search = self.runtime.search_session(&key, client_id);
        documents::search_documents_filtered(
            &request.root,
            &request.query,
            request.limit.min(crate::state::MAX_SEARCH_RESULTS),
            request.path_prefix.as_deref(),
            &request.file_kinds,
            request.modified_after_ms,
            || search.is_current(),
        )
    }

    pub(crate) fn create_folder(&self, root: &str, path: &str) -> AppResult<String> {
        self.run(root, || {
            self.ensure_writable(root)?;
            documents::create_folder(root, path, self.state)
        })
    }

    pub(crate) fn move_entry(
        &self,
        root: &str,
        source_path: &str,
        destination_path: &str,
    ) -> AppResult<WorkspaceEntryMoveResult> {
        self.run(root, || {
            self.ensure_writable(root)?;
            documents::move_entry(root, source_path, destination_path, self.state)
        })
    }

    pub(crate) fn duplicate_entry(
        &self,
        root: &str,
        source_path: &str,
        destination_path: &str,
    ) -> AppResult<WorkspaceEntryDuplicateResult> {
        self.run(root, || {
            self.ensure_writable(root)?;
            documents::duplicate_entry(root, source_path, destination_path, self.state)
        })
    }

    pub(crate) fn workspace_view_from_status(
        &self,
        root: &str,
        status: Option<GitStatusSnapshot>,
    ) -> AppResult<WorkspaceViewSnapshot> {
        let entries = documents::list_workspace_entries(
            root,
            status
                .as_ref()
                .map(|snapshot| snapshot.files.as_slice())
                .unwrap_or_default(),
        )?;
        let (branches, pending_operation, conflicts) = if status.is_some() {
            let pending = self
                .state
                .try_pending_git_operation(root)?
                .as_ref()
                .map(PendingGitOperationSummary::from);
            (
                git::list_branches(root)?,
                pending,
                git::pending_conflicts(root, self.state)?,
            )
        } else {
            (Vec::new(), None, Vec::new())
        };
        Ok(WorkspaceViewSnapshot {
            entries,
            status,
            branches,
            pending_operation,
            conflicts,
        })
    }
}

fn status_for_content_root(
    descriptor: &WorkspaceDescriptor,
    content_root: &str,
) -> AppResult<Option<GitStatusSnapshot>> {
    let Some(capability) = &descriptor.git else {
        if crate::paths::paths_equal(&descriptor.root, content_root) {
            return Ok(None);
        }
        return Err(AppError::InvalidPath(content_root.to_owned()));
    };
    if crate::paths::paths_equal(&descriptor.root, content_root) {
        return Ok(capability.status.clone());
    }
    capability
        .worktrees
        .iter()
        .find(|worktree| crate::paths::paths_equal(&worktree.path, content_root))
        .map(|worktree| worktree.status.clone())
        .ok_or_else(|| AppError::InvalidPath(content_root.to_owned()))
}

fn one_batch_root(requests: &[SaveDocumentRequest]) -> AppResult<String> {
    let Some(first) = requests.first() else {
        return Err(AppError::Message(
            "A document batch must contain at least one write.".to_owned(),
        ));
    };
    if requests.iter().any(|request| request.root != first.root) {
        return Err(AppError::Message(
            "Every document in a batch must use the same workspace root.".to_owned(),
        ));
    }
    Ok(first.root.clone())
}
