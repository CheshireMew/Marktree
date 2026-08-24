use chrono::Utc;
use git2::{
    build::CheckoutBuilder, Oid, Repository, RepositoryState, ResetType, StashApplyOptions,
};

use super::{
    conflicts::{
        archive_operation_recoveries, capture_conflicts, continue_rebase, rebase_onto_upstream,
    },
    remote::{
        ensure_current_branch_upstream, fetch_remote, push_current_branch, UpstreamDisposition,
    },
    repository::{current_branch, status_snapshot, workdir_string},
    stash::{
        find_operation_stash, operation_stash_index, stash_if_needed, stash_snapshot_is_applied,
        stash_touched_paths_are_dirty,
    },
    sync_commit::{
        align_index_paths_to_head, commit_prepared_workspace_changes, find_operation_commit,
        prepare_workspace_changes, tracked_workspace_paths,
    },
};
use crate::{
    error::{AppError, AppResult},
    file_version::hash_bytes,
    state::PersistentState,
    types::{
        ConflictRecord, CredentialRecord, GitOperationKind, GitOperationPhase, GitStatusSnapshot,
        OperationLogOutcome, PendingGitOperation, SyncPlan, SyncResult, SyncStage,
    },
};

pub fn pull_rebase(
    root: &str,
    credential: Option<CredentialRecord>,
    app_state: &PersistentState,
) -> AppResult<SyncResult> {
    start_or_resume_git_operation(root, GitOperationKind::Pull, credential, app_state)
}

pub fn sync_plan(root: &str, app_state: &PersistentState) -> AppResult<SyncPlan> {
    let repo = super::repository::open_exact_repository(root)?;
    let changed_paths = tracked_workspace_paths(&repo, &app_state.try_workspace_changes(root)?)?;
    let remote_url = super::remote::remote_url(&repo);
    Ok(SyncPlan {
        root: workdir_string(&repo)?,
        branch: current_branch(&repo),
        changed_paths,
        can_push: remote_url.is_some() && current_branch(&repo).is_some(),
        remote_url,
    })
}

pub fn sync_workspace_changes(
    root: &str,
    credential: Option<CredentialRecord>,
    app_state: &PersistentState,
) -> AppResult<SyncResult> {
    start_or_resume_git_operation(root, GitOperationKind::Sync, credential, app_state)
}

pub fn pending_git_operation(
    root: &str,
    app_state: &PersistentState,
) -> AppResult<Option<PendingGitOperation>> {
    app_state.try_pending_git_operation(root)
}

pub fn resume_git_operation(
    root: &str,
    credential: Option<CredentialRecord>,
    app_state: &PersistentState,
) -> AppResult<SyncResult> {
    let operation = app_state.try_pending_git_operation(root)?.ok_or_else(|| {
        AppError::Message("There is no unfinished Git operation to resume.".to_owned())
    })?;
    if operation.aborting {
        abort_git_operation(root, app_state)?;
        return Ok(SyncResult {
            committed: operation.committed,
            commit_id: operation.commit_id,
            pulled: operation.pulled,
            pushed: false,
            changed_paths: operation.changed_paths,
            conflicts: Vec::new(),
            failure_stage: None,
            error: None,
        });
    }
    drive_git_operation(operation, credential, app_state)
}

