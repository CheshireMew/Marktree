use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::Path,
};

use chrono::Utc;
use git2::{Oid, Repository, RepositoryState};
use serde::{Deserialize, Serialize};

use super::repository::{fast_forward, signature, upstream_commit, workdir_string};
use crate::{
    error::{AppError, AppResult},
    file_version::hash_bytes,
    paths::{atomic_write, canonical_root, normalize_relative, paths_equal, resolve_for_write},
    state::PersistentState,
    types::{ConflictChoice, ConflictKind, ConflictRecord, PendingGitOperation},
};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct ConflictRecoveryMetadata {
    operation_id: String,
    root: String,
    path: String,
    created_at: String,
    kind: ConflictKind,
    ancestor_exists: bool,
    local_exists: bool,
    remote_exists: bool,
    #[serde(default)]
    pub(super) choice: Option<ConflictChoice>,
}

pub fn resolve_conflict(
    root: &str,
    path: &str,
    recovery_id: &str,
    choice: ConflictChoice,
    app_state: &PersistentState,
) -> AppResult<()> {
    validate_recovery_id(recovery_id)?;
    let relative = normalize_relative(path)?;
    let recovery_dir = app_state.recovery_dir()?.join(recovery_id);
    let selected = match choice {
        ConflictChoice::Local => recovery_dir.join("local.bin"),
        ConflictChoice::Remote => recovery_dir.join("remote.bin"),
        ConflictChoice::Merged => {
            return Err(AppError::Message(
                "Merged conflict content must be supplied explicitly.".to_owned(),
            ))
        }
    };
    let mut metadata = recovery_metadata(&recovery_dir)?;
    validate_recovery_target(root, &relative, &metadata)?;
    let exists = match choice {
        ConflictChoice::Local => metadata.local_exists,
        ConflictChoice::Remote => metadata.remote_exists,
        ConflictChoice::Merged => false,
    };
    if exists {
        stage_resolved_content(root, &relative, &fs::read(&selected)?)?;
    } else {
        stage_resolved_deletion(root, &relative)?;
    }
    metadata.choice = Some(choice);
    write_recovery_metadata(&recovery_dir, &metadata)?;
    Ok(())
}

pub fn resolve_conflict_with_content(
    root: &str,
    path: &str,
    recovery_id: &str,
    content: &str,
    app_state: &PersistentState,
) -> AppResult<()> {
    validate_recovery_id(recovery_id)?;
    let relative = normalize_relative(path)?;
    let recovery_dir = app_state.recovery_dir()?.join(recovery_id);
    let mut metadata = recovery_metadata(&recovery_dir)?;
    validate_recovery_target(root, &relative, &metadata)?;
    if metadata.kind != ConflictKind::Text {
        return Err(AppError::Message(
            "Only text conflicts can be resolved with merged content.".to_owned(),
        ));
    }
    stage_resolved_content(root, &relative, content.as_bytes())?;
    metadata.choice = Some(ConflictChoice::Merged);
    write_recovery_metadata(&recovery_dir, &metadata)?;
    Ok(())
}

pub fn pending_conflicts(
    root: &str,
    app_state: &PersistentState,
) -> AppResult<Vec<ConflictRecord>> {
    let repo = Repository::open(root)?;
    if !repo.index()?.has_conflicts() {
        return Ok(Vec::new());
    }
    capture_conflicts(&repo, app_state)
}

pub(super) fn rebase_onto_upstream(
    repo: &mut Repository,
    app_state: &PersistentState,
) -> AppResult<Vec<ConflictRecord>> {
    let upstream = upstream_commit(repo)?;
    let head = repo.head()?.peel_to_commit()?;
    if head.id() == upstream.id() {
        return Ok(Vec::new());
    }
    let (ahead, behind) = repo.graph_ahead_behind(head.id(), upstream.id())?;
    if behind == 0 {
        return Ok(Vec::new());
    }
    if ahead == 0 {
        fast_forward(repo, upstream.id())?;
        return Ok(Vec::new());
    }

    let annotated = repo.find_annotated_commit(upstream.id())?;
    let mut rebase = repo.rebase(None, Some(&annotated), None, None)?;
    drain_rebase(repo, &mut rebase, app_state, false)
}

