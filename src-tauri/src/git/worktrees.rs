use std::{fs, path::PathBuf};

use git2::{
    build::CheckoutBuilder, BranchType, Reference, Repository, RepositoryState, WorktreeAddOptions,
};

use crate::{
    error::{AppError, AppResult},
    types::{
        BranchDescriptor, CreateWorktreeRequest, GitStatusSnapshot, WorktreeDescriptor,
        WorktreeSearchResult,
    },
};

use super::repository::{
    current_branch, descriptor_for_worktree, main_repository, status_snapshot, workdir_string,
};

pub fn list_branches(root: &str) -> AppResult<Vec<BranchDescriptor>> {
    let repo = Repository::open(root)?;
    let mut branches = Vec::new();
    for item in repo.branches(Some(BranchType::Local))? {
        let (branch, _) = item?;
        if let Some(name) = branch.name()? {
            let upstream = branch.upstream().ok();
            let upstream_name = upstream
                .as_ref()
                .and_then(|value| value.get().shorthand().ok())
                .map(str::to_owned);
            let (ahead, behind) = branch
                .get()
                .target()
                .zip(upstream.as_ref().and_then(|value| value.get().target()))
                .and_then(|(local, remote)| repo.graph_ahead_behind(local, remote).ok())
                .unwrap_or((0, 0));
            branches.push(BranchDescriptor {
                name: name.to_owned(),
                is_current: current_branch(&repo).as_deref() == Some(name),
                upstream: upstream_name,
                ahead,
                behind,
                checked_out_path: checked_out_branch_path(&repo, name)?,
            });
        }
    }
    branches.sort_by(|left, right| {
        right
            .is_current
            .cmp(&left.is_current)
            .then_with(|| left.name.to_lowercase().cmp(&right.name.to_lowercase()))
    });
    Ok(branches)
}

pub fn create_branch(
    root: &str,
    name: &str,
    start_point: Option<&str>,
    checkout: bool,
) -> AppResult<GitStatusSnapshot> {
    validate_branch_name(name)?;
    let repo = Repository::open(root)?;
    if repo.find_branch(name, BranchType::Local).is_ok() {
        return Err(AppError::Message(format!(
            "The branch '{name}' already exists."
        )));
    }
    let start = repo
        .revparse_single(start_point.unwrap_or("HEAD"))?
        .peel_to_commit()?;
    repo.branch(name, &start, false)?;
    if checkout {
        if let Err(error) = checkout_branch_in_repository(&repo, name) {
            let rollback = repo
                .find_branch(name, BranchType::Local)
                .and_then(|mut branch| branch.delete());
            if let Err(rollback_error) = rollback {
                return Err(AppError::Message(format!(
                    "{error} The newly created branch also could not be rolled back: {rollback_error}"
                )));
            }
            return Err(error);
        }
    }
    status_snapshot(&repo)
}

pub fn checkout_branch(root: &str, name: &str) -> AppResult<GitStatusSnapshot> {
    validate_branch_name(name)?;
    let repo = Repository::open(root)?;
    checkout_branch_in_repository(&repo, name)?;
    status_snapshot(&repo)
}

pub fn delete_branch(root: &str, name: &str) -> AppResult<Vec<BranchDescriptor>> {
    validate_branch_name(name)?;
    let repo = Repository::open(root)?;
    if let Some(path) = checked_out_branch_path(&repo, name)? {
        return Err(AppError::Message(format!(
            "The branch '{name}' is checked out at {path}."
        )));
    }
    let mut branch = repo.find_branch(name, BranchType::Local)?;
    branch.delete()?;
    list_branches(root)
}

pub fn create_worktree(request: CreateWorktreeRequest) -> AppResult<WorktreeDescriptor> {
    validate_worktree_name(&request.name)?;
    let repo = main_repository(&Repository::open(&request.root)?)?;
    let path = PathBuf::from(&request.path);
    if path.exists() {
        return Err(AppError::Message(
            "The worktree directory already exists.".to_owned(),
        ));
    }

    let mut created_branch = false;
    let reference = match repo.find_branch(&request.branch, BranchType::Local) {
        Ok(branch) => branch.into_reference(),
        Err(error) if error.code() == git2::ErrorCode::NotFound => {
            let start = request.start_point.as_deref().unwrap_or("HEAD");
            let commit = repo.revparse_single(start)?.peel_to_commit()?;
            created_branch = true;
            repo.branch(&request.branch, &commit, false)?
                .into_reference()
        }
        Err(error) => return Err(error.into()),
    };
    let mut options = WorktreeAddOptions::new();
    options.reference(Some(&reference));
    if let Err(error) = repo.worktree(&request.name, &path, Some(&options)) {
        if created_branch {
            let rollback = repo
                .find_branch(&request.branch, BranchType::Local)
                .and_then(|mut branch| branch.delete());
            if let Err(rollback_error) = rollback {
                return Err(AppError::Message(format!(
                    "{error} The newly created branch also could not be rolled back: {rollback_error}"
                )));
            }
        }
        return Err(error.into());
    }
    descriptor_for_worktree(&request.name, &path, false, false)
}

