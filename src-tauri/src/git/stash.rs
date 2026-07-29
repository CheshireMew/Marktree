use std::{collections::BTreeSet, path::PathBuf};

use git2::{Index, Oid, Repository, StashFlags, StatusOptions, Tree, TreeWalkMode, TreeWalkResult};

use super::repository::{signature, workdir};
use crate::{
    error::{AppError, AppResult},
    types::PendingGitOperation,
};

const AUTO_STASH_MESSAGE: &str = "Marktree automatic operation";

pub(super) fn stash_if_needed(repo: &mut Repository, operation_id: &str) -> AppResult<Option<Oid>> {
    let mut options = StatusOptions::new();
    options
        .include_untracked(true)
        .recurse_untracked_dirs(true)
        .exclude_submodules(true);
    if repo.statuses(Some(&mut options))?.is_empty() {
        return Ok(None);
    }
    let oid = repo.stash_save(
        &signature(repo)?,
        &format!("{AUTO_STASH_MESSAGE} {operation_id}"),
        Some(StashFlags::INCLUDE_UNTRACKED),
    )?;
    Ok(Some(oid))
}

pub(super) fn stash_snapshot_is_applied(
    repo: &Repository,
    operation: &PendingGitOperation,
) -> AppResult<bool> {
    let (base_tree, stash_worktree, stash_index, untracked_tree) = stash_trees(repo, operation)?;
    let head_tree = repo.head()?.peel_to_tree()?;
    let expected_worktree = repo.merge_trees(&base_tree, &head_tree, &stash_worktree, None)?;
    let expected_index = repo.merge_trees(&base_tree, &head_tree, &stash_index, None)?;
    if expected_worktree.has_conflicts() || expected_index.has_conflicts() {
        return Ok(false);
    }

    let worktree_paths = changed_tree_paths(repo, &base_tree, &stash_worktree)?;
    if !worktree_matches_index(repo, &expected_worktree, &worktree_paths)? {
        return Ok(false);
    }
    let index_paths = changed_tree_paths(repo, &base_tree, &stash_index)?;
    if !repository_index_matches(repo, &expected_index, &index_paths)? {
        return Ok(false);
    }
    if let Some(untracked_tree) = untracked_tree {
        let untracked_paths = tree_file_paths(&untracked_tree)?;
        if !worktree_matches_tree(repo, &untracked_tree, &untracked_paths)? {
            return Ok(false);
        }
    }
    Ok(true)
}

pub(super) fn stash_touched_paths_are_dirty(
    repo: &Repository,
    operation: &PendingGitOperation,
) -> AppResult<bool> {
    let (base_tree, stash_worktree, stash_index, untracked_tree) = stash_trees(repo, operation)?;
    let mut touched = changed_tree_paths(repo, &base_tree, &stash_worktree)?;
    touched.extend(changed_tree_paths(repo, &base_tree, &stash_index)?);
    if let Some(untracked_tree) = untracked_tree {
        touched.extend(tree_file_paths(&untracked_tree)?);
    }
    let mut options = StatusOptions::new();
    options
        .include_untracked(true)
        .recurse_untracked_dirs(true)
        .exclude_submodules(true);
    Ok(repo.statuses(Some(&mut options))?.iter().any(|entry| {
        entry
            .path()
            .is_ok_and(|path| touched.contains(std::path::Path::new(path)))
    }))
}

pub(super) fn find_operation_stash(
    repo: &mut Repository,
    operation_id: &str,
) -> AppResult<Option<Oid>> {
    let mut found = None;
    repo.stash_foreach(|_, message, oid| {
        if message.contains(AUTO_STASH_MESSAGE) && message.contains(operation_id) {
            found = Some(*oid);
            false
        } else {
            true
        }
    })?;
    Ok(found)
}