pub(super) fn continue_rebase(
    repo: &mut Repository,
    app_state: &PersistentState,
) -> AppResult<Vec<ConflictRecord>> {
    if repo.state() != RepositoryState::RebaseMerge {
        return Err(AppError::Message(
            "There is no Marktree rebase waiting for conflict resolution.".to_owned(),
        ));
    }
    if repo.index()?.has_conflicts() {
        return capture_conflicts(repo, app_state);
    }
    let mut rebase = repo.open_rebase(None)?;
    drain_rebase(repo, &mut rebase, app_state, true)
}

pub(super) fn capture_conflicts(
    repo: &Repository,
    app_state: &PersistentState,
) -> AppResult<Vec<ConflictRecord>> {
    let index = repo.index()?;
    let recovery_root = app_state.recovery_dir()?;
    let root = workdir_string(repo)?;
    let operation = app_state.pending_git_operation(&root).ok_or_else(|| {
        AppError::Message(
            "Git reported conflicts without a persisted Marktree operation.".to_owned(),
        )
    })?;
    let mut unresolved_paths = BTreeSet::new();
    for conflict in index.conflicts()? {
        let conflict = conflict?;
        if let Some(path) = conflict
            .our
            .as_ref()
            .or(conflict.their.as_ref())
            .or(conflict.ancestor.as_ref())
            .map(|entry| String::from_utf8_lossy(&entry.path).replace('\\', "/"))
        {
            unresolved_paths.insert(path);
        }
    }
    let mut existing = load_recovery_records(&root, &operation.id, &unresolved_paths, app_state)?
        .into_iter()
        .map(|record| (record.path.clone(), record))
        .collect::<BTreeMap<_, _>>();
    let mut records = Vec::new();
    for conflict in index.conflicts()? {
        let conflict = conflict?;
        let path_bytes = conflict
            .our
            .as_ref()
            .or(conflict.their.as_ref())
            .or(conflict.ancestor.as_ref())
            .map(|entry| entry.path.as_slice())
            .ok_or_else(|| AppError::Message("Conflict path is missing.".to_owned()))?;
        let path = String::from_utf8_lossy(path_bytes).replace('\\', "/");
        if let Some(record) = existing.remove(&path) {
            records.push(record);
            continue;
        }
        let ancestor = conflict
            .ancestor
            .as_ref()
            .map(|entry| blob_bytes(repo, entry.id))
            .transpose()?;
        let local = conflict
            .their
            .as_ref()
            .map(|entry| blob_bytes(repo, entry.id))
            .transpose()?;
        let remote = conflict
            .our
            .as_ref()
            .map(|entry| blob_bytes(repo, entry.id))
            .transpose()?;
        let kind = if local.is_none() || remote.is_none() {
            ConflictKind::DeleteModify
        } else if local.as_deref().is_some_and(is_binary)
            || remote.as_deref().is_some_and(is_binary)
            || ancestor.as_deref().is_some_and(is_binary)
        {
            ConflictKind::Binary
        } else {
            ConflictKind::Text
        };
        let seed = format!(
            "{}\n{}\n{}\n{:?}\n{:?}\n{:?}",
            operation.id,
            root,
            path,
            conflict.ancestor.as_ref().map(|entry| entry.id),
            conflict.their.as_ref().map(|entry| entry.id),
            conflict.our.as_ref().map(|entry| entry.id)
        );
        let recovery_id = hash_bytes(seed.as_bytes())[..20].to_owned();
        let directory = recovery_root.join(&recovery_id);
        fs::create_dir_all(&directory)?;
        if let Some(content) = ancestor.as_deref() {
            atomic_write(&directory.join("ancestor.bin"), content)?;
        }
        if let Some(content) = local.as_deref() {
            atomic_write(&directory.join("local.bin"), content)?;
        }
        if let Some(content) = remote.as_deref() {
            atomic_write(&directory.join("remote.bin"), content)?;
        }
        write_recovery_metadata(
            &directory,
            &ConflictRecoveryMetadata {
                operation_id: operation.id.clone(),
                root: root.clone(),
                path: path.clone(),
                created_at: Utc::now().to_rfc3339(),
                kind,
                ancestor_exists: ancestor.is_some(),
                local_exists: local.is_some(),
                remote_exists: remote.is_some(),
                choice: None,
            },
        )?;
        records.push(ConflictRecord {
            path,
            kind,
            ancestor: ancestor.and_then(|bytes| String::from_utf8(bytes).ok()),
            local: local.and_then(|bytes| String::from_utf8(bytes).ok()),
            remote: remote.and_then(|bytes| String::from_utf8(bytes).ok()),
            ancestor_exists: conflict.ancestor.is_some(),
            local_exists: conflict.their.is_some(),
            remote_exists: conflict.our.is_some(),
            recovery_id,
            choice: None,
        });
    }
    Ok(records)
}

