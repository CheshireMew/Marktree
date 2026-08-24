use std::{
    fs,
    path::{Path, PathBuf},
};

use crate::{
    error::{AppError, AppResult},
    paths::paths_equal,
    types::{
        CredentialRecord, GitCapability, GitFileStatus, GitStatusSnapshot, WorktreeDescriptor,
    },
};
use git2::{
    build::{CheckoutBuilder, RepoBuilder},
    BranchType, Oid, Repository, RepositoryInitOptions, Signature, Status, StatusOptions,
    WorktreeLockStatus,
};

use super::remote::{fetch_options, remote_url, validate_remote_url};

pub fn repository_lock_key(path: &str) -> String {
    let repository = open_exact_repository(path).ok();
    let resolved = repository
        .as_ref()
        .and_then(|repo| fs::canonicalize(repo.commondir()).ok())
        .or_else(|| fs::canonicalize(path).ok())
        .or_else(|| {
            let path = Path::new(path);
            path.parent()
                .and_then(|parent| fs::canonicalize(parent).ok())
                .map(|parent| parent.join(path.file_name().unwrap_or_default()))
        })
        .unwrap_or_else(|| PathBuf::from(path));
    let key = resolved.to_string_lossy().into_owned();
    if cfg!(target_os = "windows") {
        key.to_lowercase()
    } else {
        key
    }
}

pub fn has_git_capability(path: &str) -> bool {
    open_exact_repository(path).is_ok()
}

pub fn git_capability(path: &str) -> AppResult<Option<GitCapability>> {
    if !Path::new(path).join(".git").exists() {
        return Ok(None);
    }
    Ok(Some(capability(&open_exact_repository(path)?)?))
}

pub fn git_capability_metadata(path: &str) -> AppResult<Option<GitCapability>> {
    if !Path::new(path).join(".git").exists() {
        return Ok(None);
    }
    Ok(Some(capability_with_status(
        &open_exact_repository(path)?,
        false,
    )?))
}

pub(super) fn open_exact_repository(root: &str) -> AppResult<Repository> {
    open_exact_repository_path(Path::new(root))
}

pub(super) fn open_exact_repository_path(root: &Path) -> AppResult<Repository> {
    let requested_root = fs::canonicalize(root)?;
    if !requested_root.is_dir() || !requested_root.join(".git").exists() {
        return Err(AppError::Message(format!(
            "The workspace root '{}' does not own a Git repository.",
            root.display()
        )));
    }
    let repository = Repository::open(&requested_root)?;
    let repository_root = repository
        .workdir()
        .ok_or_else(|| AppError::Message("A bare repository is not a workspace.".to_owned()))?;
    if !paths_equal(
        requested_root.to_string_lossy().as_ref(),
        repository_root.to_string_lossy().as_ref(),
    ) {
        return Err(AppError::Message(format!(
            "Git resolved '{}' to a different repository root '{}'.",
            requested_root.display(),
            repository_root.display()
        )));
    }
    Ok(repository)
}

pub fn initialize_repository(path: &str) -> AppResult<GitCapability> {
    fs::create_dir_all(path)?;
    let mut options = RepositoryInitOptions::new();
    options.initial_head("main");
    let repo = Repository::init_opts(path, &options)?;
    capability(&repo)
}

pub fn clone_repository(
    remote_url: &str,
    path: &str,
    credential: Option<CredentialRecord>,
) -> AppResult<GitCapability> {
    validate_remote_url(remote_url)?;
    let fetch_options = fetch_options(credential);
    let mut builder = RepoBuilder::new();
    builder.fetch_options(fetch_options);
    let repo = builder.clone(remote_url, Path::new(path))?;
    capability(&repo)
}

pub fn refresh_repository(root: &str) -> AppResult<GitCapability> {
    capability(&open_exact_repository(root)?)
}

pub fn repository_status(root: &str) -> AppResult<GitStatusSnapshot> {
    status_snapshot(&open_exact_repository(root)?)
}

fn capability(repo: &Repository) -> AppResult<GitCapability> {
    capability_with_status(repo, true)
}