pub fn search_worktrees(
    root: &str,
    query: &str,
    limit: usize,
    is_current: impl Fn() -> bool,
) -> AppResult<Vec<WorktreeSearchResult>> {
    let repo = main_repository(&Repository::open(root)?)?;
    let mut worktrees = vec![("main".to_owned(), workdir_string(&repo)?)];
    let names = repo.worktrees()?;
    for item in names.iter() {
        let Some(name) = item? else {
            continue;
        };
        let worktree = repo.find_worktree(name)?;
        worktrees.push((
            name.to_owned(),
            fs::canonicalize(worktree.path())?
                .to_string_lossy()
                .into_owned(),
        ));
    }
    let mut results = Vec::new();
    for (worktree, worktree_root) in worktrees {
        if !is_current() {
            break;
        }
        for path in crate::documents::search_documents(
            &worktree_root,
            query,
            limit.saturating_sub(results.len()).max(1),
            &is_current,
        )? {
            results.push(WorktreeSearchResult {
                worktree: worktree.clone(),
                root: worktree_root.clone(),
                path,
            });
            if results.len() >= limit.max(1) {
                return Ok(results);
            }
        }
    }
    Ok(results)
}

fn checkout_branch_in_repository(repo: &Repository, name: &str) -> AppResult<()> {
    if current_branch(repo).as_deref() == Some(name) {
        return Ok(());
    }
    if repo.state() != RepositoryState::Clean {
        return Err(AppError::Message(
            "Finish the current Git operation before switching branches.".to_owned(),
        ));
    }
    let status = status_snapshot(repo)?;
    if !status.files.is_empty() {
        return Err(AppError::Message(
            "Commit or discard the working tree changes before switching branches.".to_owned(),
        ));
    }
    if let Some(path) = checked_out_branch_path(repo, name)? {
        let current_path = workdir_string(repo)?;
        if !path.eq_ignore_ascii_case(&current_path) {
            return Err(AppError::Message(format!(
                "The branch '{name}' is already checked out at {path}."
            )));
        }
    }
    let branch = repo.find_branch(name, BranchType::Local)?;
    let commit = branch.get().peel_to_commit()?;
    let mut checkout = CheckoutBuilder::new();
    checkout.safe();
    repo.checkout_tree(commit.as_object(), Some(&mut checkout))?;
    repo.set_head(&format!("refs/heads/{name}"))?;
    Ok(())
}

fn checked_out_branch_path(repo: &Repository, name: &str) -> AppResult<Option<String>> {
    let main = main_repository(repo)?;
    if current_branch(&main).as_deref() == Some(name) {
        return Ok(Some(workdir_string(&main)?));
    }
    for item in main.worktrees()?.iter() {
        let Some(worktree_name) = item? else {
            continue;
        };
        let worktree = main.find_worktree(worktree_name)?;
        let Ok(worktree_repo) = Repository::open(worktree.path()) else {
            continue;
        };
        if current_branch(&worktree_repo).as_deref() == Some(name) {
            return Ok(Some(workdir_string(&worktree_repo)?));
        }
    }
    Ok(None)
}

fn validate_worktree_name(name: &str) -> AppResult<()> {
    if name.trim().is_empty()
        || name.contains('/')
        || name.contains('\\')
        || name == "."
        || name == ".."
    {
        return Err(AppError::Message("Invalid worktree name.".to_owned()));
    }
    Ok(())
}

fn validate_branch_name(name: &str) -> AppResult<()> {
    let name = name.trim();
    if name.is_empty() || !Reference::is_valid_name(&format!("refs/heads/{name}")) {
        return Err(AppError::Message("Invalid branch name.".to_owned()));
    }
    Ok(())
}
