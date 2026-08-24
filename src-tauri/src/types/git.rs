use serde::{Deserialize, Serialize};

use super::{DocumentKind, DocumentSearchMatchType, SearchStatistics};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitCapability {
    pub common_dir: String,
    pub remote_url: Option<String>,
    pub worktrees: Vec<WorktreeDescriptor>,
    pub status: Option<GitStatusSnapshot>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorktreeDescriptor {
    pub name: String,
    pub path: String,
    pub branch: Option<String>,
    pub is_main: bool,
    pub is_locked: bool,
    pub is_detached: bool,
    pub status: Option<GitStatusSnapshot>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateWorktreeRequest {
    pub root: String,
    pub name: String,
    pub path: String,
    pub branch: String,
    pub start_point: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BranchDescriptor {
    pub name: String,
    pub is_current: bool,
    pub upstream: Option<String>,
    pub ahead: usize,
    pub behind: usize,
    pub checked_out_path: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitStatusSnapshot {
    pub branch: Option<String>,
    pub upstream: Option<String>,
    pub ahead: usize,
    pub behind: usize,
    pub staged_count: usize,
    pub changed_count: usize,
    pub untracked_count: usize,
    pub conflicted_count: usize,
    pub files: Vec<GitFileStatus>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitFileStatus {
    pub path: String,
    pub index_status: String,
    pub worktree_status: String,
    pub staged: bool,
    pub conflicted: bool,
    pub untracked: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum DiffMode {
    WorktreeToIndex,
    IndexToHead,
    WorktreeToHead,
    LocalToUpstream,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ConflictChoice {
    Local,
    Remote,
    Merged,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ConflictKind {
    Text,
    Binary,
    DeleteModify,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum GitOperationKind {
    Pull,
    Sync,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum GitOperationPhase {
    Prepare,
    Commit,
    Fetch,
    PreserveWorkingTree,
    Rebase,
    RestoreWorkingTree,
    Push,
    Finalize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SyncStage {
    Prepare,
    Credential,
    Stage,
    Commit,
    Fetch,
    PreserveWorkingTree,
    Rebase,
    RestoreWorkingTree,
    Push,
    Finalize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiffResult {
    pub mode: DiffMode,
    pub old_label: String,
    pub new_label: String,
    pub insertions: usize,
    pub deletions: usize,
    pub truncated: bool,
    pub omitted_lines: usize,
    pub files: Vec<DiffFile>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiffFile {
    pub path: String,
    pub old_path: Option<String>,
    pub status: String,
    pub binary: bool,
    pub hunks: Vec<DiffHunk>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiffHunk {
    pub header: String,
    pub old_start: u32,
    pub old_lines: u32,
    pub new_start: u32,
    pub new_lines: u32,
    pub lines: Vec<DiffLine>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiffLine {
    pub kind: String,
    pub old_line: Option<u32>,
    pub new_line: Option<u32>,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TextComparison {
    pub path: String,
    pub left_label: String,
    pub right_label: String,
    pub left: String,
    pub right: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorktreeSearchResult {
    pub worktree: String,
    pub root: String,
    pub path: String,
    pub line: Option<usize>,
    pub column: Option<usize>,
    pub snippet: String,
    pub match_type: DocumentSearchMatchType,
    pub file_kind: DocumentKind,
    pub modified_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorktreeSearchResponse {
    pub results: Vec<WorktreeSearchResult>,
    pub statistics: SearchStatistics,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorktreeSearchRequest {
    pub root: String,
    pub query: String,
    pub limit: usize,
    pub path_prefix: Option<String>,
    pub file_kinds: Vec<DocumentKind>,
    pub modified_after_ms: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncPlan {
    pub root: String,
    pub branch: Option<String>,
    pub changed_paths: Vec<String>,
    pub remote_url: Option<String>,
    pub can_push: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum WorkspaceChangeOperation {
    Upsert,
    Delete,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceChange {
    pub path: String,
    pub generation: u64,
    pub operation: WorkspaceChangeOperation,
    pub version: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PendingGitOperation {
    pub id: String,
    pub root: String,
    pub kind: GitOperationKind,
    pub phase: GitOperationPhase,
    pub started_at: String,
    pub workspace_changes: Vec<WorkspaceChange>,
    pub changed_paths: Vec<String>,
    pub committed: bool,
    pub commit_id: Option<String>,
    #[serde(default)]
    pub pulled: bool,
    #[serde(default)]
    pub pushed: bool,
    #[serde(default)]
    pub original_head_oid: Option<String>,
    pub stash_oid: Option<String>,
    #[serde(default)]
    pub aborting: bool,
    #[serde(default)]
    pub stash_apply_started: bool,
    pub stash_applied: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PendingGitOperationSummary {
    pub id: String,
    pub root: String,
    pub kind: GitOperationKind,
    pub phase: GitOperationPhase,
    pub started_at: String,
    pub aborting: bool,
}

impl From<&PendingGitOperation> for PendingGitOperationSummary {
    fn from(operation: &PendingGitOperation) -> Self {
        Self {
            id: operation.id.clone(),
            root: operation.root.clone(),
            kind: operation.kind,
            phase: operation.phase,
            started_at: operation.started_at.clone(),
            aborting: operation.aborting,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncResult {
    pub committed: bool,
    pub commit_id: Option<String>,
    pub pulled: bool,
    pub pushed: bool,
    pub changed_paths: Vec<String>,
    pub conflicts: Vec<ConflictRecord>,
    pub failure_stage: Option<SyncStage>,
    pub error: Option<crate::error::ErrorPayload>,
}

impl SyncResult {
    pub fn failure(
        stage: SyncStage,
        error: impl Into<crate::error::AppError>,
        changed_paths: Vec<String>,
    ) -> Self {
        let error = error.into();
        Self {
            committed: false,
            commit_id: None,
            pulled: false,
            pushed: false,
            changed_paths,
            conflicts: Vec::new(),
            failure_stage: Some(stage),
            error: Some(error.payload()),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConflictRecord {
    pub path: String,
    pub kind: ConflictKind,
    pub ancestor: Option<String>,
    pub local: Option<String>,
    pub remote: Option<String>,
    pub ancestor_exists: bool,
    pub local_exists: bool,
    pub remote_exists: bool,
    pub recovery_id: String,
    pub choice: Option<ConflictChoice>,
}