pub(super) fn archive_operation_recoveries(
    operation: &PendingGitOperation,
    app_state: &PersistentState,
) -> AppResult<()> {
    let recovery_root = app_state.recovery_dir()?;
    let archive_root = recovery_root.join("archive").join(&operation.id);
    let mut matches = Vec::new();
    for item in fs::read_dir(&recovery_root)? {
        let directory = item?.path();
        if !directory.is_dir() || directory.file_name().is_some_and(|name| name == "archive") {
            continue;
        }
        let Ok(metadata) = recovery_metadata(&directory) else {
            continue;
        };
        if metadata.operation_id == operation.id {
            matches.push(directory);
        }
    }
    if matches.is_empty() {
        return Ok(());
    }
    fs::create_dir_all(&archive_root)?;
    for directory in matches {
        let name = directory.file_name().ok_or_else(|| {
            AppError::Message("A recovery directory has no file name.".to_owned())
        })?;
        fs::rename(&directory, archive_root.join(name))?;
    }
    Ok(())
}

fn drain_rebase(
    repo: &Repository,
    rebase: &mut git2::Rebase<'_>,
    app_state: &PersistentState,
    commit_current: bool,
) -> AppResult<Vec<ConflictRecord>> {
    if commit_current {
        commit_rebase_operation(repo, rebase)?;
    }
    loop {
        match rebase.next() {
            Some(Ok(_)) => {
                let index = repo.index()?;
                if index.has_conflicts() {
                    drop(index);
                    return capture_conflicts(repo, app_state);
                }
                commit_rebase_operation(repo, rebase)?;
            }
            Some(Err(error)) => return Err(error.into()),
            None => {
                rebase.finish(Some(&signature(repo)?))?;
                return Ok(Vec::new());
            }
        }
    }
}

fn commit_rebase_operation(repo: &Repository, rebase: &mut git2::Rebase<'_>) -> AppResult<()> {
    match rebase.commit(None, &signature(repo)?, None) {
        Ok(_) => Ok(()),
        Err(error) if error.code() == git2::ErrorCode::Applied => Ok(()),
        Err(error) => Err(error.into()),
    }
}

fn load_recovery_records(
    root: &str,
    operation_id: &str,
    unresolved_paths: &BTreeSet<String>,
    app_state: &PersistentState,
) -> AppResult<Vec<ConflictRecord>> {
    let recovery_root = app_state.recovery_dir()?;
    let mut records = Vec::new();
    for item in fs::read_dir(recovery_root)? {
        let directory = item?.path();
        if !directory.is_dir() {
            continue;
        }
        let Ok(metadata) = recovery_metadata(&directory) else {
            continue;
        };
        if metadata.choice.is_some()
            || metadata.operation_id != operation_id
            || !paths_equal(&metadata.root, root)
            || !unresolved_paths.contains(&metadata.path)
        {
            continue;
        }
        let Some(recovery_id) = directory
            .file_name()
            .and_then(|value| value.to_str())
            .map(str::to_owned)
        else {
            continue;
        };
        if validate_recovery_id(&recovery_id).is_err() {
            continue;
        }
        records.push(recovery_record(&directory, recovery_id, metadata)?);
    }
    records.sort_by(|left, right| left.path.cmp(&right.path));
    Ok(records)
}

