use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

use parking_lot::Mutex;
use serde::{Deserialize, Serialize};

use crate::{
    error::{AppError, AppResult},
    operation_log::OperationLog,
    paths::atomic_write,
    process_lock,
    types::{
        GitOperationKind, GitOperationPhase, OperationLogCategory, OperationLogEntry,
        OperationLogOutcome, PendingGitOperation, WorkspaceChange,
    },
    workspace_operation::PendingWorkspaceOperation,
};

mod catalog;
mod git_operations;
mod migrations;
mod workspace_operations;

const STATE_SCHEMA_VERSION: u32 = 7;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
#[serde(rename_all = "camelCase")]
struct LocalState {
    schema_version: u32,
    next_generation: u64,
    workspaces: Vec<String>,
    workspace_changes: BTreeMap<String, BTreeMap<String, WorkspaceChange>>,
    pending_workspace_operations: BTreeMap<String, PendingWorkspaceOperation>,
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
            pending_workspace_operations: BTreeMap::new(),
            pending_git_operations: BTreeMap::new(),
            recent_files: Vec::new(),
            credential_refs: BTreeMap::new(),
        }
    }
}

pub struct PersistentState {
    file_path: PathBuf,
    backup_path: PathBuf,
    lock_path: PathBuf,
    app_data_dir: PathBuf,
    inner: Mutex<LocalState>,
    operation_log: OperationLog,
    read_only: bool,
}

