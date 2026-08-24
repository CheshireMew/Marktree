use crate::{error::AppResult, types::StartupState};

#[cfg(test)]
use crate::types::LocalStateSnapshot;

use super::PersistentState;

impl PersistentState {
    pub fn register_workspace(&self, root: &str) -> AppResult<()> {
        self.update(|state| {
            if !state.workspaces.iter().any(|value| value == root) {
                state.workspaces.push(root.to_owned());
            }
        })
    }

    pub fn register_workspace_with_credential(
        &self,
        root: &str,
        credential_key: &str,
        credential_id: Option<&str>,
    ) -> AppResult<()> {
        self.update(|state| {
            if !state.workspaces.iter().any(|value| value == root) {
                state.workspaces.push(root.to_owned());
            }
            if let Some(credential_id) = credential_id {
                state
                    .credential_refs
                    .insert(credential_key.to_owned(), credential_id.to_owned());
            }
        })
    }

    pub fn forget_workspace(
        &self,
        workspace_root: &str,
        roots: &[String],
        credential_key: &str,
    ) -> AppResult<()> {
        self.update(|state| {
            state.workspaces.retain(|value| value != workspace_root);
            state.credential_refs.remove(credential_key);
            for root in roots {
                state.workspace_changes.remove(root);
                state.pending_workspace_operations.remove(root);
                state.pending_git_operations.remove(root);
                state
                    .recent_files
                    .retain(|value| !value.starts_with(&format!("{root}\n")));
            }
        })
    }

    pub fn remember_file(&self, root: &str, path: &str) -> AppResult<()> {
        let key = format!("{root}\n{path}");
        self.update(|state| {
            state.recent_files.retain(|value| value != &key);
            state.recent_files.insert(0, key);
            state.recent_files.truncate(crate::state::RECENT_FILE_LIMIT);
        })
    }

    pub fn set_credential_ref(&self, root: &str, credential_id: &str) -> AppResult<()> {
        self.update(|state| {
            state
                .credential_refs
                .insert(root.to_owned(), credential_id.to_owned());
        })
    }

    pub fn credential_ref(&self, root: &str) -> AppResult<Option<String>> {
        self.read(|state| state.credential_refs.get(root).cloned())
    }

    pub fn startup_state(&self) -> AppResult<StartupState> {
        self.read(|state| StartupState {
            workspaces: state.workspaces.clone(),
            recent_files: state.recent_files.clone(),
            recent_file_limit: crate::state::RECENT_FILE_LIMIT,
        })
    }

    #[cfg(test)]
    pub fn try_snapshot(&self) -> AppResult<LocalStateSnapshot> {
        self.read(|state| LocalStateSnapshot {
            workspaces: state.workspaces.clone(),
            workspace_changes: state
                .workspace_changes
                .iter()
                .map(|(root, changes)| {
                    (root.clone(), changes.values().cloned().collect::<Vec<_>>())
                })
                .collect(),
            pending_git_operations: state.pending_git_operations.clone(),
            recent_files: state.recent_files.clone(),
            credential_refs: state.credential_refs.clone(),
        })
    }

    #[cfg(test)]
    pub fn snapshot(&self) -> LocalStateSnapshot {
        self.try_snapshot().expect("test state should be readable")
    }
}
