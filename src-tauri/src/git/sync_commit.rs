use std::path::Path;

use git2::{Index, Oid, Repository};

use crate::{
    error::{AppError, AppResult},
    file_version::verify_expected_version,
    paths::{
        canonical_root, normalize_relative, normalize_relative_paths, resolve_existing_file,
        resolve_for_write,
    },
    types::{SyncStage, WorkspaceChange, WorkspaceChangeOperation},
};

use super::repository::{signature, workdir};

pub(super) fn commit_only_paths(
    repo: &Repository,
    paths: &[String],
    message: &str,
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
        } else if let Err(error) = isolated.remove_path(Path::new(path)) {
            if error.code() != git2::ErrorCode::NotFound {
                return Err(IsolatedCommitError::new(SyncStage::Stage, error));
            }
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
    let root = canonical_root(workdir(repo)?.to_string_lossy().as_ref())?;
    let mut result = Vec::new();
    for change in changes {
        let path = normalize_relative(&change.path)?;
        if repo.status_file(Path::new(&path))? == git2::Status::CURRENT {
            continue;
        }
        match change.operation {
            WorkspaceChangeOperation::Upsert => {
                let version = change
                    .version
                    .as_deref()
                    .ok_or_else(|| AppError::ManagedContentChanged { path: path.clone() })?;
                let absolute = resolve_existing_file(&root, &path)
                    .map_err(|_| AppError::ManagedContentChanged { path: path.clone() })?;
                verify_expected_version(&absolute, Some(version), false)
                    .map_err(|_| AppError::ManagedContentChanged { path: path.clone() })?;
            }
            WorkspaceChangeOperation::Delete => {
                let absolute = resolve_for_write(&root, &path)?;
                if absolute.exists() {
                    return Err(AppError::ManagedContentChanged { path });
                }
            }
        }
        result.push(path);
    }
    Ok(result)
}

pub fn commit_workspace_baseline(root: &str, paths: &[String]) -> AppResult<Oid> {
    let repo = Repository::open(root)?;
    commit_only_paths(&repo, paths, "Marktree workspace baseline").map_err(|error| error.error)
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