impl PersistentState {
    pub fn load(app_data_dir: &Path) -> AppResult<Self> {
        fs::create_dir_all(app_data_dir)?;
        let file_path = app_data_dir.join("state.json");
        let backup_path = app_data_dir.join("state.backup.json");
        let lock_path = app_data_dir.join("state.lock");
        let _file_lock = process_lock::exclusive(&lock_path)?;
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
            lock_path,
            app_data_dir: app_data_dir.to_path_buf(),
            inner: Mutex::new(inner),
            operation_log: OperationLog::new(app_data_dir),
            read_only: false,
        };
        if migrated {
            state.persist(&state.inner.lock())?;
        }
        Ok(state)
    }

    pub fn load_read_only(app_data_dir: &Path) -> AppResult<Self> {
        let file_path = app_data_dir.join("state.json");
        let backup_path = app_data_dir.join("state.backup.json");
        let lock_path = app_data_dir.join("state.lock");
        let inner = match (load_valid_state(&file_path), load_valid_state(&backup_path)) {
            (Some((state, _)), _) | (None, Some((state, _))) => state,
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
            lock_path,
            app_data_dir: app_data_dir.to_path_buf(),
            inner: Mutex::new(inner),
            operation_log: OperationLog::new(app_data_dir),
            read_only: true,
        })
    }

    pub fn operation_log(&self, limit: usize) -> AppResult<Vec<OperationLogEntry>> {
        self.operation_log.read_recent(limit)
    }

    pub fn recovery_dir(&self) -> AppResult<PathBuf> {
        let path = self.app_data_dir.join("recovery");
        fs::create_dir_all(&path)?;
        Ok(path)
    }

    #[cfg(target_os = "windows")]
    pub(crate) fn lock_workspace(&self, key: &str) -> AppResult<process_lock::NamedMutexGuard> {
        if self.read_only {
            return Err(AppError::Message(
                "A read-only state view cannot create a workspace lock.".to_owned(),
            ));
        }
        let identity = crate::file_version::hash_bytes(key.as_bytes());
        process_lock::named_workspace_mutex(&identity)
    }

    #[cfg(not(target_os = "windows"))]
    pub(crate) fn lock_workspace(&self, key: &str) -> AppResult<std::fs::File> {
        if self.read_only {
            return Err(AppError::Message(
                "A read-only state view cannot create a workspace lock.".to_owned(),
            ));
        }
        let identity = crate::file_version::hash_bytes(key.as_bytes());
        process_lock::exclusive(
            &self
                .app_data_dir
                .join("locks")
                .join(format!("workspace-{identity}.lock")),
        )
    }

    #[cfg(target_os = "windows")]
    pub(crate) fn lock_workspace_read_only(
        &self,
        key: &str,
    ) -> AppResult<process_lock::NamedMutexGuard> {
        let identity = crate::file_version::hash_bytes(key.as_bytes());
        process_lock::named_workspace_mutex(&identity)
    }

    #[cfg(not(target_os = "windows"))]
    pub(crate) fn lock_workspace_read_only(&self, key: &str) -> AppResult<Option<std::fs::File>> {
        let identity = crate::file_version::hash_bytes(key.as_bytes());
        process_lock::shared_existing(
            &self
                .app_data_dir
                .join("locks")
                .join(format!("workspace-{identity}.lock")),
        )
    }

    #[cfg(target_os = "android")]
    pub(crate) fn app_data_dir(&self) -> &Path {
        &self.app_data_dir
    }

    fn update(&self, mutate: impl FnOnce(&mut LocalState)) -> AppResult<()> {
        if self.read_only {
            return Err(AppError::Message(
                "A read-only state view cannot be modified.".to_owned(),
            ));
        }
        let _file_lock = process_lock::exclusive(&self.lock_path)?;
        let mut current = self.inner.lock();
        let mut next = self.load_authoritative()?;
        let before = serde_json::to_vec(&next)?;
        mutate(&mut next);
        if serde_json::to_vec(&next)? == before {
            *current = next;
            return Ok(());
        }
        self.persist(&next)?;
        *current = next;
        Ok(())
    }

    fn read<T>(&self, inspect: impl FnOnce(&LocalState) -> T) -> AppResult<T> {
        if self.read_only {
            let latest = self.load_authoritative()?;
            let result = inspect(&latest);
            *self.inner.lock() = latest;
            return Ok(result);
        }
        let _file_lock = process_lock::exclusive(&self.lock_path)?;
        let latest = self.load_authoritative()?;
        let result = inspect(&latest);
        *self.inner.lock() = latest;
        Ok(result)
    }

    fn load_authoritative(&self) -> AppResult<LocalState> {
        match (
            load_valid_state(&self.file_path),
            load_valid_state(&self.backup_path),
        ) {
            (Some((state, _)), _) | (None, Some((state, _))) => Ok(state),
            (None, None) if !self.file_path.exists() && !self.backup_path.exists() => {
                Ok(LocalState::default())
            }
            (None, None) => Err(AppError::Message(format!(
                "Marktree local state is unreadable. The files were preserved at '{}' and '{}'.",
                self.file_path.display(),
                self.backup_path.display()
            ))),
        }
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

    fn append_workspace_operation_log(
        &self,
        operation: &PendingWorkspaceOperation,
        category: OperationLogCategory,
        outcome: OperationLogOutcome,
        error: Option<&AppError>,
    ) {
        self.append_operation_log(OperationLogEntry {
            timestamp: chrono::Utc::now().to_rfc3339(),
            category,
            action: operation.kind.log_action().to_owned(),
            phase: operation.phase.log_name().to_owned(),
            outcome,
            root: Some(operation.root.clone()),
            operation_id: Some(operation.id.clone()),
            error_code: error.map(AppError::code),
        });
    }

    fn append_git_operation_log(
        &self,
        operation: &PendingGitOperation,
        outcome: OperationLogOutcome,
        error: Option<&AppError>,
    ) {
        self.append_operation_log(OperationLogEntry {
            timestamp: chrono::Utc::now().to_rfc3339(),
            category: OperationLogCategory::Git,
            action: git_operation_action(operation.kind).to_owned(),
            phase: git_operation_phase(operation.phase).to_owned(),
            outcome,
            root: Some(operation.root.clone()),
            operation_id: Some(operation.id.clone()),
            error_code: error.map(AppError::code),
        });
    }

    fn append_operation_log(&self, entry: OperationLogEntry) {
        let _ = self.operation_log.append(&entry);
    }
}

fn git_operation_action(kind: GitOperationKind) -> &'static str {
    match kind {
        GitOperationKind::Pull => "pull",
        GitOperationKind::Sync => "sync",
    }
}

fn git_operation_phase(phase: GitOperationPhase) -> &'static str {
    match phase {
        GitOperationPhase::Prepare => "prepare",
        GitOperationPhase::Commit => "commit",
        GitOperationPhase::Fetch => "fetch",
        GitOperationPhase::PreserveWorkingTree => "preserveWorkingTree",
        GitOperationPhase::Rebase => "rebase",
        GitOperationPhase::RestoreWorkingTree => "restoreWorkingTree",
        GitOperationPhase::Push => "push",
        GitOperationPhase::Finalize => "finalize",
    }
}

fn load_valid_state(path: &Path) -> Option<(LocalState, bool)> {
    let mut value = serde_json::from_slice::<serde_json::Value>(&fs::read(path).ok()?).ok()?;
    let original_schema_version = value.get("schemaVersion")?.as_u64()? as u32;
    migrations::migrate_state_value(&mut value)?;
    if value.get("schemaVersion")?.as_u64()? as u32 != STATE_SCHEMA_VERSION {
        return None;
    }
    serde_json::from_value(value)
        .ok()
        .map(|state| (state, original_schema_version != STATE_SCHEMA_VERSION))
}
