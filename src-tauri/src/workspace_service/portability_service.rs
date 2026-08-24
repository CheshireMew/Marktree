use std::path::Path;

use crate::{
    archive,
    error::AppResult,
    types::{WorkspaceArchiveExportResult, WorkspaceArchiveImportResult},
};

use super::WorkspaceService;

impl WorkspaceService<'_> {
    pub(crate) fn export_workspace_archive(
        &self,
        root: &str,
        output_path: &Path,
    ) -> AppResult<WorkspaceArchiveExportResult> {
        self.run(root, || archive::export_workspace(root, output_path))
    }

    pub(crate) fn import_workspace_archive(
        &self,
        archive_path: &Path,
        app_data_dir: &Path,
        preferred_name: &str,
    ) -> AppResult<WorkspaceArchiveImportResult> {
        let import_root = app_data_dir.join("workspaces");
        self.run(&import_root.to_string_lossy(), || {
            archive::import_workspace(archive_path, app_data_dir, preferred_name, self.state)
        })
    }
}
