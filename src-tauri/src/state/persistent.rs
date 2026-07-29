use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

use parking_lot::Mutex;
use serde::{Deserialize, Serialize};

use crate::{
    error::{AppError, AppResult},
    paths::atomic_write,
    types::{
        GitOperationPhase, LocalStateSnapshot, ManagedChange, ManagedChangeKind,
        PendingGitOperation,
    },
};

const STATE_SCHEMA_VERSION: u32 = 5;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
#[serde(rename_all = "camelCase")]
struct LocalState {
    schema_version: u32,
    next_generation: u64,
    repositories: Vec<String>,
    managed_changes: BTreeMap<String, BTreeMap<String, ManagedChange>>,
    pending_git_operations: BTreeMap<String, PendingGitOperation>,
    recent_files: Vec<String>,
    credential_refs: BTreeMap<String, String>,
}

impl Default for LocalState {
    fn default() -> Self {
        Self {
            schema_version: STATE_SCHEMA_VERSION,
            next_generation: 1,
            repositories: Vec::new(),
            managed_changes: BTreeMap::new(),
            pending_git_operations: BTreeMap::new(),
            recent_files: Vec::new(),
            credential_refs: BTreeMap::new(),
        }
    }
}

pub struct PersistentState {
    file_path: PathBuf,
    backup_path: PathBuf,
    app_data_dir: PathBuf,
    inner: Mutex<LocalState>,
}

impl PersistentState {
    pub fn load(app_data_dir: &Path) -> AppResult<Self> {
        fs::create_dir_all(app_data_dir)?;
        let file_path = app_data_dir.join("state.json");
        let backup_path = app_data_dir.join("state.backup.json");
        let inner = match (load_valid_state(&file_path), load_valid_state(&backup_path)) {
            (Some(state), _) | (None, Some(state)) => state,
            (None, None) if !file_path.exists() && !backup_path.exists() => LocalState::default(),
            (None, None) => {
                return Err(AppError::Message(format!(
                "Marktree local state is unreadable. The files were preserved at '{}' and '{}'.",
                file_path.display(),
                backup_path.display()
            )))
            }
        };
        Ok(Self {
            file_path,
            backup_path,
            app_data_dir: app_data_dir.to_path_buf(),
            inner: Mutex::new(inner),
        })
    }

    pub fn register_repository(&self, root: &str) -> AppResult<()> {
        self.update(|state| {
            if !state.repositories.iter().any(|value| value == root) {
                state.repositories.push(root.to_owned());
            }
        })
    }

    pub fn forget_repository(
        &self,
        repository_root: &str,
        worktree_roots: &[String],
        credential_key: &str,
    ) -> AppResult<()> {
        self.update(|state| {
            state.repositories.retain(|value| value != repository_root);
            state.credential_refs.remove(credential_key);
            for root in worktree_roots {
                state.managed_changes.remove(root);
                state.pending_git_operations.remove(root);
                state
                    .recent_files
                    .retain(|value| !value.starts_with(&format!("{root}\n")));
            }
        })
    }

    pub fn record_change(
        &self,
        root: &str,
        path: &str,
        sha256: &str,
        kind: ManagedChangeKind,
    ) -> AppResult<ManagedChange> {
        let mut recorded = None;
        self.update(|state| {
            let generation = state.next_generation;
            state.next_generation = state.next_generation.saturating_add(1);
            let change = ManagedChange {
                path: path.to_owned(),
                sha256: sha256.to_owned(),
                generation,
                kind,
            };
            state
                .managed_changes
                .entry(root.to_owned())
                .or_default()
                .insert(path.to_owned(), change.clone());
            recorded = Some(change);
        })?;
        recorded.ok_or_else(|| AppError::Message("Failed to record the saved change.".to_owned()))
    }

    pub fn managed_changes(&self, root: &str) -> Vec<ManagedChange> {
        self.inner
            .lock()
            .managed_changes
            .get(root)
            .map(|changes| changes.values().cloned().collect())
            .unwrap_or_default()
    }

    pub fn clear_managed_changes(&self, root: &str, completed: &[ManagedChange]) -> AppResult<()> {
        self.update(|state| {
            if let Some(current) = state.managed_changes.get_mut(root) {
                for change in completed {
                    let unchanged = current.get(&change.path).is_some_and(|candidate| {
                        candidate.generation == change.generation
                            && candidate.sha256 == change.sha256
                    });
                    if unchanged {
                        current.remove(&change.path);
                    }
                }
                if current.is_empty() {
                    state.managed_changes.remove(root);
                }
            }
        })
    }

