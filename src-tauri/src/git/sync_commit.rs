use std::path::Path;

use git2::{Index, Oid, Repository};

use crate::{
    error::{AppError, AppResult},
    file_version::{guard_expected_version, FileVersionGuard},
    paths::{
        canonical_root, normalize_relative, normalize_relative_paths, resolve_existing_file,
        resolve_for_write,
    },
    types::{SyncStage, WorkspaceChange, WorkspaceChangeOperation},
};

use super::repository::{open_exact_repository, signature, workdir};

pub(super) fn commit_only_paths(
    repo: &Repository,
    paths: &[String],
    message: &str,
) -> Result<Oid, IsolatedCommitError> {
    commit_only_paths_with_prepared(repo, paths, message, None)
}

fn commit_only_paths_with_prepared(
    repo: &Repository,
    paths: &[String],
    message: &str,
    prepared: Option<&[PreparedWorkspaceChange]>,
) -> Result<Oid, IsolatedCommitError> {
    let normalized = normalize_relative_paths(paths)
        .map_err(|error| IsolatedCommitError::new(SyncStage::Stage, error))?;
    let actual_index = repo
        .index()
        .map_err(|error| IsolatedCommitError::new(SyncStage::Stage, error))?;
    if actual_index.has_conflicts() {
        return Err(IsolatedCommitError::new(
            SyncStage::Stage,
            AppError::Message(
                "Resolve the existing Git index conflicts before synchronizing.".to_owned(),
            ),
        ));
    }
    drop(actual_index);

    let mut isolated = Index::new_ext(repo.object_format())
        .map_err(|error| IsolatedCommitError::new(SyncStage::Stage, error))?;
    let parent = match repo.head() {
        Ok(head) => {
            let commit = head
                .peel_to_commit()
                .map_err(|error| IsolatedCommitError::new(SyncStage::Stage, error))?;
            isolated
                .read_tree(
                    &commit
                        .tree()
                        .map_err(|error| IsolatedCommitError::new(SyncStage::Stage, error))?,
                )
                .map_err(|error| IsolatedCommitError::new(SyncStage::Stage, error))?;
            Some(commit)
        }
        Err(error) if error.code() == git2::ErrorCode::UnbornBranch => {
            isolated
                .clear()
                .map_err(|error| IsolatedCommitError::new(SyncStage::Stage, error))?;
            None
        }
        Err(error) => {
            return Err(IsolatedCommitError::new(SyncStage::Stage, error));
        }
    };
    let root = workdir(repo).map_err(|error| IsolatedCommitError::new(SyncStage::Stage, error))?;
    let root = canonical_root(root.to_string_lossy().as_ref())
        .map_err(|error| IsolatedCommitError::new(SyncStage::Stage, error))?;
    let mut worktree_entries = repo
        .index()
        .map_err(|error| IsolatedCommitError::new(SyncStage::Stage, error))?;
    for path in &normalized {
        if let Some(prepared) = prepared {
            let change = prepared
                .iter()
                .find(|change| change.path == *path)
                .ok_or_else(|| {
                    IsolatedCommitError::new(
                        SyncStage::Stage,
                        AppError::ManagedContentChanged { path: path.clone() },
                    )
                })?;
            if let Some(bytes) = &change.bytes {
                worktree_entries
                    .add_path(Path::new(path))
                    .map_err(|error| IsolatedCommitError::new(SyncStage::Stage, error))?;
                let mut entry = worktree_entries
                    .get_path(Path::new(path), 0)
                    .ok_or_else(|| {
                        IsolatedCommitError::new(
                            SyncStage::Stage,
                            AppError::ManagedContentChanged { path: path.clone() },
                        )
                    })?;
                entry.id = repo
                    .blob(bytes)
                    .map_err(|error| IsolatedCommitError::new(SyncStage::Stage, error))?;
                entry.file_size = bytes.len().try_into().unwrap_or(u32::MAX);
                isolated
                    .add(&entry)
                    .map_err(|error| IsolatedCommitError::new(SyncStage::Stage, error))?;
            } else {
                remove_isolated_path(&mut isolated, path)?;
            }
            continue;
        }
        let absolute = resolve_for_write(&root, path)
            .map_err(|error| IsolatedCommitError::new(SyncStage::Stage, error))?;
        if absolute.exists() {
            worktree_entries
                .add_path(Path::new(path))
                .map_err(|error| IsolatedCommitError::new(SyncStage::Stage, error))?;
            let entry = worktree_entries
                .get_path(Path::new(path), 0)
                .ok_or_else(|| {
                    IsolatedCommitError::new(
                        SyncStage::Stage,
                        AppError::Message(format!(
                            "Git did not produce an index entry for '{path}'."
                        )),
                    )
                })?;
            isolated
                .add(&entry)
                .map_err(|error| IsolatedCommitError::new(SyncStage::Stage, error))?;
        } else {
            remove_isolated_path(&mut isolated, path)?;
        }
    }
    drop(worktree_entries);
    let tree_id = isolated
        .write_tree_to(repo)
        .map_err(|error| IsolatedCommitError::new(SyncStage::Stage, error))?;
    if parent
        .as_ref()
        .is_some_and(|parent| parent.tree_id().eq(&tree_id))
    {
        return Err(IsolatedCommitError::new(
            SyncStage::Commit,
            AppError::Message("There are no workspace changes to commit.".to_owned()),
        ));
    }
    let tree = repo
        .find_tree(tree_id)
        .map_err(|error| IsolatedCommitError::new(SyncStage::Commit, error))?;
    let signature =
        signature(repo).map_err(|error| IsolatedCommitError::new(SyncStage::Commit, error))?;
    let parent_refs = parent.as_ref().into_iter().collect::<Vec<_>>();
    let oid = repo
        .commit(
            Some("HEAD"),
            &signature,
            &signature,
            message,
            &tree,
            &parent_refs,
        )
        .map_err(|error| IsolatedCommitError::new(SyncStage::Commit, error))?;
    if !normalized.is_empty() {
        align_index_paths_to_head(repo, &normalized)
            .map_err(|error| IsolatedCommitError::new(SyncStage::Finalize, error))?;
    }
    Ok(oid)
}