pub fn abort_git_operation(
    root: &str,
    app_state: &PersistentState,
) -> AppResult<GitStatusSnapshot> {
    let mut operation = app_state.try_pending_git_operation(root)?.ok_or_else(|| {
        AppError::Message("There is no unfinished Git operation to abort.".to_owned())
    })?;
    if !operation.aborting {
        if operation.pushed
            || (operation.kind == GitOperationKind::Sync
                && matches!(
                    operation.phase,
                    GitOperationPhase::Push | GitOperationPhase::Finalize
                ))
        {
            return Err(AppError::Message(
                "This sync may already have reached the remote; resume it to finish safely."
                    .to_owned(),
            ));
        }
        operation.aborting = true;
        app_state.update_git_operation(operation.clone())?;
    }
    let mut repo = super::repository::open_exact_repository(root)?;

    if operation.stash_oid.is_some() && !operation.stash_applied {
        if operation.stash_apply_started {
            if repo.index()?.has_conflicts() {
                return Err(AppError::Message(
                    "The interrupted abort left Git conflicts while restoring the exact \
                     working-tree snapshot. Resolve or safeguard them before retrying."
                        .to_owned(),
                ));
            }
            if stash_snapshot_is_applied(&repo, &operation)? {
                operation.stash_apply_started = false;
                operation.stash_applied = true;
                app_state.update_git_operation(operation.clone())?;
            } else if stash_touched_paths_are_dirty(&repo, &operation)? {
                return Err(AppError::Message(
                    "Files covered by the interrupted abort changed before Marktree could \
                     confirm the restored snapshot. The operation and exact Git stash were \
                     preserved."
                        .to_owned(),
                ));
            }
        } else {
            reset_operation_to_original_head(&mut repo, &operation)?;
            operation.stash_apply_started = true;
            app_state.update_git_operation(operation.clone())?;
        }
        if !operation.stash_applied {
            let stash_index = operation_stash_index(&mut repo, &operation)?;
            let mut options = StashApplyOptions::new();
            options.reinstantiate_index();
            repo.stash_apply(stash_index, Some(&mut options))?;
            operation.stash_apply_started = false;
            operation.stash_applied = true;
            app_state.update_git_operation(operation.clone())?;
        }
    } else if operation.stash_oid.is_none() && !operation.stash_applied {
        reset_operation_to_original_head(&mut repo, &operation)?;
    }

    if operation.stash_oid.is_some() {
        if let Ok(stash_index) = operation_stash_index(&mut repo, &operation) {
            repo.stash_drop(stash_index)?;
        } else if !operation.stash_applied {
            return Err(AppError::Message(
                "The exact working-tree snapshot for this Git operation is missing.".to_owned(),
            ));
        }
        operation.stash_oid = None;
        operation.stash_apply_started = false;
        operation.stash_applied = false;
        app_state.update_git_operation(operation.clone())?;
    }
    repo.cleanup_state()?;
    operation.phase = GitOperationPhase::Finalize;
    operation.original_head_oid = None;
    operation.pulled = false;
    app_state.update_git_operation(operation.clone())?;
    archive_operation_recoveries(&operation, app_state)?;
    app_state.finish_git_operation(root, &operation.id, OperationLogOutcome::Cancelled, None)?;
    status_snapshot(&repo)
}

fn reset_operation_to_original_head(
    repo: &mut Repository,
    operation: &PendingGitOperation,
) -> AppResult<()> {
    if repo.state() == RepositoryState::RebaseMerge {
        let mut rebase = repo.open_rebase(None)?;
        rebase.abort()?;
        drop(rebase);
    }
    if let Some(oid) = operation
        .original_head_oid
        .as_deref()
        .and_then(|value| Oid::from_str(value).ok())
    {
        let object = repo.find_object(oid, None)?;
        let mut checkout = CheckoutBuilder::new();
        checkout.force();
        repo.reset(&object, ResetType::Hard, Some(&mut checkout))?;
    }
    repo.cleanup_state()?;
    Ok(())
}

fn start_or_resume_git_operation(
    root: &str,
    kind: GitOperationKind,
    credential: Option<CredentialRecord>,
    app_state: &PersistentState,
) -> AppResult<SyncResult> {
    if let Some(operation) = app_state.try_pending_git_operation(root)? {
        if operation.kind != kind {
            return Err(AppError::GitOperationPending {
                root: operation.root,
            });
        }
        return drive_git_operation(operation, credential, app_state);
    }
    let workspace_changes = if kind == GitOperationKind::Sync {
        app_state.try_workspace_changes(root)?
    } else {
        Vec::new()
    };
    let seed = format!(
        "{root}\n{kind:?}\n{}",
        Utc::now().timestamp_nanos_opt().unwrap_or_default()
    );
    let operation = PendingGitOperation {
        id: hash_bytes(seed.as_bytes())[..20].to_owned(),
        root: root.to_owned(),
        kind,
        phase: GitOperationPhase::Prepare,
        started_at: Utc::now().to_rfc3339(),
        workspace_changes,
        changed_paths: Vec::new(),
        committed: false,
        commit_id: None,
        pulled: false,
        pushed: false,
        original_head_oid: None,
        stash_oid: None,
        aborting: false,
        stash_apply_started: false,
        stash_applied: false,
    };
    app_state.begin_git_operation(operation.clone())?;
    drive_git_operation(operation, credential, app_state)
}

