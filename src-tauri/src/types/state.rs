#[cfg(test)]
use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

#[cfg(test)]
use super::{PendingGitOperation, WorkspaceChange};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum OperationLogCategory {
    Workspace,
    Git,
    Recovery,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum OperationLogOutcome {
    Started,
    Progress,
    Succeeded,
    Cancelled,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OperationLogEntry {
    pub timestamp: String,
    pub category: OperationLogCategory,
    pub action: String,
    pub phase: String,
    pub outcome: OperationLogOutcome,
    pub root: Option<String>,
    pub operation_id: Option<String>,
    pub error_code: Option<crate::error::ErrorCode>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StartupState {
    pub workspaces: Vec<String>,
    pub recent_files: Vec<String>,
    pub recent_file_limit: usize,
}

#[cfg(test)]
#[derive(Debug, Clone, Default)]
pub struct LocalStateSnapshot {
    pub workspaces: Vec<String>,
    pub workspace_changes: BTreeMap<String, Vec<WorkspaceChange>>,
    pub pending_git_operations: BTreeMap<String, PendingGitOperation>,
    pub recent_files: Vec<String>,
    pub credential_refs: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceChangedEvent {
    pub root: String,
    pub paths: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceWatchErrorEvent {
    pub root: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceForgottenEvent {
    pub workspace_id: String,
    pub roots: Vec<String>,
}

#[cfg(test)]
mod wire_contract_tests {
    use super::*;
    use crate::types::{GitOperationKind, GitOperationPhase, PendingGitOperationSummary};

    #[test]
    fn public_state_and_pending_operation_use_the_actual_camel_case_wire_shape() {
        let recent_file_limit = crate::state::RECENT_FILE_LIMIT;
        let startup = serde_json::to_value(StartupState {
            workspaces: vec!["C:\\notes".to_owned()],
            recent_files: vec!["C:\\notes\nnote.md".to_owned()],
            recent_file_limit,
        })
        .unwrap();
        assert_eq!(
            startup,
            serde_json::json!({
                "workspaces": ["C:\\notes"],
                "recentFiles": ["C:\\notes\nnote.md"],
                "recentFileLimit": recent_file_limit
            })
        );

        let summary = PendingGitOperationSummary {
            id: "operation".to_owned(),
            root: "C:\\notes".to_owned(),
            kind: GitOperationKind::Sync,
            phase: GitOperationPhase::RestoreWorkingTree,
            started_at: "2026-01-01T00:00:00Z".to_owned(),
            aborting: true,
        };
        assert_eq!(
            serde_json::to_value(summary).unwrap(),
            serde_json::json!({
                "id": "operation",
                "root": "C:\\notes",
                "kind": "sync",
                "phase": "restoreWorkingTree",
                "startedAt": "2026-01-01T00:00:00Z",
                "aborting": true
            })
        );
    }
}