fn recovery_record(
    directory: &Path,
    recovery_id: String,
    metadata: ConflictRecoveryMetadata,
) -> AppResult<ConflictRecord> {
    let read_text = |name: &str, exists: bool| -> AppResult<Option<String>> {
        if !exists {
            return Ok(None);
        }
        let bytes = fs::read(directory.join(name))?;
        Ok(String::from_utf8(bytes).ok())
    };
    Ok(ConflictRecord {
        path: metadata.path,
        kind: metadata.kind,
        ancestor: read_text("ancestor.bin", metadata.ancestor_exists)?,
        local: read_text("local.bin", metadata.local_exists)?,
        remote: read_text("remote.bin", metadata.remote_exists)?,
        ancestor_exists: metadata.ancestor_exists,
        local_exists: metadata.local_exists,
        remote_exists: metadata.remote_exists,
        recovery_id,
        choice: metadata.choice,
    })
}

fn stage_resolved_content(root: &str, relative: &str, content: &[u8]) -> AppResult<()> {
    let repo = Repository::open(root)?;
    let root_path = canonical_root(workdir_string(&repo)?.as_str())?;
    let destination = resolve_for_write(&root_path, relative)?;
    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent)?;
    }
    atomic_write(&destination, content)?;
    let mut index = repo.index()?;
    if let Err(error) = index.conflict_remove(Path::new(relative)) {
        if error.code() != git2::ErrorCode::NotFound {
            return Err(error.into());
        }
    }
    index.add_path(Path::new(relative))?;
    index.write()?;
    Ok(())
}

fn stage_resolved_deletion(root: &str, relative: &str) -> AppResult<()> {
    let repo = Repository::open(root)?;
    let root_path = canonical_root(workdir_string(&repo)?.as_str())?;
    let destination = resolve_for_write(&root_path, relative)?;
    if destination.exists() {
        fs::remove_file(&destination)?;
    }
    let mut index = repo.index()?;
    if let Err(error) = index.conflict_remove(Path::new(relative)) {
        if error.code() != git2::ErrorCode::NotFound {
            return Err(error.into());
        }
    }
    if let Err(error) = index.remove_path(Path::new(relative)) {
        if error.code() != git2::ErrorCode::NotFound {
            return Err(error.into());
        }
    }
    index.write()?;
    Ok(())
}

fn blob_bytes(repo: &Repository, id: Oid) -> AppResult<Vec<u8>> {
    Ok(repo.find_blob(id)?.content().to_vec())
}

fn is_binary(content: &[u8]) -> bool {
    content.contains(&0) || std::str::from_utf8(content).is_err()
}

pub(super) fn recovery_metadata(directory: &Path) -> AppResult<ConflictRecoveryMetadata> {
    Ok(serde_json::from_slice(&fs::read(
        directory.join("metadata.json"),
    )?)?)
}

fn write_recovery_metadata(directory: &Path, metadata: &ConflictRecoveryMetadata) -> AppResult<()> {
    atomic_write(
        &directory.join("metadata.json"),
        &serde_json::to_vec_pretty(metadata)?,
    )
}

fn validate_recovery_target(
    root: &str,
    relative: &str,
    metadata: &ConflictRecoveryMetadata,
) -> AppResult<()> {
    if !paths_equal(&metadata.root, root) || metadata.path != relative {
        return Err(AppError::Message(
            "The recovery record does not belong to this repository conflict.".to_owned(),
        ));
    }
    Ok(())
}

fn validate_recovery_id(value: &str) -> AppResult<()> {
    if value.len() != 20 || !value.chars().all(|character| character.is_ascii_hexdigit()) {
        return Err(AppError::InvalidPath(value.to_owned()));
    }
    Ok(())
}
