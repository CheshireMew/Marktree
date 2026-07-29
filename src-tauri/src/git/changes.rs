use std::path::Path;

use git2::{Diff, DiffOptions, IndexAddOption, Oid, Patch, Repository, StatusOptions};

use crate::{
    documents::read_text_at_root,
    error::{AppError, AppResult},
    paths::{
        canonical_root, normalize_relative, normalize_relative_paths, path_to_slashes,
        resolve_for_write,
    },
    types::{
        CredentialRecord, DiffFile, DiffHunk, DiffLine, DiffMode, DiffResult, GitStatusSnapshot,
        TextComparison,
    },
};

use super::{
    remote::{fetch_remote, push_current_branch},
    repository::{is_staged, signature, status_snapshot, upstream_commit, workdir},
};

pub fn stage_paths(root: &str, paths: &[String]) -> AppResult<GitStatusSnapshot> {
    let repo = Repository::open(root)?;
    let root_path = canonical_root(workdir(&repo)?.to_string_lossy().as_ref())?;
    let mut index = repo.index()?;
    for path in normalize_relative_paths(paths)? {
        let absolute = resolve_for_write(&root_path, &path)?;
        if absolute.exists() {
            index.add_path(Path::new(&path))?;
        } else {
            index.remove_path(Path::new(&path))?;
        }
    }
    index.write()?;
    status_snapshot(&repo)
}

pub fn stage_all(root: &str) -> AppResult<GitStatusSnapshot> {
    let repo = Repository::open(root)?;
    let mut index = repo.index()?;
    index.add_all(["*"].iter(), IndexAddOption::DEFAULT, None)?;
    index.update_all(["*"].iter(), None)?;
    index.write()?;
    status_snapshot(&repo)
}

pub fn unstage_paths(root: &str, paths: &[String]) -> AppResult<GitStatusSnapshot> {
    let repo = Repository::open(root)?;
    let paths = normalize_relative_paths(paths)?;
    let path_refs: Vec<&Path> = paths.iter().map(Path::new).collect();
    match repo.head() {
        Ok(head) => {
            let object = head.peel(git2::ObjectType::Commit)?;
            repo.reset_default(Some(&object), path_refs)?;
        }
        Err(error) if error.code() == git2::ErrorCode::UnbornBranch => {
            let mut index = repo.index()?;
            for path in paths {
                if let Err(error) = index.remove_path(Path::new(&path)) {
                    if error.code() != git2::ErrorCode::NotFound {
                        return Err(error.into());
                    }
                }
            }
            index.write()?;
        }
        Err(error) => return Err(error.into()),
    }
    status_snapshot(&repo)
}

pub fn commit(root: &str, message: &str) -> AppResult<String> {
    if message.trim().is_empty() {
        return Err(AppError::Message(
            "A commit message is required.".to_owned(),
        ));
    }
    let repo = Repository::open(root)?;
    Ok(create_commit(&repo, message.trim())?.to_string())
}

pub fn fetch(root: &str, credential: Option<CredentialRecord>) -> AppResult<GitStatusSnapshot> {
    let repo = Repository::open(root)?;
    fetch_remote(&repo, credential)?;
    status_snapshot(&repo)
}

pub fn push(root: &str, credential: Option<CredentialRecord>) -> AppResult<GitStatusSnapshot> {
    let repo = Repository::open(root)?;
    push_current_branch(&repo, credential)?;
    status_snapshot(&repo)
}

pub fn diff(root: &str, mode: DiffMode) -> AppResult<DiffResult> {
    let repo = Repository::open(root)?;
    let mut options = DiffOptions::new();
    options
        .include_untracked(true)
        .recurse_untracked_dirs(true)
        .include_typechange(true);

    let (diff, old_label, new_label) = match mode {
        DiffMode::WorktreeToIndex => (
            repo.diff_index_to_workdir(None, Some(&mut options))?,
            "Index".to_owned(),
            "Working tree".to_owned(),
        ),
        DiffMode::IndexToHead => {
            let head_tree = repo.head().ok().and_then(|head| head.peel_to_tree().ok());
            (
                repo.diff_tree_to_index(head_tree.as_ref(), None, Some(&mut options))?,
                "HEAD".to_owned(),
                "Index".to_owned(),
            )
        }
        DiffMode::WorktreeToHead => {
            let head_tree = repo.head().ok().and_then(|head| head.peel_to_tree().ok());
            (
                repo.diff_tree_to_workdir_with_index(head_tree.as_ref(), Some(&mut options))?,
                "HEAD".to_owned(),
                "Working tree".to_owned(),
            )
        }
        DiffMode::LocalToUpstream => {
            let head = repo.head()?.peel_to_commit()?;
            let upstream = upstream_commit(&repo)?;
            let head_tree = head.tree()?;
            let upstream_tree = upstream.tree()?;
            (
                repo.diff_tree_to_tree(Some(&upstream_tree), Some(&head_tree), Some(&mut options))?,
                upstream.id().to_string()[..8].to_owned(),
                head.id().to_string()[..8].to_owned(),
            )
        }
    };
    structured_diff(&diff, mode, old_label, new_label)
}