fn remove_isolated_path(index: &mut Index, path: &str) -> Result<(), IsolatedCommitError> {
    if let Err(error) = index.remove_path(Path::new(path)) {
        if error.code() != git2::ErrorCode::NotFound {
            return Err(IsolatedCommitError::new(SyncStage::Stage, error));
        }
    }
    Ok(())
}

pub(super) struct PreparedWorkspaceChanges {
    paths: Vec<String>,
    changes: Vec<PreparedWorkspaceChange>,
}

struct PreparedWorkspaceChange {
    path: String,
    bytes: Option<Vec<u8>>,
    _guard: FileVersionGuard,
}

impl PreparedWorkspaceChanges {
    pub(super) fn paths(&self) -> &[String] {
        &self.paths
    }
}

pub(super) fn commit_prepared_workspace_changes(
    repo: &Repository,
    prepared: &PreparedWorkspaceChanges,
    message: &str,
) -> Result<Oid, IsolatedCommitError> {
    commit_only_paths_with_prepared(repo, &prepared.paths, message, Some(&prepared.changes))
}

pub(super) fn find_operation_commit(
    repo: &Repository,
    operation_id: &str,
) -> AppResult<Option<Oid>> {
    let marker = format!("[marktree-operation:{operation_id}]");
    let mut commit = match repo.head() {
        Ok(head) => head.peel_to_commit()?,
        Err(error) if error.code() == git2::ErrorCode::UnbornBranch => return Ok(None),
        Err(error) => return Err(error.into()),
    };
    for _ in 0..1024 {
        if commit
            .message()
            .is_ok_and(|message| message.contains(&marker))
        {
            return Ok(Some(commit.id()));
        }
        if commit.parent_count() == 0 {
            break;
        }
        commit = commit.parent(0)?;
    }
    Ok(None)
}

pub(super) fn align_index_paths_to_head(repo: &Repository, paths: &[String]) -> AppResult<()> {
    let normalized = normalize_relative_paths(paths)?;
    let path_refs = normalized.iter().map(Path::new).collect::<Vec<_>>();
    let head = repo.head()?.peel(git2::ObjectType::Commit)?;
    repo.reset_default(Some(&head), path_refs)?;
    Ok(())
}

pub(super) fn tracked_workspace_paths(
    repo: &Repository,
    changes: &[WorkspaceChange],
) -> AppResult<Vec<String>> {
    Ok(prepare_workspace_changes(repo, changes)?.paths)
}

pub(super) fn prepare_workspace_changes(
    repo: &Repository,
    changes: &[WorkspaceChange],
) -> AppResult<PreparedWorkspaceChanges> {
    let root = canonical_root(workdir(repo)?.to_string_lossy().as_ref())?;
    let mut paths = Vec::new();
    let mut prepared = Vec::new();
    for change in changes {
        let path = normalize_relative(&change.path)?;
        let (guard, bytes) = match change.operation {
            WorkspaceChangeOperation::Upsert => {
                let version = change
                    .version
                    .as_deref()
                    .ok_or_else(|| AppError::ManagedContentChanged { path: path.clone() })?;
                let absolute = resolve_existing_file(&root, &path)
                    .map_err(|_| AppError::ManagedContentChanged { path: path.clone() })?;
                let guard = guard_expected_version(&absolute, Some(version), false)
                    .map_err(|_| AppError::ManagedContentChanged { path: path.clone() })?;
                let bytes = guard
                    .read_bytes()
                    .map_err(|_| AppError::ManagedContentChanged { path: path.clone() })?;
                (guard, Some(bytes))
            }
            WorkspaceChangeOperation::Delete => {
                let absolute = resolve_for_write(&root, &path)?;
                let guard = guard_expected_version(&absolute, None, true)
                    .map_err(|_| AppError::ManagedContentChanged { path: path.clone() })?;
                (guard, None)
            }
        };
        if repo.status_file(Path::new(&path))? == git2::Status::CURRENT {
            continue;
        }
        paths.push(path.clone());
        prepared.push(PreparedWorkspaceChange {
            path,
            bytes,
            _guard: guard,
        });
    }
    Ok(PreparedWorkspaceChanges {
        paths,
        changes: prepared,
    })
}

pub fn commit_workspace_baseline(
    root: &str,
    paths: &[String],
    operation_id: &str,
) -> AppResult<Oid> {
    let repo = open_exact_repository(root)?;
    if let Some(commit) = find_operation_commit(&repo, operation_id)? {
        return Ok(commit);
    }
    let message = format!("Marktree workspace baseline\n\n[marktree-operation:{operation_id}]");
    commit_only_paths(&repo, paths, &message).map_err(|error| error.error)
}

#[derive(Debug)]
pub(super) struct IsolatedCommitError {
    pub(super) stage: SyncStage,
    pub(super) error: AppError,
}

impl IsolatedCommitError {
    fn new(stage: SyncStage, error: impl Into<AppError>) -> Self {
        Self {
            stage,
            error: error.into(),
        }
    }
}