fn capability_with_status(repo: &Repository, include_status: bool) -> AppResult<GitCapability> {
    let main = main_repository(repo)?;
    let main_root = workdir_string(&main)?;
    let current_root = workdir_string(repo)?;
    let mut worktrees = vec![descriptor_for_worktree_with_status(
        "main",
        Path::new(&main_root),
        true,
        false,
        include_status,
    )?];
    let worktree_names = main.worktrees()?;
    for item in worktree_names.iter() {
        let Some(name) = item? else {
            continue;
        };
        let worktree = main.find_worktree(name)?;
        let locked = worktree.is_locked()? != WorktreeLockStatus::Unlocked;
        worktrees.push(descriptor_for_worktree_with_status(
            name,
            worktree.path(),
            false,
            locked,
            include_status,
        )?);
    }
    worktrees.sort_by(|left, right| {
        right
            .is_main
            .cmp(&left.is_main)
            .then_with(|| left.name.to_lowercase().cmp(&right.name.to_lowercase()))
    });
    let common_dir = fs::canonicalize(main.commondir())?
        .to_string_lossy()
        .into_owned();
    let active = worktrees
        .iter()
        .find(|worktree| paths_equal(&worktree.path, &current_root))
        .ok_or_else(|| {
            AppError::Message(format!(
                "The active worktree '{current_root}' is missing from its repository descriptor."
            ))
        })?;
    Ok(GitCapability {
        common_dir,
        remote_url: remote_url(&main),
        status: active.status.clone(),
        worktrees,
    })
}

pub(super) fn descriptor_for_worktree(
    name: &str,
    path: &Path,
    is_main: bool,
    is_locked: bool,
) -> AppResult<WorktreeDescriptor> {
    descriptor_for_worktree_with_status(name, path, is_main, is_locked, true)
}

fn descriptor_for_worktree_with_status(
    name: &str,
    path: &Path,
    is_main: bool,
    is_locked: bool,
    include_status: bool,
) -> AppResult<WorktreeDescriptor> {
    let repo = open_exact_repository_path(path)?;
    Ok(WorktreeDescriptor {
        name: name.to_owned(),
        path: workdir_string(&repo)?,
        branch: current_branch(&repo),
        is_main,
        is_locked,
        is_detached: repo.head_detached()?,
        status: include_status.then(|| status_snapshot(&repo)).transpose()?,
    })
}

pub(super) fn status_snapshot(repo: &Repository) -> AppResult<GitStatusSnapshot> {
    let mut options = StatusOptions::new();
    options
        .include_untracked(true)
        .recurse_untracked_dirs(true)
        .renames_head_to_index(true)
        .renames_index_to_workdir(true)
        .exclude_submodules(true);
    let statuses = repo.statuses(Some(&mut options))?;
    let mut files = Vec::new();
    for entry in statuses.iter() {
        let status = entry.status();
        let path = entry
            .path()
            .ok()
            .map(str::to_owned)
            .or_else(|| {
                entry
                    .head_to_index()
                    .and_then(|delta| delta.new_file().path())
                    .and_then(Path::to_str)
                    .map(str::to_owned)
            })
            .or_else(|| {
                entry
                    .index_to_workdir()
                    .and_then(|delta| delta.new_file().path())
                    .and_then(Path::to_str)
                    .map(str::to_owned)
            })
            .unwrap_or_default()
            .replace('\\', "/");
        if path.is_empty() {
            continue;
        }
        if path
            .split('/')
            .any(crate::content_policy::is_internal_transaction_file_name)
        {
            continue;
        }
        files.push(GitFileStatus {
            path,
            index_status: index_status(status).to_owned(),
            worktree_status: worktree_status(status).to_owned(),
            staged: is_staged(status),
            conflicted: status.contains(Status::CONFLICTED),
            untracked: status.contains(Status::WT_NEW),
        });
    }
    files.sort_by_key(|entry| entry.path.to_lowercase());
    let (upstream, ahead, behind) = upstream_status(repo);
    Ok(GitStatusSnapshot {
        branch: current_branch(repo),
        upstream,
        ahead,
        behind,
        staged_count: files.iter().filter(|file| file.staged).count(),
        changed_count: files
            .iter()
            .filter(|file| file.worktree_status != "clean")
            .count(),
        untracked_count: files.iter().filter(|file| file.untracked).count(),
        conflicted_count: files.iter().filter(|file| file.conflicted).count(),
        files,
    })
}

fn upstream_status(repo: &Repository) -> (Option<String>, usize, usize) {
    let Ok(head) = repo.head() else {
        return (None, 0, 0);
    };
    let Ok(branch_name) = head.shorthand() else {
        return (None, 0, 0);
    };
    let Ok(branch) = repo.find_branch(branch_name, BranchType::Local) else {
        return (None, 0, 0);
    };
    let Ok(upstream) = branch.upstream() else {
        return (None, 0, 0);
    };
    let name = upstream
        .get()
        .shorthand()
        .ok()
        .map(str::to_owned)
        .or_else(|| upstream.name().ok().flatten().map(str::to_owned));
    let counts = head
        .target()
        .zip(upstream.get().target())
        .and_then(|(local, remote)| repo.graph_ahead_behind(local, remote).ok())
        .unwrap_or((0, 0));
    (name, counts.0, counts.1)
}