pub(super) fn operation_stash_index(
    repo: &mut Repository,
    operation: &PendingGitOperation,
) -> AppResult<usize> {
    let expected = operation
        .stash_oid
        .as_deref()
        .and_then(|value| Oid::from_str(value).ok());
    let mut found = None;
    repo.stash_foreach(|index, message, oid| {
        let matches_oid = expected.is_some_and(|expected| expected == *oid);
        let matches_id =
            message.contains(AUTO_STASH_MESSAGE) && message.contains(operation.id.as_str());
        if matches_oid || matches_id {
            found = Some(index);
            false
        } else {
            true
        }
    })?;
    found.ok_or_else(|| {
        AppError::Message(
            "The exact working-tree snapshot for this Git operation is missing.".to_owned(),
        )
    })
}

fn stash_trees<'repo>(
    repo: &'repo Repository,
    operation: &PendingGitOperation,
) -> AppResult<(Tree<'repo>, Tree<'repo>, Tree<'repo>, Option<Tree<'repo>>)> {
    let stash_oid = operation
        .stash_oid
        .as_deref()
        .and_then(|value| Oid::from_str(value).ok())
        .ok_or_else(|| {
            AppError::Message(
                "The exact working-tree snapshot for this Git operation is missing.".to_owned(),
            )
        })?;
    let stash = repo.find_commit(stash_oid)?;
    if stash.parent_count() < 2 {
        return Err(AppError::Message(
            "The automatic Git stash has an invalid structure.".to_owned(),
        ));
    }
    let base_tree = stash.parent(0)?.tree()?;
    let stash_worktree = stash.tree()?;
    let stash_index = stash.parent(1)?.tree()?;
    let untracked_tree = if stash.parent_count() > 2 {
        Some(stash.parent(2)?.tree()?)
    } else {
        None
    };
    Ok((base_tree, stash_worktree, stash_index, untracked_tree))
}

fn changed_tree_paths(
    repo: &Repository,
    base: &Tree<'_>,
    changed: &Tree<'_>,
) -> AppResult<BTreeSet<PathBuf>> {
    let diff = repo.diff_tree_to_tree(Some(base), Some(changed), None)?;
    let mut paths = BTreeSet::new();
    for delta in diff.deltas() {
        if let Some(path) = delta.old_file().path() {
            paths.insert(path.to_path_buf());
        }
        if let Some(path) = delta.new_file().path() {
            paths.insert(path.to_path_buf());
        }
    }
    Ok(paths)
}

fn tree_file_paths(tree: &Tree<'_>) -> AppResult<BTreeSet<PathBuf>> {
    let mut paths = BTreeSet::new();
    tree.walk(TreeWalkMode::PreOrder, |directory, entry| {
        if entry.kind() == Some(git2::ObjectType::Blob) {
            if let Ok(name) = entry.name() {
                paths.insert(PathBuf::from(format!("{directory}{name}")));
            }
        }
        TreeWalkResult::Ok
    })?;
    Ok(paths)
}

fn worktree_matches_index(
    repo: &Repository,
    expected: &Index,
    paths: &BTreeSet<PathBuf>,
) -> AppResult<bool> {
    let root = workdir(repo)?;
    for path in paths {
        let destination = root.join(path);
        let Some(entry) = expected.get_path(path, 0) else {
            if destination.exists() {
                return Ok(false);
            }
            continue;
        };
        if !destination.is_file() || repo.blob_path(&destination)? != entry.id {
            return Ok(false);
        }
    }
    Ok(true)
}

fn repository_index_matches(
    repo: &Repository,
    expected: &Index,
    paths: &BTreeSet<PathBuf>,
) -> AppResult<bool> {
    let actual = repo.index()?;
    Ok(paths.iter().all(
        |path| match (actual.get_path(path, 0), expected.get_path(path, 0)) {
            (None, None) => true,
            (Some(actual), Some(expected)) => {
                actual.id == expected.id && actual.mode == expected.mode
            }
            _ => false,
        },
    ))
}

fn worktree_matches_tree(
    repo: &Repository,
    expected: &Tree<'_>,
    paths: &BTreeSet<PathBuf>,
) -> AppResult<bool> {
    let root = workdir(repo)?;
    for path in paths {
        let entry = expected.get_path(path)?;
        let destination = root.join(path);
        if !destination.is_file() || repo.blob_path(&destination)? != entry.id() {
            return Ok(false);
        }
    }
    Ok(true)
}