fn drive_git_operation(
    operation: PendingGitOperation,
    credential: Option<CredentialRecord>,
    app_state: &PersistentState,
) -> AppResult<SyncResult> {
    let root = operation.root.clone();
    let id = operation.id.clone();
    let result = drive_git_operation_inner(operation, credential, app_state);
    if let Err(error) = &result {
        if let Some(operation) = app_state
            .try_pending_git_operation(&root)?
            .filter(|operation| operation.id == id)
        {
            app_state.record_git_operation_failure(&operation, error);
        }
    }
    result
}

fn drive_git_operation_inner(
    mut operation: PendingGitOperation,
    credential: Option<CredentialRecord>,
    app_state: &PersistentState,
) -> AppResult<SyncResult> {
    loop {
        let mut repo = match super::repository::open_exact_repository(&operation.root) {
            Ok(repo) => repo,
            Err(error) => {
                return Ok(operation_failure(
                    &operation,
                    SyncStage::Prepare,
                    error,
                    app_state,
                ))
            }
        };
        match operation.phase {
            GitOperationPhase::Prepare => {
                if operation.kind == GitOperationKind::Sync {
                    let paths = match tracked_workspace_paths(&repo, &operation.workspace_changes) {
                        Ok(paths) => paths,
                        Err(error) => {
                            let error: AppError = error;
                            return finish_operation_failure(
                                &operation,
                                SyncStage::Prepare,
                                &error,
                                app_state,
                            );
                        }
                    };
                    operation.changed_paths = paths.clone();
                    operation.phase = if paths.is_empty() {
                        GitOperationPhase::Fetch
                    } else {
                        GitOperationPhase::Commit
                    };
                } else {
                    operation.phase = GitOperationPhase::Fetch;
                }
                app_state.update_git_operation(operation.clone())?;
            }
            GitOperationPhase::Commit => {
                if operation.kind != GitOperationKind::Sync {
                    return Ok(operation_failure(
                        &operation,
                        SyncStage::Commit,
                        AppError::Message(
                            "Only a Marktree sync can resume from the commit phase.".to_owned(),
                        ),
                        app_state,
                    ));
                }
                if !operation.committed {
                    let paths = operation.changed_paths.clone();
                    if paths.is_empty() {
                        return Ok(operation_failure(
                            &operation,
                            SyncStage::Commit,
                            AppError::Message(
                                "The pending sync commit has no recorded paths.".to_owned(),
                            ),
                            app_state,
                        ));
                    }
                    let oid = match find_operation_commit(&repo, &operation.id) {
                        Ok(Some(oid)) => {
                            if let Err(error) = align_index_paths_to_head(&repo, &paths) {
                                return Ok(operation_failure(
                                    &operation,
                                    SyncStage::Finalize,
                                    error,
                                    app_state,
                                ));
                            }
                            oid
                        }
                        Ok(None) => {
                            let prepared = match prepare_workspace_changes(
                                &repo,
                                &operation.workspace_changes,
                            ) {
                                Ok(prepared) if prepared.paths() == paths => prepared,
                                Ok(prepared) => {
                                    let path = paths
                                        .iter()
                                        .find(|path| !prepared.paths().contains(path))
                                        .or_else(|| {
                                            prepared
                                                .paths()
                                                .iter()
                                                .find(|path| !paths.contains(path))
                                        })
                                        .cloned()
                                        .unwrap_or_else(|| "workspace".to_owned());
                                    let error = AppError::ManagedContentChanged { path };
                                    return finish_operation_failure(
                                        &operation,
                                        SyncStage::Commit,
                                        &error,
                                        app_state,
                                    );
                                }
                                Err(error) => {
                                    return finish_operation_failure(
                                        &operation,
                                        SyncStage::Commit,
                                        &error,
                                        app_state,
                                    );
                                }
                            };
                            match commit_prepared_workspace_changes(
                                &repo,
                                &prepared,
                                &format!(
                                    "Marktree sync {} [marktree-operation:{}]",
                                    Utc::now().format("%Y-%m-%d %H:%M UTC"),
                                    operation.id
                                ),
                            ) {
                                Ok(oid) => oid,
                                Err(error) => {
                                    return Ok(operation_failure(
                                        &operation,
                                        error.stage,
                                        error.error,
                                        app_state,
                                    ))
                                }
                            }
                        }
                        Err(error) => {
                            return Ok(operation_failure(
                                &operation,
                                SyncStage::Commit,
                                error,
                                app_state,
                            ))
                        }
                    };
                    operation.committed = true;
                    operation.commit_id = Some(oid.to_string());
                }
                operation.phase = GitOperationPhase::Fetch;
                app_state.update_git_operation(operation.clone())?;
            }
            GitOperationPhase::Fetch => {
                if let Err(error) = fetch_remote(&repo, credential.clone()) {
                    return Ok(operation_failure(
                        &operation,
                        SyncStage::Fetch,
                        error,
                        app_state,
                    ));
                }
                match ensure_current_branch_upstream(&repo)? {
                    UpstreamDisposition::Configured => {
                        operation.phase = GitOperationPhase::PreserveWorkingTree;
                    }
                    UpstreamDisposition::MissingRemoteBranch
                        if operation.kind == GitOperationKind::Sync =>
                    {
                        operation.phase = GitOperationPhase::Push;
                    }
                    UpstreamDisposition::MissingRemoteBranch => {
                        operation.phase = GitOperationPhase::Finalize;
                    }
                }
                app_state.update_git_operation(operation.clone())?;
            }
            GitOperationPhase::PreserveWorkingTree => {
                if operation.original_head_oid.is_none() {
                    operation.original_head_oid = repo
                        .head()
                        .ok()
                        .and_then(|head| head.target())
                        .map(|oid| oid.to_string());
                    app_state.update_git_operation(operation.clone())?;
                }
                if operation.stash_oid.is_none() {
                    if let Some(oid) = find_operation_stash(&mut repo, &operation.id)? {
                        operation.stash_oid = Some(oid.to_string());
                    } else {
                        operation.stash_oid =
                            stash_if_needed(&mut repo, &operation.id)?.map(|oid| oid.to_string());
                    }
                }
                operation.phase = GitOperationPhase::Rebase;
                app_state.update_git_operation(operation.clone())?;
            }
            GitOperationPhase::Rebase => {
                let conflicts = if repo.state() == RepositoryState::RebaseMerge {
                    if repo.index()?.has_conflicts() {
                        capture_conflicts(&repo, app_state)?
                    } else {
                        continue_rebase(&mut repo, app_state)?
                    }
                } else {
                    rebase_onto_upstream(&mut repo, app_state)?
                };
                if !conflicts.is_empty() {
                    return Ok(operation_conflicts(&operation, conflicts));
                }
                operation.pulled = true;
                operation.phase = GitOperationPhase::RestoreWorkingTree;
                app_state.update_git_operation(operation.clone())?;
            }
            GitOperationPhase::RestoreWorkingTree => {
                if operation.stash_oid.is_some() {
                    if !operation.stash_applied {
                        if operation.stash_apply_started {
                            if repo.index()?.has_conflicts()
                                || stash_snapshot_is_applied(&repo, &operation)?
                            {
                                operation.stash_apply_started = false;
                                operation.stash_applied = true;
                                app_state.update_git_operation(operation.clone())?;
                            } else if stash_touched_paths_are_dirty(&repo, &operation)? {
                                return Ok(operation_failure(
                                    &operation,
                                    SyncStage::RestoreWorkingTree,
                                    AppError::Message(
                                        "Files covered by the interrupted working-tree recovery \
                                         changed before Marktree could confirm the result. The \
                                         operation and its exact Git stash were preserved; restore \
                                         those files or safeguard the newer copies before retrying."
                                            .to_owned(),
                                    ),
                                    app_state,
                                ));
                            }
                        } else {
                            operation.stash_apply_started = true;
                            app_state.update_git_operation(operation.clone())?;
                        }
                        if !operation.stash_applied {
                            let stash_index = operation_stash_index(&mut repo, &operation)?;
                            let mut options = StashApplyOptions::new();
                            options.reinstantiate_index();
                            match repo.stash_apply(stash_index, Some(&mut options)) {
                                Ok(()) => {
                                    operation.stash_apply_started = false;
                                    operation.stash_applied = true;
                                    app_state.update_git_operation(operation.clone())?;
                                }
                                Err(_error) if repo.index()?.has_conflicts() => {
                                    operation.stash_apply_started = false;
                                    operation.stash_applied = true;
                                    app_state.update_git_operation(operation.clone())?;
                                    let conflicts = capture_conflicts(&repo, app_state)?;
                                    return Ok(operation_conflicts(&operation, conflicts));
                                }
                                Err(error) => {
                                    return Ok(operation_failure(
                                        &operation,
                                        SyncStage::RestoreWorkingTree,
                                        error,
                                        app_state,
                                    ))
                                }
                            }
                        }
                    }
                    if repo.index()?.has_conflicts() {
                        let conflicts = capture_conflicts(&repo, app_state)?;
                        return Ok(operation_conflicts(&operation, conflicts));
                    }
                    repo.cleanup_state()?;
                }
                operation.phase = if operation.kind == GitOperationKind::Sync {
                    GitOperationPhase::Push
                } else {
                    GitOperationPhase::Finalize
                };
                app_state.update_git_operation(operation.clone())?;
            }
            GitOperationPhase::Push => {
                if let Err(error) = push_current_branch(&repo, credential.clone()) {
                    return Ok(operation_failure(
                        &operation,
                        SyncStage::Push,
                        error,
                        app_state,
                    ));
                }
                operation.pushed = true;
                operation.phase = GitOperationPhase::Finalize;
                app_state.update_git_operation(operation.clone())?;
            }
            GitOperationPhase::Finalize => {
                if operation.stash_oid.is_some() {
                    if let Ok(stash_index) = operation_stash_index(&mut repo, &operation) {
                        repo.stash_drop(stash_index)?;
                    } else if !operation.stash_applied {
                        return Ok(operation_failure(
                            &operation,
                            SyncStage::Finalize,
                            AppError::Message(
                                "The exact working-tree snapshot for this Git operation is missing."
                                    .to_owned(),
                            ),
                            app_state,
                        ));
                    }
                    operation.stash_oid = None;
                    operation.stash_apply_started = false;
                    operation.stash_applied = false;
                    app_state.update_git_operation(operation.clone())?;
                }
                if operation.kind == GitOperationKind::Sync {
                    if let Err(error) = app_state
                        .clear_workspace_changes(&operation.root, &operation.workspace_changes)
                    {
                        return Ok(operation_failure(
                            &operation,
                            SyncStage::Finalize,
                            error,
                            app_state,
                        ));
                    }
                }
                if let Err(error) = archive_operation_recoveries(&operation, app_state) {
                    return Ok(operation_failure(
                        &operation,
                        SyncStage::Finalize,
                        error,
                        app_state,
                    ));
                }
                app_state.finish_git_operation(
                    &operation.root,
                    &operation.id,
                    OperationLogOutcome::Succeeded,
                    None,
                )?;
                return Ok(SyncResult {
                    committed: operation.committed,
                    commit_id: operation.commit_id,
                    pulled: operation.pulled,
                    pushed: operation.pushed,
                    changed_paths: operation.changed_paths,
                    conflicts: Vec::new(),
                    failure_stage: None,
                    error: None,
                });
            }
        }
    }
}

