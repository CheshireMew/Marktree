mod changes;
mod conflicts;
mod remote;
mod repository;
mod stash;
mod sync;
mod sync_commit;
mod worktrees;

pub use changes::{
    commit, compare_worktrees, diff, fetch, push, stage_all, stage_paths, unstage_paths,
};
pub use conflicts::{pending_conflicts, resolve_conflict, resolve_conflict_with_content};
pub use repository::{
    clone_repository, git_capability, has_git_capability, initialize_repository,
    refresh_repository, repository_lock_key, repository_status,
};
pub use sync::{
    abort_git_operation, pending_git_operation, pull_rebase, resume_git_operation, sync_plan,
    sync_workspace_changes,
};
pub use sync_commit::commit_workspace_baseline;
pub use worktrees::{
    checkout_branch, create_branch, create_worktree, delete_branch, list_branches, search_worktrees,
};

#[cfg(test)]
use remote::validate_remote_url;
#[cfg(test)]
use repository::status_snapshot;
#[cfg(test)]
use sync_commit::{commit_only_paths, find_operation_commit};

include!("git/tests.rs");
