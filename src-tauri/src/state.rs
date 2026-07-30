mod persistent;
mod runtime;

pub use persistent::PersistentState;
pub use runtime::WorkspaceRuntime;

#[cfg(test)]
mod tests {
    use std::fs;

    use chrono::Utc;
    use tempfile::TempDir;

    use super::*;
    use crate::types::{
        GitOperationKind, GitOperationPhase, PendingGitOperation, WorkspaceChangeOperation,
    };

    #[test]
    fn compare_and_clear_preserves_a_newer_save_of_the_same_path() {
        let directory = TempDir::new().unwrap();
        let state = PersistentState::load(directory.path()).unwrap();
        let first = state
            .record_workspace_change(
                "C:\\repo",
                "notes/shared.md",
                WorkspaceChangeOperation::Upsert,
                Some("first"),
            )
            .unwrap();
        let second = state
            .record_workspace_change(
                "C:\\repo",
                "notes/shared.md",
                WorkspaceChangeOperation::Upsert,
                Some("second"),
            )
            .unwrap();

        state.clear_workspace_changes("C:\\repo", &[first]).unwrap();

        assert_eq!(state.workspace_changes("C:\\repo"), vec![second]);
    }

    #[test]
    fn corrupted_primary_state_recovers_the_last_valid_backup() {
        let directory = TempDir::new().unwrap();
        {
            let state = PersistentState::load(directory.path()).unwrap();
            state.register_workspace("C:\\first").unwrap();
            state.register_workspace("C:\\second").unwrap();
        }
        fs::write(directory.path().join("state.json"), b"not json").unwrap();

        let recovered = PersistentState::load(directory.path()).unwrap();

        assert_eq!(recovered.snapshot().workspaces, vec!["C:\\first"]);
    }

    #[test]
    fn unreadable_state_without_a_valid_backup_is_reported_instead_of_replaced() {
        let directory = TempDir::new().unwrap();
        let state_path = directory.path().join("state.json");
        fs::write(&state_path, b"not json").unwrap();

        let error = match PersistentState::load(directory.path()) {
            Ok(_) => panic!("corrupt state should not be replaced"),
            Err(error) => error.to_string(),
        };

        assert!(error.contains("local state is unreadable"));
        assert_eq!(fs::read(&state_path).unwrap(), b"not json");
    }

    #[test]
    fn pending_git_operation_survives_a_state_reload() {
        let directory = TempDir::new().unwrap();
        let operation = PendingGitOperation {
            id: "1234567890abcdef1234".to_owned(),
            root: "C:\\repo".to_owned(),
            kind: GitOperationKind::Sync,
            phase: GitOperationPhase::Push,
            started_at: Utc::now().to_rfc3339(),
            workspace_changes: Vec::new(),
            changed_paths: Vec::new(),
            committed: true,
            commit_id: Some("abc".to_owned()),
            pulled: true,
            pushed: false,
            original_head_oid: Some("def".to_owned()),
            stash_oid: None,
            aborting: false,
            stash_apply_started: false,
            stash_applied: false,
        };
        {
            let state = PersistentState::load(directory.path()).unwrap();
            state.begin_git_operation(operation.clone()).unwrap();
        }

        let recovered = PersistentState::load(directory.path()).unwrap();

        assert_eq!(recovered.pending_git_operation("C:\\repo"), Some(operation));
    }

    #[test]
    fn schema_five_state_is_migrated_and_immediately_rewritten() {
        let directory = TempDir::new().unwrap();
        let previous = serde_json::json!({
            "schemaVersion": 5,
            "nextGeneration": 4,
            "repositories": ["C:\\repo"],
            "managedChanges": {},
            "pendingGitOperations": {
                "C:\\repo": {
                    "id": "1234567890abcdef1234",
                    "root": "C:\\repo",
                    "kind": "sync",
                    "phase": "push",
                    "startedAt": "2026-07-29T00:00:00Z",
                    "managedChanges": [],
                    "changedPaths": ["notes/day.md"],
                    "committed": true,
                    "commitId": "abc",
                    "stashOid": null,
                    "stashApplied": false
                }
            },
            "recentFiles": ["C:\\repo\nnotes/day.md"],
            "credentialRefs": {"C:\\repo\\.git": "credential"}
        });
        fs::write(
            directory.path().join("state.json"),
            serde_json::to_vec(&previous).unwrap(),
        )
        .unwrap();

        let migrated = PersistentState::load(directory.path()).unwrap().snapshot();

        assert_eq!(migrated.workspaces, vec!["C:\\repo"]);
        assert_eq!(migrated.recent_files, vec!["C:\\repo\nnotes/day.md"]);
        assert_eq!(
            migrated.credential_refs.get("C:\\repo\\.git"),
            Some(&"credential".to_owned())
        );
        let operation = migrated.pending_git_operations.get("C:\\repo").unwrap();
        assert!(!operation.pulled);
        assert!(!operation.pushed);
        assert_eq!(operation.original_head_oid, None);
        assert!(!operation.aborting);
        assert!(!operation.stash_apply_started);
        let rewritten: serde_json::Value =
            serde_json::from_slice(&fs::read(directory.path().join("state.json")).unwrap())
                .unwrap();
        assert_eq!(rewritten["schemaVersion"], 6);
        assert!(rewritten.get("repositories").is_none());
        assert!(rewritten.get("managedChanges").is_none());
    }

    #[test]
    fn newer_search_generation_cancels_the_previous_scan() {
        let runtime = WorkspaceRuntime::default();

        let first = runtime.begin_search("workspace");
        let second = runtime.begin_search("workspace");

        assert!(!runtime.is_search_current("workspace", first));
        assert!(runtime.is_search_current("workspace", second));
    }

    #[test]
    fn forgetting_a_workspace_removes_persistent_and_runtime_records() {
        let directory = TempDir::new().unwrap();
        let state = PersistentState::load(directory.path()).unwrap();
        let runtime = WorkspaceRuntime::default();
        let main = "C:\\repo";
        let worktree = "C:\\repo-draft";
        let roots = [main.to_owned(), worktree.to_owned()];
        let credential_key = "C:\\repo\\.git";
        state.register_workspace(main).unwrap();
        state
            .record_workspace_change(
                main,
                "main.md",
                WorkspaceChangeOperation::Upsert,
                Some("main"),
            )
            .unwrap();
        state
            .record_workspace_change(
                worktree,
                "draft.md",
                WorkspaceChangeOperation::Upsert,
                Some("draft"),
            )
            .unwrap();
        state.remember_file(main, "main.md").unwrap();
        state.remember_file(worktree, "draft.md").unwrap();
        state
            .set_credential_ref(credential_key, "credential")
            .unwrap();
        assert_eq!(runtime.begin_search(main), 1);
        assert_eq!(runtime.begin_search(worktree), 1);

        state
            .forget_workspace(main, &roots, credential_key)
            .unwrap();
        runtime.forget_roots(&roots);
        let snapshot = state.snapshot();

        assert!(snapshot.workspaces.is_empty());
        assert!(snapshot.workspace_changes.is_empty());
        assert!(snapshot.recent_files.is_empty());
        assert!(snapshot.credential_refs.is_empty());
        assert_eq!(runtime.begin_search(main), 1);
        assert_eq!(runtime.begin_search(worktree), 1);
    }
}