    pub fn begin_git_operation(&self, operation: PendingGitOperation) -> AppResult<()> {
        let root = operation.root.clone();
        let mut conflict = false;
        self.update(|state| {
            if state.pending_git_operations.contains_key(&root) {
                conflict = true;
            } else {
                state.pending_git_operations.insert(root.clone(), operation);
            }
        })?;
        if conflict {
            Err(AppError::Message(
                "This worktree already has an unfinished Git operation.".to_owned(),
            ))
        } else {
            Ok(())
        }
    }

    pub fn pending_git_operation(&self, root: &str) -> Option<PendingGitOperation> {
        self.inner.lock().pending_git_operations.get(root).cloned()
    }

    pub fn update_git_operation(&self, operation: PendingGitOperation) -> AppResult<()> {
        let root = operation.root.clone();
        let id = operation.id.clone();
        let mut missing = false;
        self.update(|state| {
            let matches = state
                .pending_git_operations
                .get(&root)
                .is_some_and(|current| current.id == id);
            if matches {
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
            Ok(())
        }
    }

    pub fn finish_git_operation(&self, root: &str, id: &str) -> AppResult<()> {
        let mut missing = false;
        self.update(|state| {
            let matches = state
                .pending_git_operations
                .get(root)
                .is_some_and(|current| current.id == id);
            if matches {
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
            Ok(())
        }
    }

    pub fn remember_file(&self, root: &str, path: &str) -> AppResult<()> {
        let key = format!("{root}\n{path}");
        self.update(|state| {
            state.recent_files.retain(|value| value != &key);
            state.recent_files.insert(0, key);
            state.recent_files.truncate(40);
        })
    }

    pub fn set_credential_ref(&self, root: &str, credential_id: &str) -> AppResult<()> {
        self.update(|state| {
            state
                .credential_refs
                .insert(root.to_owned(), credential_id.to_owned());
        })
    }

    pub fn credential_ref(&self, root: &str) -> Option<String> {
        self.inner.lock().credential_refs.get(root).cloned()
    }

    pub fn recovery_dir(&self) -> AppResult<PathBuf> {
        let path = self.app_data_dir.join("recovery");
        fs::create_dir_all(&path)?;
        Ok(path)
    }

    pub fn snapshot(&self) -> LocalStateSnapshot {
        let state = self.inner.lock();
        LocalStateSnapshot {
            repositories: state.repositories.clone(),
            managed_changes: state
                .managed_changes
                .iter()
                .map(|(root, changes)| {
                    (root.clone(), changes.values().cloned().collect::<Vec<_>>())
                })
                .collect(),
            pending_git_operations: state.pending_git_operations.clone(),
            recent_files: state.recent_files.clone(),
            credential_refs: state.credential_refs.clone(),
        }
    }

    fn update(&self, mutate: impl FnOnce(&mut LocalState)) -> AppResult<()> {
        let mut current = self.inner.lock();
        let mut next = current.clone();
        mutate(&mut next);
        self.persist(&next)?;
        *current = next;
        Ok(())
    }

    fn persist(&self, state: &LocalState) -> AppResult<()> {
        let bytes = serde_json::to_vec_pretty(state)?;
        if self.file_path.exists() {
            let current = fs::read(&self.file_path)?;
            if serde_json::from_slice::<LocalState>(&current)
                .ok()
                .is_some_and(|value| value.schema_version == STATE_SCHEMA_VERSION)
            {
                atomic_write(&self.backup_path, &current)?;
            }
        }
        atomic_write(&self.file_path, &bytes)
    }
}

fn load_valid_state(path: &Path) -> Option<LocalState> {
    let mut state = serde_json::from_slice::<LocalState>(&fs::read(path).ok()?).ok()?;
    match state.schema_version {
        STATE_SCHEMA_VERSION => Some(state),
        2..=4 => {
            for operation in state.pending_git_operations.values_mut() {
                if operation.stash_oid.is_some()
                    && !operation.stash_applied
                    && operation.phase == GitOperationPhase::RestoreWorkingTree
                {
                    operation.stash_apply_started = true;
                }
            }
            state.schema_version = STATE_SCHEMA_VERSION;
            Some(state)
        }
        _ => None,
    }
}
