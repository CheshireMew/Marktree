use serde::{Deserialize, Serialize};

use super::WorkspaceDescriptor;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceArchiveExportResult {
    pub file_count: usize,
    pub total_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceArchiveImportResult {
    pub workspace: WorkspaceDescriptor,
    pub file_count: usize,
    pub total_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AndroidShareImportResult {
    pub workspace: WorkspaceDescriptor,
    pub open_path: Option<String>,
    pub insert_markdown: Option<String>,
    pub archive_imported: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum AndroidShareKind {
    Text,
    Markdown,
    Image,
    Attachment,
    Archive,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PendingAndroidShare {
    pub text: Option<String>,
    pub subject: Option<String>,
    pub file_path: Option<String>,
    pub file_name: Option<String>,
    pub media_type: Option<String>,
    pub kind: AndroidShareKind,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportAndroidShareRequest {
    pub share: PendingAndroidShare,
    pub root: Option<String>,
    pub target_directory: String,
    pub document_path: Option<String>,
}
