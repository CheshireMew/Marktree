use serde::{Deserialize, Serialize};

mod auth;
mod documents;
mod git;
mod portability;
mod state;

pub use auth::*;
pub use documents::*;
pub use git::*;
pub use portability::*;
pub use state::*;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceDescriptor {
    pub id: String,
    pub name: String,
    pub root: String,
    pub git: Option<GitCapability>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceEntry {
    pub path: String,
    pub name: String,
    pub entry_type: WorkspaceEntryType,
    pub file_kind: Option<DocumentKind>,
    pub size: u64,
    pub modified_ms: u64,
    pub read_only: bool,
    pub git_status: Option<GitFileStatus>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceViewSnapshot {
    pub entries: Vec<WorkspaceEntry>,
    pub status: Option<GitStatusSnapshot>,
    pub branches: Vec<BranchDescriptor>,
    pub pending_operation: Option<PendingGitOperationSummary>,
    pub conflicts: Vec<ConflictRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceEntriesPatch {
    pub entries: Vec<WorkspaceEntry>,
    pub removed_paths: Vec<String>,
    pub status: Option<GitStatusSnapshot>,
    pub full_reload_required: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceRefreshSnapshot {
    pub workspace: WorkspaceDescriptor,
    pub view: WorkspaceViewSnapshot,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitBaselinePreview {
    pub file_count: usize,
    pub total_bytes: u64,
    pub ignored_count: usize,
    pub ignore_rules: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MoveWorkspaceEntryRequest {
    pub root: String,
    pub source_path: String,
    pub destination_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DuplicateWorkspaceEntryRequest {
    pub root: String,
    pub source_path: String,
    pub destination_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceEntryMoveResult {
    pub old_path: String,
    pub new_path: String,
    pub moved_files: Vec<WorkspacePathMove>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceEntryDuplicateResult {
    pub source_path: String,
    pub new_path: String,
    pub copied_files: Vec<WorkspacePathMove>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspacePathMove {
    pub old_path: String,
    pub new_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TrashEntry {
    pub id: String,
    pub workspace_root: String,
    pub original_path: String,
    pub name: String,
    pub deleted_at: String,
}