pub fn compare_worktrees(
    left_root: &str,
    right_root: &str,
    path: &str,
) -> AppResult<TextComparison> {
    Ok(TextComparison {
        path: normalize_relative(path)?,
        left_label: left_root.to_owned(),
        right_label: right_root.to_owned(),
        left: read_text_at_root(left_root, path)?,
        right: read_text_at_root(right_root, path)?,
    })
}

fn structured_diff(
    diff: &Diff<'_>,
    mode: DiffMode,
    old_label: String,
    new_label: String,
) -> AppResult<DiffResult> {
    let stats = diff.stats()?;
    let mut files = Vec::new();
    for index in 0..diff.deltas().len() {
        let delta = diff
            .get_delta(index)
            .ok_or_else(|| AppError::Message("Diff entry disappeared.".to_owned()))?;
        let path = delta
            .new_file()
            .path()
            .or_else(|| delta.old_file().path())
            .map(path_to_slashes)
            .unwrap_or_default();
        let old_path = delta.old_file().path().map(path_to_slashes);
        let patch = Patch::from_diff(diff, index)?;
        let binary = patch.is_none();
        let mut hunks = Vec::new();
        if let Some(patch) = patch {
            for hunk_index in 0..patch.num_hunks() {
                let (hunk, line_count) = patch.hunk(hunk_index)?;
                let mut lines = Vec::new();
                for line_index in 0..line_count {
                    let line = patch.line_in_hunk(hunk_index, line_index)?;
                    lines.push(DiffLine {
                        kind: match line.origin() {
                            '+' => "addition",
                            '-' => "deletion",
                            _ => "context",
                        }
                        .to_owned(),
                        old_line: line.old_lineno(),
                        new_line: line.new_lineno(),
                        content: String::from_utf8_lossy(line.content()).into_owned(),
                    });
                }
                hunks.push(DiffHunk {
                    header: String::from_utf8_lossy(hunk.header()).trim_end().to_owned(),
                    old_start: hunk.old_start(),
                    old_lines: hunk.old_lines(),
                    new_start: hunk.new_start(),
                    new_lines: hunk.new_lines(),
                    lines,
                });
            }
        }
        files.push(DiffFile {
            path,
            old_path,
            status: format!("{:?}", delta.status()).to_lowercase(),
            binary,
            hunks,
        });
    }
    Ok(DiffResult {
        mode,
        old_label,
        new_label,
        insertions: stats.insertions(),
        deletions: stats.deletions(),
        files,
    })
}

fn create_commit(repo: &Repository, message: &str) -> AppResult<Oid> {
    let mut index = repo.index()?;
    if !index.has_conflicts() && !has_staged_changes(repo)? {
        return Err(AppError::Message("There are no staged changes.".to_owned()));
    }
    let tree_id = index.write_tree()?;
    let tree = repo.find_tree(tree_id)?;
    let signature = signature(repo)?;
    match repo.head() {
        Ok(head) => {
            let parent = head.peel_to_commit()?;
            Ok(repo.commit(
                Some("HEAD"),
                &signature,
                &signature,
                message,
                &tree,
                &[&parent],
            )?)
        }
        Err(error) if error.code() == git2::ErrorCode::UnbornBranch => {
            Ok(repo.commit(Some("HEAD"), &signature, &signature, message, &tree, &[])?)
        }
        Err(error) => Err(error.into()),
    }
}

fn has_staged_changes(repo: &Repository) -> AppResult<bool> {
    let mut options = StatusOptions::new();
    options.show(git2::StatusShow::Index);
    Ok(repo
        .statuses(Some(&mut options))?
        .iter()
        .any(|entry| is_staged(entry.status())))
}
