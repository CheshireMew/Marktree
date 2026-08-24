use std::{fs, path::Path};

use chrono::Utc;
use serde::{Deserialize, Serialize};

use crate::{
    error::{AppError, AppResult},
    file_version::{hash_bytes, hash_file},
    git,
    paths::{canonical_root, resolve_for_write},
    state::PersistentState,
    types::{TrashEntry, WorkspaceChangeOperation, WorkspacePathMove},
};

mod android_recovery;
#[cfg(target_os = "windows")]
mod conditional_write_recovery;

use android_recovery::{
    cleanup_restored_android_trash, cleanup_unapplied_android_trash, finish_applied_android_trash,
    restored_android_payload_exists,
};
#[cfg(target_os = "windows")]
use conditional_write_recovery::{reconcile_conditional_write_artifacts, ConditionalWriteRecovery};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct WorkspaceChangeIntent {
    pub(crate) path: String,
    pub(crate) operation: WorkspaceChangeOperation,
    pub(crate) version: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct WorkspaceCopyFile {
    pub(crate) source_path: String,
    pub(crate) destination_path: String,
    pub(crate) version: String,
}

impl WorkspaceChangeIntent {
    pub(crate) fn upsert(path: impl Into<String>, version: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            operation: WorkspaceChangeOperation::Upsert,
            version: Some(version.into()),
        }
    }

    pub(crate) fn delete(path: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            operation: WorkspaceChangeOperation::Delete,
            version: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum WorkspaceOperationPhase {
    Prepared,
    FilesystemApplied,
    BaselineCommitted,
}

impl WorkspaceOperationPhase {
    pub(crate) fn log_name(self) -> &'static str {
        match self {
            Self::Prepared => "prepared",
            Self::FilesystemApplied => "filesystemApplied",
            Self::BaselineCommitted => "baselineCommitted",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub(crate) enum WorkspaceOperationKind {
    WriteFile {
        path: String,
        version: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        previous_version: Option<String>,
        #[serde(default, skip_serializing_if = "is_false")]
        replace_existing: bool,
    },
    CreateFolder {
        path: String,
    },
    MoveEntry {
        source_path: String,
        destination_path: String,
        moved_files: Vec<WorkspacePathMove>,
    },
    DuplicateEntry {
        source_path: String,
        destination_path: String,
        copied_files: Vec<WorkspaceCopyFile>,
        directories: Vec<String>,
    },
    TrashEntry {
        path: String,
        trash_entry: Option<TrashEntry>,
    },
    RestoreTrash {
        trash_entry: TrashEntry,
    },
    EnableGit {
        baseline_paths: Vec<String>,
    },
}

impl WorkspaceOperationKind {
    pub(crate) fn log_action(&self) -> &'static str {
        match self {
            Self::WriteFile { .. } => "writeFile",
            Self::CreateFolder { .. } => "createFolder",
            Self::MoveEntry { .. } => "moveEntry",
            Self::DuplicateEntry { .. } => "duplicateEntry",
            Self::TrashEntry { .. } => "trashEntry",
            Self::RestoreTrash { .. } => "restoreTrash",
            Self::EnableGit { .. } => "enableGit",
        }
    }
}

fn is_false(value: &bool) -> bool {
    !*value
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PendingWorkspaceOperation {
    pub(crate) id: String,
    pub(crate) root: String,
    pub(crate) phase: WorkspaceOperationPhase,
    pub(crate) started_at: String,
    pub(crate) track_changes: bool,
    pub(crate) kind: WorkspaceOperationKind,
    pub(crate) changes: Vec<WorkspaceChangeIntent>,
}

pub(crate) fn execute_mutation<T>(
    root: &str,
    kind: WorkspaceOperationKind,
    changes: Vec<WorkspaceChangeIntent>,
    state: &PersistentState,
    recovered_result: T,
    apply: impl FnOnce(&PendingWorkspaceOperation) -> AppResult<T>,
) -> AppResult<T> {
    recover_pending_for_root(root, state)?;
    let operation = new_operation(root, kind, git::has_git_capability(root), changes);
    state.begin_workspace_operation(operation.clone())?;
    test_crash(TestCrashPoint::PreparedPersisted)?;

    let result = match apply(&operation) {
        Ok(result) => result,
        Err(error) => match reconcile_prepared_operation(&operation, state, Some(&error)) {
            Ok(PreparedReconciliation::Completed) => return Ok(recovered_result),
            Ok(PreparedReconciliation::Unapplied) => return Err(error),
            Err(recovery_error) => {
                state.record_workspace_operation_failure(&operation, &recovery_error, true);
                return Err(error);
            }
        },
    };
    test_crash(TestCrashPoint::FilesystemMutated)?;
    state.update_workspace_operation_phase(
        root,
        &operation.id,
        WorkspaceOperationPhase::FilesystemApplied,
    )?;
    test_crash(TestCrashPoint::FilesystemPhasePersisted)?;
    state.complete_workspace_operation(root, &operation.id)?;
    Ok(result)
}

pub(crate) fn enable_git(
    root: &str,
    baseline_paths: Vec<String>,
    state: &PersistentState,
) -> AppResult<()> {
    recover_pending_for_root(root, state)?;
    let operation = new_operation(
        root,
        WorkspaceOperationKind::EnableGit { baseline_paths },
        false,
        Vec::new(),
    );
    state.begin_workspace_operation(operation.clone())?;
    test_crash(TestCrashPoint::PreparedPersisted)?;
    match drive_enable_git(operation.clone(), state) {
        Ok(()) => Ok(()),
        Err(error) => {
            state.record_workspace_operation_failure(&operation, &error, false);
            Err(error)
        }
    }
}

pub(crate) fn recover_pending_for_root(root: &str, state: &PersistentState) -> AppResult<()> {
    let Some(operation) = state.try_pending_workspace_operation(root)? else {
        return Ok(());
    };
    let result = if matches!(&operation.kind, WorkspaceOperationKind::EnableGit { .. }) {
        drive_enable_git(operation.clone(), state)
    } else {
        match operation.phase {
            WorkspaceOperationPhase::Prepared => {
                reconcile_prepared_operation(&operation, state, None).map(|_| ())
            }
            WorkspaceOperationPhase::FilesystemApplied => {
                state.complete_workspace_operation(&operation.root, &operation.id)
            }
            WorkspaceOperationPhase::BaselineCommitted => {
                state.complete_workspace_operation(&operation.root, &operation.id)
            }
        }
    };
    if let Err(error) = &result {
        state.record_workspace_operation_failure(&operation, error, true);
    }
    result
}

pub(crate) fn recover_pending_operations(state: &PersistentState) {
    let Ok(operations) = state.pending_workspace_operations() else {
        return;
    };
    for operation in operations {
        if !Path::new(&operation.root).exists() {
            continue;
        }
        let Ok(_process_guard) = state.lock_workspace(&git::repository_lock_key(&operation.root))
        else {
            continue;
        };
        let _ = recover_pending_for_root(&operation.root, state);
    }
}

fn reconcile_prepared_operation(
    operation: &PendingWorkspaceOperation,
    state: &PersistentState,
    apply_error: Option<&AppError>,
) -> AppResult<PreparedReconciliation> {
    match &operation.kind {
        WorkspaceOperationKind::WriteFile {
            path,
            version,
            previous_version,
            replace_existing,
        } => {
            let root = canonical_root(&operation.root)?;
            let target = resolve_for_write(&root, path)?;
            #[cfg(target_os = "windows")]
            match reconcile_conditional_write_artifacts(
                operation,
                &target,
                version,
                previous_version.as_deref(),
            )? {
                ConditionalWriteRecovery::Applied => {
                    complete_recovered_filesystem_operation(operation, state)?;
                    return Ok(PreparedReconciliation::Completed);
                }
                ConditionalWriteRecovery::Unapplied => {
                    return finish_unapplied_operation(operation, state, apply_error);
                }
                ConditionalWriteRecovery::NoArtifacts => {}
            }
            if target.is_file() && hash_file(&target)? == *version {
                remove_operation_temporary(&target, &operation.id)?;
                complete_recovered_filesystem_operation(operation, state)?;
                return Ok(PreparedReconciliation::Completed);
            }

            let temporary = crate::paths::operation_copy_temporary_path(&target, &operation.id);
            if !temporary.is_file() {
                return finish_unapplied_operation(operation, state, apply_error);
            }
            if hash_file(&temporary)? != *version {
                fs::remove_file(&temporary)?;
                return finish_unapplied_operation(operation, state, apply_error);
            }

            let expected_target_is_present = previous_version.as_ref().is_some_and(|expected| {
                target.is_file() && hash_file(&target).is_ok_and(|actual| actual == *expected)
            });
            let expected_target_is_missing = previous_version.is_none() && !target.exists();
            if !*replace_existing && !expected_target_is_present && !expected_target_is_missing {
                if apply_error.is_some() {
                    fs::remove_file(&temporary)?;
                    return finish_unapplied_operation(operation, state, apply_error);
                }
                return Err(recovery_error(
                    operation,
                    "the staged write is complete but the destination no longer has its expected version",
                ));
            }

            crate::paths::publish_operation_temporary(
                &temporary,
                &target,
                previous_version.as_deref(),
                previous_version.is_none(),
                *replace_existing,
                &operation.id,
            )?;
            if !target.is_file() || hash_file(&target)? != *version {
                return Err(recovery_error(
                    operation,
                    "the staged write was published but its final version could not be verified",
                ));
            }
            complete_recovered_filesystem_operation(operation, state)?;
            Ok(PreparedReconciliation::Completed)
        }
        WorkspaceOperationKind::CreateFolder { path } => {
            let root = canonical_root(&operation.root)?;
            let target = resolve_for_write(&root, path)?;
            if target.is_dir() {
                complete_recovered_filesystem_operation(operation, state)?;
                Ok(PreparedReconciliation::Completed)
            } else {
                finish_unapplied_operation(operation, state, apply_error)
            }
        }
        WorkspaceOperationKind::MoveEntry {
            source_path,
            destination_path,
            ..
        } => {
            let root = canonical_root(&operation.root)?;
            let source = resolve_for_write(&root, source_path)?;
            let destination = resolve_for_write(&root, destination_path)?;
            if !source.exists() && destination.exists() {
                complete_recovered_filesystem_operation(operation, state)?;
                Ok(PreparedReconciliation::Completed)
            } else if source.exists() {
                finish_unapplied_operation(operation, state, apply_error)
            } else {
                Err(recovery_error(
                    operation,
                    "both the source and destination are missing",
                ))
            }
        }
        WorkspaceOperationKind::DuplicateEntry {
            source_path,
            destination_path,
            copied_files,
            directories,
        } => {
            let root = canonical_root(&operation.root)?;
            let source = resolve_for_write(&root, source_path)?;
            let destination = resolve_for_write(&root, destination_path)?;
            let has_owned_temporary = copied_files.iter().any(|copied| {
                resolve_for_write(&root, &copied.destination_path)
                    .ok()
                    .is_some_and(|destination| {
                        crate::paths::operation_copy_temporary_path(&destination, &operation.id)
                            .exists()
                    })
            });
            if !destination.exists() && !has_owned_temporary {
                return if source.exists() {
                    finish_unapplied_operation(operation, state, apply_error)
                } else {
                    Err(recovery_error(operation, "the copy source is missing"))
                };
            }
            if !source.exists() {
                return Err(recovery_error(operation, "the copy source is missing"));
            }
            for directory in directories {
                fs::create_dir_all(resolve_for_write(&root, directory)?)?;
            }
            for copied in copied_files {
                let source_file = resolve_for_write(&root, &copied.source_path)?;
                if !source_file.is_file() || hash_file(&source_file)? != copied.version {
                    return Err(recovery_error(
                        operation,
                        "the copy source changed before recovery",
                    ));
                }
                let destination_file = resolve_for_write(&root, &copied.destination_path)?;
                if destination_file.exists() {
                    if !destination_file.is_file()
                        || hash_file(&destination_file)? != copied.version
                    {
                        return Err(recovery_error(
                            operation,
                            "the copy destination conflicts with another file",
                        ));
                    }
                    continue;
                }
                if let Some(parent) = destination_file.parent() {
                    fs::create_dir_all(parent)?;
                }
                crate::paths::atomic_copy_for_operation(
                    &source_file,
                    &destination_file,
                    &operation.id,
                )?;
            }
            complete_recovered_filesystem_operation(operation, state)?;
            Ok(PreparedReconciliation::Completed)
        }
        WorkspaceOperationKind::TrashEntry { path, trash_entry } => {
            let root = canonical_root(&operation.root)?;
            let source = resolve_for_write(&root, path)?;
            if source.exists() {
                cleanup_unapplied_android_trash(state, trash_entry)?;
                finish_unapplied_operation(operation, state, apply_error)
            } else {
                finish_applied_android_trash(state, trash_entry)?;
                complete_recovered_filesystem_operation(operation, state)?;
                Ok(PreparedReconciliation::Completed)
            }
        }
        WorkspaceOperationKind::RestoreTrash { trash_entry } => {
            let root = canonical_root(&operation.root)?;
            let destination = resolve_for_write(&root, &trash_entry.original_path)?;
            if restored_android_payload_exists(state, trash_entry) {
                finish_unapplied_operation(operation, state, apply_error)
            } else if destination.exists() {
                cleanup_restored_android_trash(state, trash_entry)?;
                complete_recovered_filesystem_operation(operation, state)?;
                Ok(PreparedReconciliation::Completed)
            } else {
                Err(recovery_error(
                    operation,
                    "neither the restored entry nor its trash payload exists",
                ))
            }
        }
        WorkspaceOperationKind::EnableGit { .. } => {
            drive_enable_git(operation.clone(), state)?;
            Ok(PreparedReconciliation::Completed)
        }
    }
}

fn remove_operation_temporary(target: &Path, operation_id: &str) -> AppResult<()> {
    let temporary = crate::paths::operation_copy_temporary_path(target, operation_id);
    if temporary.exists() {
        fs::remove_file(temporary)?;
    }
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PreparedReconciliation {
    Completed,
    Unapplied,
}

fn finish_unapplied_operation(
    operation: &PendingWorkspaceOperation,
    state: &PersistentState,
    apply_error: Option<&AppError>,
) -> AppResult<PreparedReconciliation> {
    if let Some(error) = apply_error {
        state.fail_workspace_operation(&operation.root, &operation.id, error)?;
    } else {
        state.cancel_workspace_operation(&operation.root, &operation.id)?;
    }
    Ok(PreparedReconciliation::Unapplied)
}

fn complete_recovered_filesystem_operation(
    operation: &PendingWorkspaceOperation,
    state: &PersistentState,
) -> AppResult<()> {
    state.update_workspace_operation_phase(
        &operation.root,
        &operation.id,
        WorkspaceOperationPhase::FilesystemApplied,
    )?;
    state.complete_workspace_operation(&operation.root, &operation.id)
}

fn drive_enable_git(
    mut operation: PendingWorkspaceOperation,
    state: &PersistentState,
) -> AppResult<()> {
    let WorkspaceOperationKind::EnableGit { baseline_paths } = &operation.kind else {
        return Err(recovery_error(
            &operation,
            "the operation kind is not Git initialization",
        ));
    };
    if operation.phase == WorkspaceOperationPhase::Prepared {
        if !git::has_git_capability(&operation.root) {
            git::initialize_repository(&operation.root)?;
            test_crash(TestCrashPoint::FilesystemMutated)?;
        }
        operation.phase = WorkspaceOperationPhase::FilesystemApplied;
        state.update_workspace_operation_phase(&operation.root, &operation.id, operation.phase)?;
        test_crash(TestCrashPoint::FilesystemPhasePersisted)?;
    }
    if operation.phase == WorkspaceOperationPhase::FilesystemApplied {
        git::commit_workspace_baseline(&operation.root, baseline_paths, &operation.id)?;
        test_crash(TestCrashPoint::BaselineCreated)?;
        operation.phase = WorkspaceOperationPhase::BaselineCommitted;
        state.update_workspace_operation_phase(&operation.root, &operation.id, operation.phase)?;
        test_crash(TestCrashPoint::BaselinePhasePersisted)?;
    }
    state.complete_workspace_operation(&operation.root, &operation.id)
}

fn new_operation(
    root: &str,
    kind: WorkspaceOperationKind,
    track_changes: bool,
    changes: Vec<WorkspaceChangeIntent>,
) -> PendingWorkspaceOperation {
    let started_at = Utc::now().to_rfc3339();
    let seed = format!("{root}\n{kind:?}\n{started_at}\n{}", std::process::id());
    PendingWorkspaceOperation {
        id: hash_bytes(seed.as_bytes())[..24].to_owned(),
        root: root.to_owned(),
        phase: WorkspaceOperationPhase::Prepared,
        started_at,
        track_changes,
        kind,
        changes,
    }
}

fn recovery_error(operation: &PendingWorkspaceOperation, detail: &str) -> AppError {
    AppError::Message(format!(
        "Workspace operation '{}' for '{}' could not be recovered because {detail}.",
        operation.id, operation.root
    ))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TestCrashPoint {
    PreparedPersisted,
    FilesystemMutated,
    CopyFileApplied,
    FilesystemPhasePersisted,
    BaselineCreated,
    BaselinePhasePersisted,
}

#[cfg(test)]
thread_local! {
    static TEST_CRASH_POINT: std::cell::Cell<Option<TestCrashPoint>> = const { std::cell::Cell::new(None) };
}

#[cfg(test)]
pub(crate) fn inject_crash_at(point: TestCrashPoint) {
    TEST_CRASH_POINT.set(Some(point));
}

#[cfg(test)]
pub(crate) fn test_abrupt_stop(point: TestCrashPoint) {
    let should_stop = TEST_CRASH_POINT.get() == Some(point);
    if should_stop {
        TEST_CRASH_POINT.set(None);
        panic!("Injected abrupt process stop after {point:?}.");
    }
}

#[cfg(not(test))]
pub(crate) fn test_abrupt_stop(_point: TestCrashPoint) {}

#[cfg(test)]
fn test_crash(point: TestCrashPoint) -> AppResult<()> {
    let should_crash = TEST_CRASH_POINT.get() == Some(point);
    if should_crash {
        TEST_CRASH_POINT.set(None);
        Err(AppError::Message(format!(
            "Injected process stop after {point:?}."
        )))
    } else {
        Ok(())
    }
}

#[cfg(not(test))]
fn test_crash(_point: TestCrashPoint) -> AppResult<()> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::{collections::BTreeSet, fs, path::Path};

    use git2::Repository;
    use tempfile::TempDir;

    use super::*;
    use crate::{
        documents,
        types::{
            OperationLogOutcome, SaveDocumentRequest, SaveWorkspaceConfigRequest, TextEncoding,
            WorkspaceChangeOperation, WorkspaceConfig,
        },
        workspace,
    };

    fn save_request(root: &str, content: &str) -> SaveDocumentRequest {
        let opened = documents::read_document(root, "note.md").unwrap();
        SaveDocumentRequest {
            root: root.to_owned(),
            path: opened.path,
            content: content.to_owned(),
            expected_sha256: Some(opened.sha256),
            expected_missing: false,
            encoding: TextEncoding::Utf8,
        }
    }

    #[test]
    fn prepared_write_without_a_filesystem_effect_is_cancelled_after_restart() {
        let workspace = TempDir::new().unwrap();
        let app_data = TempDir::new().unwrap();
        let root = workspace.path().to_string_lossy().into_owned();
        fs::write(workspace.path().join("note.md"), "before\n").unwrap();
        git::initialize_repository(&root).unwrap();
        let state = PersistentState::load(app_data.path()).unwrap();

        inject_crash_at(TestCrashPoint::PreparedPersisted);
        assert!(documents::save_document(save_request(&root, "after\n"), &state).is_err());
        assert_eq!(
            fs::read_to_string(workspace.path().join("note.md")).unwrap(),
            "before\n"
        );
        assert!(state.workspace_changes(&root).is_empty());
        assert!(state.pending_workspace_operation(&root).is_some());
        drop(state);

        let recovered = PersistentState::load(app_data.path()).unwrap();
        recover_pending_for_root(&root, &recovered).unwrap();

        assert!(recovered.pending_workspace_operation(&root).is_none());
        assert!(recovered.workspace_changes(&root).is_empty());
        assert_eq!(
            fs::read_to_string(workspace.path().join("note.md")).unwrap(),
            "before\n"
        );
    }

    #[test]
    fn prepared_create_publishes_a_complete_operation_owned_staging_file_after_restart() {
        let workspace = TempDir::new().unwrap();
        let app_data = TempDir::new().unwrap();
        let root = workspace.path().to_string_lossy().into_owned();
        let state = PersistentState::load(app_data.path()).unwrap();
        let intended = b"complete imported bytes";
        let version = hash_bytes(intended);
        let operation = new_operation(
            &root,
            WorkspaceOperationKind::WriteFile {
                path: "imported.md".to_owned(),
                version: version.clone(),
                previous_version: None,
                replace_existing: false,
            },
            false,
            vec![WorkspaceChangeIntent::upsert("imported.md", version)],
        );
        let destination = workspace.path().join("imported.md");
        let temporary = crate::paths::operation_copy_temporary_path(&destination, &operation.id);
        state.begin_workspace_operation(operation).unwrap();
        fs::write(&temporary, intended).unwrap();

        recover_pending_for_root(&root, &state).unwrap();

        assert_eq!(fs::read(&destination).unwrap(), intended);
        assert!(!temporary.exists());
        assert!(state.pending_workspace_operation(&root).is_none());
    }

    #[test]
    fn prepared_create_discards_a_partial_staging_file_without_exposing_a_document() {
        let workspace = TempDir::new().unwrap();
        let app_data = TempDir::new().unwrap();
        let root = workspace.path().to_string_lossy().into_owned();
        let state = PersistentState::load(app_data.path()).unwrap();
        let version = hash_bytes(b"complete imported bytes");
        let operation = new_operation(
            &root,
            WorkspaceOperationKind::WriteFile {
                path: "imported.md".to_owned(),
                version: version.clone(),
                previous_version: None,
                replace_existing: false,
            },
            false,
            vec![WorkspaceChangeIntent::upsert("imported.md", version)],
        );
        let destination = workspace.path().join("imported.md");
        let temporary = crate::paths::operation_copy_temporary_path(&destination, &operation.id);
        state.begin_workspace_operation(operation).unwrap();
        fs::write(&temporary, b"partial").unwrap();

        recover_pending_for_root(&root, &state).unwrap();

        assert!(!destination.exists());
        assert!(!temporary.exists());
        assert!(state.pending_workspace_operation(&root).is_none());
    }

    #[test]
    fn apply_error_after_the_expected_effect_returns_the_recovered_result_once() {
        let workspace = TempDir::new().unwrap();
        let app_data = TempDir::new().unwrap();
        let root = workspace.path().to_string_lossy().into_owned();
        git::initialize_repository(&root).unwrap();
        let state = PersistentState::load(app_data.path()).unwrap();
        let expected = hash_bytes(b"recovered\n");

        let result = execute_mutation(
            &root,
            WorkspaceOperationKind::WriteFile {
                path: "recovered.md".to_owned(),
                version: expected.clone(),
                previous_version: None,
                replace_existing: false,
            },
            vec![WorkspaceChangeIntent::upsert("recovered.md", &expected)],
            &state,
            "recovered result",
            |_| {
                fs::write(workspace.path().join("recovered.md"), "recovered\n").unwrap();
                Err(AppError::Message(
                    "the writer reported an error after replacement".to_owned(),
                ))
            },
        )
        .unwrap();

        assert_eq!(result, "recovered result");
        assert!(state.pending_workspace_operation(&root).is_none());
        assert_eq!(state.workspace_changes(&root).len(), 1);
        let terminal = state
            .operation_log(20)
            .unwrap()
            .into_iter()
            .filter(|entry| {
                entry.action == "writeFile"
                    && matches!(
                        entry.outcome,
                        OperationLogOutcome::Succeeded
                            | OperationLogOutcome::Failed
                            | OperationLogOutcome::Cancelled
                    )
            })
            .map(|entry| entry.outcome)
            .collect::<Vec<_>>();
        assert_eq!(terminal, vec![OperationLogOutcome::Succeeded]);
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn prepared_conditional_write_removes_its_verified_backup_after_restart() {
        let workspace = TempDir::new().unwrap();
        let app_data = TempDir::new().unwrap();
        let root = workspace.path().to_string_lossy().into_owned();
        git::initialize_repository(&root).unwrap();
        let state = PersistentState::load(app_data.path()).unwrap();
        let previous = hash_bytes(b"before\n");
        let saved = hash_bytes(b"after\n");
        let operation = new_operation(
            &root,
            WorkspaceOperationKind::WriteFile {
                path: "note.md".to_owned(),
                version: saved.clone(),
                previous_version: Some(previous),
                replace_existing: false,
            },
            true,
            vec![WorkspaceChangeIntent::upsert("note.md", &saved)],
        );
        state.begin_workspace_operation(operation.clone()).unwrap();
        let target = workspace.path().join("note.md");
        fs::write(&target, b"after\n").unwrap();
        let (backup, rejected) =
            crate::paths::conditional_write_artifact_paths(&target, &operation.id).unwrap();
        fs::write(&backup, b"before\n").unwrap();

        recover_pending_for_root(&root, &state).unwrap();

        assert_eq!(fs::read(&target).unwrap(), b"after\n");
        assert!(!backup.exists());
        assert!(!rejected.exists());
        assert!(state.pending_workspace_operation(&root).is_none());
        assert_eq!(state.workspace_changes(&root).len(), 1);
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn prepared_conditional_write_restores_a_release_race_winner_after_restart() {
        let workspace = TempDir::new().unwrap();
        let app_data = TempDir::new().unwrap();
        let root = workspace.path().to_string_lossy().into_owned();
        git::initialize_repository(&root).unwrap();
        let state = PersistentState::load(app_data.path()).unwrap();
        let previous = hash_bytes(b"before\n");
        let saved = hash_bytes(b"marktree\n");
        let operation = new_operation(
            &root,
            WorkspaceOperationKind::WriteFile {
                path: "note.md".to_owned(),
                version: saved.clone(),
                previous_version: Some(previous),
                replace_existing: false,
            },
            true,
            vec![WorkspaceChangeIntent::upsert("note.md", &saved)],
        );
        state.begin_workspace_operation(operation.clone()).unwrap();
        let target = workspace.path().join("note.md");
        fs::write(&target, b"marktree\n").unwrap();
        let (backup, rejected) =
            crate::paths::conditional_write_artifact_paths(&target, &operation.id).unwrap();
        fs::write(&backup, b"external winner\n").unwrap();

        recover_pending_for_root(&root, &state).unwrap();

        assert_eq!(fs::read(&target).unwrap(), b"external winner\n");
        assert!(!backup.exists());
        assert!(!rejected.exists());
        assert!(state.pending_workspace_operation(&root).is_none());
        assert!(state.workspace_changes(&root).is_empty());
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn prepared_conditional_write_finishes_an_interrupted_rollback_after_restart() {
        let workspace = TempDir::new().unwrap();
        let app_data = TempDir::new().unwrap();
        let root = workspace.path().to_string_lossy().into_owned();
        git::initialize_repository(&root).unwrap();
        let state = PersistentState::load(app_data.path()).unwrap();
        let previous = hash_bytes(b"before\n");
        let saved = hash_bytes(b"marktree\n");
        let operation = new_operation(
            &root,
            WorkspaceOperationKind::WriteFile {
                path: "note.md".to_owned(),
                version: saved.clone(),
                previous_version: Some(previous),
                replace_existing: false,
            },
            true,
            vec![WorkspaceChangeIntent::upsert("note.md", &saved)],
        );
        state.begin_workspace_operation(operation.clone()).unwrap();
        let target = workspace.path().join("note.md");
        fs::write(&target, b"external winner\n").unwrap();
        let (backup, rejected) =
            crate::paths::conditional_write_artifact_paths(&target, &operation.id).unwrap();
        fs::write(&rejected, b"marktree\n").unwrap();

        recover_pending_for_root(&root, &state).unwrap();

        assert_eq!(fs::read(&target).unwrap(), b"external winner\n");
        assert!(!backup.exists());
        assert!(!rejected.exists());
        assert!(state.pending_workspace_operation(&root).is_none());
        assert!(state.workspace_changes(&root).is_empty());
    }

    #[test]
    fn duplicate_recovery_reuses_a_partial_operation_owned_temporary_file() {
        let workspace = TempDir::new().unwrap();
        let app_data = TempDir::new().unwrap();
        let root = workspace.path().to_string_lossy().into_owned();
        let state = PersistentState::load(app_data.path()).unwrap();
        let source = workspace.path().join("source.md");
        let destination = workspace.path().join("copy.md");
        fs::write(&source, b"complete source").unwrap();
        let version = hash_bytes(b"complete source");
        let operation = new_operation(
            &root,
            WorkspaceOperationKind::DuplicateEntry {
                source_path: "source.md".to_owned(),
                destination_path: "copy.md".to_owned(),
                copied_files: vec![WorkspaceCopyFile {
                    source_path: "source.md".to_owned(),
                    destination_path: "copy.md".to_owned(),
                    version: version.clone(),
                }],
                directories: Vec::new(),
            },
            false,
            vec![WorkspaceChangeIntent::upsert("copy.md", version)],
        );
        let temporary = crate::paths::operation_copy_temporary_path(&destination, &operation.id);
        state.begin_workspace_operation(operation).unwrap();
        fs::write(&temporary, b"partial").unwrap();

        recover_pending_for_root(&root, &state).unwrap();

        assert_eq!(fs::read(destination).unwrap(), b"complete source");
        assert!(!temporary.exists());
        assert!(state.pending_workspace_operation(&root).is_none());
    }

    #[test]
    fn apply_error_without_an_effect_finishes_as_one_failed_operation() {
        let workspace = TempDir::new().unwrap();
        let app_data = TempDir::new().unwrap();
        let root = workspace.path().to_string_lossy().into_owned();
        let state = PersistentState::load(app_data.path()).unwrap();

        let result = execute_mutation(
            &root,
            WorkspaceOperationKind::CreateFolder {
                path: "never-created".to_owned(),
            },
            Vec::new(),
            &state,
            (),
            |_| {
                Err(AppError::Message(
                    "the directory could not be created".to_owned(),
                ))
            },
        );

        assert!(result.is_err());
        assert!(state.pending_workspace_operation(&root).is_none());
        assert!(!workspace.path().join("never-created").exists());
        let terminal = state
            .operation_log(20)
            .unwrap()
            .into_iter()
            .filter(|entry| {
                entry.action == "createFolder"
                    && matches!(
                        entry.outcome,
                        OperationLogOutcome::Succeeded
                            | OperationLogOutcome::Failed
                            | OperationLogOutcome::Cancelled
                    )
            })
            .map(|entry| entry.outcome)
            .collect::<Vec<_>>();
        assert_eq!(terminal, vec![OperationLogOutcome::Failed]);
    }

    #[test]
    fn applied_write_and_its_manifest_become_visible_together_after_restart() {
        let workspace = TempDir::new().unwrap();
        let app_data = TempDir::new().unwrap();
        let root = workspace.path().to_string_lossy().into_owned();
        fs::write(workspace.path().join("note.md"), "before\n").unwrap();
        git::initialize_repository(&root).unwrap();
        let state = PersistentState::load(app_data.path()).unwrap();

        inject_crash_at(TestCrashPoint::FilesystemMutated);
        assert!(documents::save_document(save_request(&root, "after\n"), &state).is_err());
        assert_eq!(
            fs::read_to_string(workspace.path().join("note.md")).unwrap(),
            "after\n"
        );
        assert!(state.workspace_changes(&root).is_empty());
        assert!(state.pending_workspace_operation(&root).is_some());
        drop(state);

        let recovered = PersistentState::load(app_data.path()).unwrap();
        recover_pending_for_root(&root, &recovered).unwrap();
        let changes = recovered.workspace_changes(&root);

        assert!(recovered.pending_workspace_operation(&root).is_none());
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].path, "note.md");
        assert_eq!(changes[0].operation, WorkspaceChangeOperation::Upsert);
        assert_eq!(changes[0].version, Some(hash_bytes(b"after\n")));
        let log = recovered.operation_log(20).unwrap();
        assert!(log.iter().any(|entry| {
            entry.action == "writeFile"
                && entry.root.as_deref() == Some(root.as_str())
                && entry.operation_id.is_some()
                && entry.outcome == OperationLogOutcome::Succeeded
        }));
    }

    #[test]
    fn recovered_directory_move_commits_its_complete_manifest_batch() {
        let workspace = TempDir::new().unwrap();
        let app_data = TempDir::new().unwrap();
        let root = workspace.path().to_string_lossy().into_owned();
        fs::create_dir(workspace.path().join("notes")).unwrap();
        fs::write(workspace.path().join("notes/a.md"), "A\n").unwrap();
        fs::write(workspace.path().join("notes/b.md"), "B\n").unwrap();
        git::initialize_repository(&root).unwrap();
        let state = PersistentState::load(app_data.path()).unwrap();

        inject_crash_at(TestCrashPoint::FilesystemMutated);
        assert!(documents::move_entry(&root, "notes", "archive/notes", &state).is_err());
        assert!(!workspace.path().join("notes").exists());
        assert!(workspace.path().join("archive/notes/a.md").exists());
        assert!(state.workspace_changes(&root).is_empty());
        drop(state);

        let recovered = PersistentState::load(app_data.path()).unwrap();
        recover_pending_for_root(&root, &recovered).unwrap();
        let changes = recovered.workspace_changes(&root);
        let actual = changes
            .iter()
            .map(|change| (change.path.clone(), change.operation.clone()))
            .collect::<BTreeSet<_>>();
        let expected = BTreeSet::from([
            (
                "archive/notes/a.md".to_owned(),
                WorkspaceChangeOperation::Upsert,
            ),
            (
                "archive/notes/b.md".to_owned(),
                WorkspaceChangeOperation::Upsert,
            ),
            ("notes/a.md".to_owned(), WorkspaceChangeOperation::Delete),
            ("notes/b.md".to_owned(), WorkspaceChangeOperation::Delete),
        ]);

        assert_eq!(actual, expected);
        assert!(recovered.pending_workspace_operation(&root).is_none());
    }

    #[test]
    fn interrupted_directory_duplicate_resumes_from_its_real_copy_plan_after_restart() {
        let workspace = TempDir::new().unwrap();
        let app_data = TempDir::new().unwrap();
        let root = workspace.path().to_string_lossy().into_owned();
        fs::create_dir(workspace.path().join("source")).unwrap();
        fs::create_dir(workspace.path().join("source/empty")).unwrap();
        fs::write(workspace.path().join("source/a.md"), "A\n").unwrap();
        fs::write(workspace.path().join("source/b.md"), "B\n").unwrap();
        git::initialize_repository(&root).unwrap();
        let state = PersistentState::load(app_data.path()).unwrap();

        inject_crash_at(TestCrashPoint::CopyFileApplied);
        let stopped = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            documents::duplicate_entry(&root, "source", "source copy", &state)
        }));
        assert!(stopped.is_err());
        assert!(state.pending_workspace_operation(&root).is_some());
        assert!(workspace.path().join("source copy").is_dir());
        assert_eq!(
            fs::read_dir(workspace.path().join("source copy"))
                .unwrap()
                .filter_map(Result::ok)
                .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_file()))
                .count(),
            1
        );
        assert!(state.workspace_changes(&root).is_empty());
        drop(state);

        let recovered = PersistentState::load(app_data.path()).unwrap();
        recover_pending_for_root(&root, &recovered).unwrap();
        assert_eq!(
            fs::read_to_string(workspace.path().join("source copy/a.md")).unwrap(),
            "A\n"
        );
        assert_eq!(
            fs::read_to_string(workspace.path().join("source copy/b.md")).unwrap(),
            "B\n"
        );
        assert!(workspace.path().join("source copy/empty").is_dir());
        let changes = recovered.workspace_changes(&root);
        assert_eq!(changes.len(), 2);
        assert!(changes.iter().all(|change| {
            change.operation == WorkspaceChangeOperation::Upsert
                && change.path.starts_with("source copy/")
        }));
        assert!(recovered.pending_workspace_operation(&root).is_none());
    }

    #[test]
    fn git_enable_recovery_creates_one_complete_baseline_at_every_durable_phase() {
        for crash_point in [
            TestCrashPoint::FilesystemMutated,
            TestCrashPoint::FilesystemPhasePersisted,
            TestCrashPoint::BaselineCreated,
            TestCrashPoint::BaselinePhasePersisted,
        ] {
            let workspace = TempDir::new().unwrap();
            let app_data = TempDir::new().unwrap();
            let root = workspace.path().to_string_lossy().into_owned();
            fs::write(workspace.path().join("note.md"), "# Note\n").unwrap();
            let state = PersistentState::load(app_data.path()).unwrap();
            documents::save_workspace_config(
                SaveWorkspaceConfigRequest {
                    root: root.clone(),
                    config: WorkspaceConfig {
                        assets_dir: "media".to_owned(),
                        ignore_rules: Vec::new(),
                    },
                    expected_sha256: None,
                    expected_missing: true,
                },
                &state,
            )
            .unwrap();
            assert_eq!(
                workspace::preview_git_baseline(&root).unwrap().file_count,
                2
            );

            inject_crash_at(crash_point);
            assert!(workspace::enable_git(&root, &state).is_err());
            assert!(state.pending_workspace_operation(&root).is_some());
            drop(state);

            let recovered = PersistentState::load(app_data.path()).unwrap();
            recover_pending_for_root(&root, &recovered).unwrap();
            let repository = Repository::open(&root).unwrap();
            let head = repository.head().unwrap().peel_to_commit().unwrap();
            let tree = head.tree().unwrap();

            assert!(
                tree.get_path(Path::new("note.md")).is_ok(),
                "{crash_point:?}"
            );
            assert!(
                tree.get_path(Path::new(".marktree/config.json")).is_ok(),
                "{crash_point:?}"
            );
            assert_eq!(head.parent_count(), 0, "{crash_point:?}");
            assert!(recovered.pending_workspace_operation(&root).is_none());
            assert!(recovered.workspace_changes(&root).is_empty());
        }
    }
}
