use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum WorkspaceEntryType {
    Directory,
    File,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum DocumentKind {
    Markdown,
    Text,
    Image,
    Pdf,
    Audio,
    Video,
    Other,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum DocumentSearchMatchType {
    Path,
    Content,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DocumentSearchResult {
    pub path: String,
    pub line: Option<usize>,
    pub column: Option<usize>,
    pub snippet: String,
    pub match_type: DocumentSearchMatchType,
    pub file_kind: DocumentKind,
    pub modified_ms: u64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchStatistics {
    pub scanned_files: usize,
    pub scanned_bytes: u64,
    pub skipped_large_files: usize,
    pub truncated: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DocumentSearchResponse {
    pub results: Vec<DocumentSearchResult>,
    pub statistics: SearchStatistics,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DocumentSearchRequest {
    pub root: String,
    pub query: String,
    pub limit: usize,
    pub path_prefix: Option<String>,
    pub file_kinds: Vec<DocumentKind>,
    pub modified_after_ms: Option<u64>,
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
pub struct WorkspaceConfig {
    #[serde(default = "default_assets_dir")]
    pub assets_dir: String,
    #[serde(default)]
    pub ignore_rules: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceConfigSnapshot {
    pub config: WorkspaceConfig,
    pub sha256: Option<String>,
    pub missing: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveWorkspaceConfigRequest {
    pub root: String,
    pub config: WorkspaceConfig,
    pub expected_sha256: Option<String>,
    pub expected_missing: bool,
}

impl Default for WorkspaceConfig {
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
pub struct BeginAssetUploadRequest {
    pub root: String,
    pub document_path: String,
    pub file_name: String,
    pub assets_dir: Option<String>,
    pub total_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AssetUploadTicket {
    pub id: String,
    pub max_chunk_bytes: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AssetUploadChunkRequest {
    pub upload_id: String,
    pub offset: u64,
    pub base64_data: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceFilePreview {
    pub path: String,
    pub kind: DocumentKind,
    pub media_type: String,
    pub resource_path: String,
}