pub(super) fn upstream_commit(repo: &Repository) -> AppResult<git2::Commit<'_>> {
    let head = repo.head()?;
    let branch_name = head
        .shorthand()
        .map_err(|_| AppError::Message("Detached HEAD has no upstream.".to_owned()))?;
    let branch = repo.find_branch(branch_name, BranchType::Local)?;
    let upstream = branch
        .upstream()
        .map_err(|_| AppError::Message("The current branch has no upstream.".to_owned()))?;
    upstream.get().peel_to_commit().map_err(AppError::from)
}

pub(super) fn fast_forward(repo: &Repository, target: Oid) -> AppResult<()> {
    let mut head = repo.head()?;
    head.set_target(target, "Marktree fast-forward")?;
    repo.set_head(
        head.name()
            .map_err(|_| AppError::Message("HEAD reference has no name.".to_owned()))?,
    )?;
    let mut checkout = CheckoutBuilder::new();
    // The caller has already preserved every worktree change in the automatic
    // stash. A force checkout is required after moving the branch reference;
    // safe checkout otherwise treats the old index as user content and can
    // leave the visible worktree at the pre-fetch commit.
    checkout.force();
    repo.checkout_head(Some(&mut checkout))?;
    Ok(())
}

pub(super) fn main_repository(repo: &Repository) -> AppResult<Repository> {
    let common_dir = repo.commondir();
    let main_root = common_dir
        .parent()
        .ok_or_else(|| AppError::InvalidPath(common_dir.display().to_string()))?;
    open_exact_repository_path(main_root)
}

pub(super) fn signature(repo: &Repository) -> AppResult<Signature<'static>> {
    if let Ok(signature) = repo.signature() {
        let name = signature.name().unwrap_or("Marktree User").to_owned();
        let email = signature.email().unwrap_or("marktree@localhost").to_owned();
        return Ok(Signature::now(&name, &email)?);
    }
    Ok(Signature::now("Marktree User", "marktree@localhost")?)
}

pub(super) fn current_branch(repo: &Repository) -> Option<String> {
    repo.head()
        .ok()
        .and_then(|head| head.shorthand().ok().map(str::to_owned))
        .or_else(|| {
            repo.find_reference("HEAD")
                .ok()
                .and_then(|head| head.symbolic_target().ok().flatten().map(str::to_owned))
                .and_then(|target| target.strip_prefix("refs/heads/").map(str::to_owned))
        })
}

pub(super) fn workdir(repo: &Repository) -> AppResult<&Path> {
    repo.workdir()
        .ok_or_else(|| AppError::Message("A bare repository has no editable worktree.".to_owned()))
}

pub(super) fn workdir_string(repo: &Repository) -> AppResult<String> {
    Ok(fs::canonicalize(workdir(repo)?)?
        .to_string_lossy()
        .into_owned())
}

pub(super) fn is_staged(status: Status) -> bool {
    status.intersects(
        Status::INDEX_NEW
            | Status::INDEX_MODIFIED
            | Status::INDEX_DELETED
            | Status::INDEX_RENAMED
            | Status::INDEX_TYPECHANGE,
    )
}

fn index_status(status: Status) -> &'static str {
    if status.contains(Status::CONFLICTED) {
        "conflicted"
    } else if status.contains(Status::INDEX_NEW) {
        "added"
    } else if status.contains(Status::INDEX_DELETED) {
        "deleted"
    } else if status.contains(Status::INDEX_RENAMED) {
        "renamed"
    } else if status.contains(Status::INDEX_TYPECHANGE) {
        "typechange"
    } else if status.contains(Status::INDEX_MODIFIED) {
        "modified"
    } else {
        "clean"
    }
}

pub(super) fn worktree_status(status: Status) -> &'static str {
    if status.contains(Status::CONFLICTED) {
        "conflicted"
    } else if status.contains(Status::WT_NEW) {
        "untracked"
    } else if status.contains(Status::WT_DELETED) {
        "deleted"
    } else if status.contains(Status::WT_RENAMED) {
        "renamed"
    } else if status.contains(Status::WT_TYPECHANGE) {
        "typechange"
    } else if status.contains(Status::WT_MODIFIED) {
        "modified"
    } else {
        "clean"
    }
}

#[cfg(test)]
mod transaction_artifact_tests {
    use super::*;

    #[test]
    fn git_status_never_reports_operation_owned_workspace_files() {
        let directory = tempfile::TempDir::new().unwrap();
        let repository = Repository::init(directory.path()).unwrap();
        fs::write(directory.path().join("note.md"), b"visible").unwrap();
        fs::write(
            directory
                .path()
                .join(".note.md.marktree-0123456789abcdef01234567.tmp"),
            b"staged",
        )
        .unwrap();

        let status = status_snapshot(&repository).unwrap();

        assert_eq!(status.files.len(), 1);
        assert_eq!(status.files[0].path, "note.md");
    }
}
