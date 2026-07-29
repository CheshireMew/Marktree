use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RepositoryDescriptor {
    pub id: String,
    pub name: String,
    pub root: String,
    pub common_dir: String,
    pub remote_url: Option<String>,
    pub worktrees: Vec<WorktreeDescriptor>,
    pub status: GitStatusSnapshot,
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
    pub status: GitStatusSnapshot,
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
pub struct DocumentDescriptor {
    pub path: String,
    pub name: String,
    pub extension: String,
    pub size: u64,
    pub modified_ms: u64,
    pub read_only: bool,
    pub kind: DocumentKind,
    pub git_status: Option<GitFileStatus>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum DocumentKind {
    Markdown,
    Text,
    Image,
    Other,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TextEncoding {
    Utf8,
    Utf8Bom,
    Unsupported,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum LineEnding {
    Lf,
    Crlf,
    Cr,
    Mixed,
    None,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DocumentContent {
    pub path: String,
    pub content: String,
    pub modified_ms: u64,
    pub sha256: String,
    pub read_only: bool,
    pub encoding: TextEncoding,
    pub line_ending: LineEnding,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RepositoryConfig {
    #[serde(default = "default_assets_dir")]
    pub assets_dir: String,
    #[serde(default)]
    pub ignore_rules: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RepositoryConfigSnapshot {
    pub config: RepositoryConfig,
    pub sha256: Option<String>,
    pub missing: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveRepositoryConfigRequest {
    pub root: String,
    pub config: RepositoryConfig,
    pub expected_sha256: Option<String>,
    pub expected_missing: bool,
}

impl Default for RepositoryConfig {
    fn default() -> Self {
        Self {
            assets_dir: default_assets_dir(),
            ignore_rules: Vec::new(),
        }
    }
}

fn default_assets_dir() -> String {
    "assets".to_owned()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveDocumentRequest {
    pub root: String,
    pub path: String,
    pub content: String,
    pub expected_sha256: Option<String>,
    #[serde(default)]
    pub expected_missing: bool,
    pub encoding: TextEncoding,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveDocumentResult {
    pub path: String,
    pub modified_ms: u64,
    pub sha256: String,
    pub encoding: TextEncoding,
    pub line_ending: LineEnding,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AssetWriteResult {
    pub path: String,
    pub markdown_path: String,
    pub sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AssetPreview {
    pub path: String,
    pub media_type: String,
    pub base64_data: String,
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ManagedChangeKind {
    Document,
    Asset,
    RepositoryConfig,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ManagedChange {
    pub path: String,
    pub sha256: String,
    pub generation: u64,
    pub kind: ManagedChangeKind,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PendingGitOperation {
    pub id: String,
    pub root: String,
    pub kind: GitOperationKind,
    pub phase: GitOperationPhase,
    pub started_at: String,
    pub managed_changes: Vec<ManagedChange>,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CredentialInput {
    pub id: String,
    pub username: String,
    pub token: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CredentialRecord {
    pub username: String,
    pub token: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GithubDeviceCode {
    pub device_code: String,
    pub user_code: String,
    pub verification_uri: String,
    pub expires_in: u64,
    pub interval: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GithubDeviceToken {
    pub access_token: Option<String>,
    pub token_type: Option<String>,
    pub scope: Option<String>,
    pub pending: bool,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalStateSnapshot {
    pub repositories: Vec<String>,
    pub managed_changes: BTreeMap<String, Vec<ManagedChange>>,
    pub pending_git_operations: BTreeMap<String, PendingGitOperation>,
    pub recent_files: Vec<String>,
    pub credential_refs: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RepositoryChangedEvent {
    pub root: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RepositoryWatchErrorEvent {
    pub root: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RepositoryForgottenEvent {
    pub repository_id: String,
    pub worktree_roots: Vec<String>,
}
