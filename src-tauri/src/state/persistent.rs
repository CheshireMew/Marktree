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
    types::{LocalStateSnapshot, PendingGitOperation, WorkspaceChange, WorkspaceChangeOperation},
};

const STATE_SCHEMA_VERSION: u32 = 6;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
#[serde(rename_all = "camelCase")]
struct LocalState {
    schema_version: u32,
    next_generation: u64,
    workspaces: Vec<String>,
    workspace_changes: BTreeMap<String, BTreeMap<String, WorkspaceChange>>,
    pending_git_operations: BTreeMap<String, PendingGitOperation>,
    recent_files: Vec<String>,
    credential_refs: BTreeMap<String, String>,
}

impl Default for LocalState {
    fn default() -> Self {
        Self {
            schema_version: STATE_SCHEMA_VERSION,
            next_generation: 1,
            workspaces: Vec::new(),
            workspace_changes: BTreeMap::new(),
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
        let (inner, migrated) = match (load_valid_state(&file_path), load_valid_state(&backup_path))
        {
            (Some(state), _) | (None, Some(state)) => state,
            (None, None) if !file_path.exists() && !backup_path.exists() => {
                (LocalState::default(), false)
            }
            (None, None) => {
                return Err(AppError::Message(format!(
                "Marktree local state is unreadable. The files were preserved at '{}' and '{}'.",
                file_path.display(),
                backup_path.display()
            )))
            }
        };
        let state = Self {
            file_path,
            backup_path,
            app_data_dir: app_data_dir.to_path_buf(),
            inner: Mutex::new(inner),
        };
        if migrated {
            state.persist(&state.inner.lock())?;
        }
        Ok(state)
    }

    pub fn register_workspace(&self, root: &str) -> AppResult<()> {
        self.update(|state| {
            if !state.workspaces.iter().any(|value| value == root) {
                state.workspaces.push(root.to_owned());
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
                state.pending_git_operations.remove(root);
                state
                    .recent_files
                    .retain(|value| !value.starts_with(&format!("{root}\n")));
            }
        })
    }

    pub fn record_workspace_change(
        &self,
        root: &str,
        path: &str,
        operation: WorkspaceChangeOperation,
        version: Option<&str>,
    ) -> AppResult<WorkspaceChange> {
        let mut recorded = None;
        self.update(|state| {
            let generation = state.next_generation;
            state.next_generation = state.next_generation.saturating_add(1);
            let change = WorkspaceChange {
                path: path.to_owned(),
                generation,
                operation,
                version: version.map(str::to_owned),
            };
            state
                .workspace_changes
                .entry(root.to_owned())
                .or_default()
                .insert(path.to_owned(), change.clone());
            recorded = Some(change);
        })?;
        recorded.ok_or_else(|| AppError::Message("Failed to record the saved change.".to_owned()))
    }

    pub fn workspace_changes(&self, root: &str) -> Vec<WorkspaceChange> {
        self.inner
            .lock()
            .workspace_changes
            .get(root)
            .map(|changes| changes.values().cloned().collect())
            .unwrap_or_default()
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

fn load_valid_state(path: &Path) -> Option<(LocalState, bool)> {
    let mut value = serde_json::from_slice::<serde_json::Value>(&fs::read(path).ok()?).ok()?;
    let schema_version = value.get("schemaVersion")?.as_u64()? as u32;
    if schema_version == 5 {
        migrate_schema_five(&mut value)?;
        return serde_json::from_value(value)
            .ok()
            .map(|state| (state, true));
    }
    if schema_version != STATE_SCHEMA_VERSION {
        return None;
    }
    serde_json::from_value(value)
        .ok()
        .map(|state| (state, false))
}

fn migrate_schema_five(value: &mut serde_json::Value) -> Option<()> {
    let object = value.as_object_mut()?;
    let workspaces = object
        .remove("repositories")
        .unwrap_or_else(|| serde_json::Value::Array(Vec::new()));
    object.insert("workspaces".to_owned(), workspaces);
    let changes = object
        .remove("managedChanges")
        .unwrap_or_else(|| serde_json::json!({}));
    object.insert("workspaceChanges".to_owned(), migrate_change_map(changes)?);
    if let Some(operations) = object
        .get_mut("pendingGitOperations")
        .and_then(serde_json::Value::as_object_mut)
    {
        for operation in operations.values_mut() {
            let operation = operation.as_object_mut()?;
            let changes = operation
                .remove("managedChanges")
                .unwrap_or_else(|| serde_json::Value::Array(Vec::new()));
            operation.insert(
                "workspaceChanges".to_owned(),
                migrate_change_array(changes)?,
            );
        }
    }
    object.insert(
        "schemaVersion".to_owned(),
        serde_json::Value::from(STATE_SCHEMA_VERSION),
    );
    Some(())
}

fn migrate_change_map(value: serde_json::Value) -> Option<serde_json::Value> {
    let mut result = serde_json::Map::new();
    for (root, changes) in value.as_object()? {
        let mut migrated = serde_json::Map::new();
        for (path, change) in changes.as_object()? {
            migrated.insert(path.clone(), migrate_change(change.clone())?);
        }
        result.insert(root.clone(), serde_json::Value::Object(migrated));
    }
    Some(serde_json::Value::Object(result))
}

fn migrate_change_array(value: serde_json::Value) -> Option<serde_json::Value> {
    let values = value
        .as_array()?
        .iter()
        .cloned()
        .map(migrate_change)
        .collect::<Option<Vec<_>>>()?;
    Some(serde_json::Value::Array(values))
}

fn migrate_change(mut value: serde_json::Value) -> Option<serde_json::Value> {
    let change = value.as_object_mut()?;
    let version = change.remove("sha256").unwrap_or(serde_json::Value::Null);
    change.remove("kind");
    change.insert(
        "operation".to_owned(),
        serde_json::Value::String("upsert".to_owned()),
    );
    change.insert("version".to_owned(), version);
    Some(value)
}
