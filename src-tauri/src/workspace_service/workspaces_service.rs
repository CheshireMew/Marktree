use crate::{
    auth,
    error::AppResult,
    git,
    types::{GitBaselinePreview, WorkspaceDescriptor},
    workspace,
};

use super::WorkspaceService;

impl WorkspaceService<'_> {
    pub(crate) fn inspect_workspace(&self, root: &str) -> AppResult<WorkspaceDescriptor> {
        self.run(root, || workspace::refresh_workspace(root))
    }

    pub(crate) fn open_workspace(&self, root: &str) -> AppResult<WorkspaceDescriptor> {
        self.run(root, || workspace::open_workspace(root, self.state))
    }

    pub(crate) fn create_workspace(&self, root: &str) -> AppResult<WorkspaceDescriptor> {
        self.run(root, || workspace::create_workspace(root, self.state))
    }

    pub(crate) fn clone_workspace(
        &self,
        remote_url: &str,
        root: &str,
        credential_id: Option<&str>,
    ) -> AppResult<WorkspaceDescriptor> {
        let credential = credential_id.map(auth::load_credential).transpose()?;
        self.run(root, || {
            let descriptor = workspace::clone_workspace_unregistered(remote_url, root, credential)?;
            let credential_key = git::repository_lock_key(&descriptor.root);
            self.state.register_workspace_with_credential(
                &descriptor.root,
                &credential_key,
                credential_id,
            )?;
            Ok(descriptor)
        })
    }

    pub(crate) fn preview_git_baseline(&self, root: &str) -> AppResult<GitBaselinePreview> {
        self.run(root, || workspace::preview_git_baseline(root))
    }

    pub(crate) fn enable_git(&self, root: &str) -> AppResult<WorkspaceDescriptor> {
        self.run(root, || workspace::enable_git(root, self.state))
    }
}