fn operation_failure(
    operation: &PendingGitOperation,
    stage: SyncStage,
    error: impl Into<AppError>,
    app_state: &PersistentState,
) -> SyncResult {
    let error = error.into();
    app_state.record_git_operation_failure(operation, &error);
    operation_failure_result(operation, stage, &error)
}

fn operation_failure_result(
    operation: &PendingGitOperation,
    stage: SyncStage,
    error: &AppError,
) -> SyncResult {
    SyncResult {
        committed: operation.committed,
        commit_id: operation.commit_id.clone(),
        pulled: operation.pulled,
        pushed: operation.pushed,
        changed_paths: operation.changed_paths.clone(),
        conflicts: Vec::new(),
        failure_stage: Some(stage),
        error: Some(error.payload()),
    }
}

fn finish_operation_failure(
    operation: &PendingGitOperation,
    stage: SyncStage,
    error: &AppError,
    app_state: &PersistentState,
) -> AppResult<SyncResult> {
    let result = operation_failure_result(operation, stage, error);
    app_state.finish_git_operation(
        &operation.root,
        &operation.id,
        OperationLogOutcome::Failed,
        Some(error),
    )?;
    Ok(result)
}

fn operation_conflicts(
    operation: &PendingGitOperation,
    conflicts: Vec<ConflictRecord>,
) -> SyncResult {
    SyncResult {
        committed: operation.committed,
        commit_id: operation.commit_id.clone(),
        pulled: operation.pulled,
        pushed: operation.pushed,
        changed_paths: operation.changed_paths.clone(),
        conflicts,
        failure_stage: None,
        error: None,
    }
}
