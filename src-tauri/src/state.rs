mod persistent;
mod runtime;

pub use persistent::PersistentState;
pub use runtime::{WorkspaceRuntime, MAX_SEARCH_RESULTS};

pub const RECENT_FILE_LIMIT: usize = 40;

#[cfg(test)]
mod tests {
    use std::fs;

    use chrono::Utc;
    use tempfile::TempDir;

    use super::*;
    use crate::types::{GitOperationKind, GitOperationPhase, PendingGitOperation};
    use crate::workspace_operation::WorkspaceChangeIntent;

    #[test]
    fn compare_and_clear_preserves_a_newer_save_of_the_same_path() {
        let directory = TempDir::new().unwrap();
        let state = PersistentState::load(directory.path()).unwrap();
        state.seed_workspace_changes(
            "C:\\repo",
            &[WorkspaceChangeIntent::upsert("notes/shared.md", "first")],
        );
        let first = state.workspace_changes("C:\\repo").remove(0);
        state.seed_workspace_changes(
            "C:\\repo",
            &[WorkspaceChangeIntent::upsert("notes/shared.md", "second")],
        );
        let second = state.workspace_changes("C:\\repo").remove(0);

        state.clear_workspace_changes("C:\\repo", &[first]).unwrap();

        assert_eq!(state.workspace_changes("C:\\repo"), vec![second]);
    }

    #[test]
    fn live_process_state_instances_merge_updates_from_the_shared_file() {
        let directory = TempDir::new().unwrap();
        let first = PersistentState::load(directory.path()).unwrap();
        let second = PersistentState::load(directory.path()).unwrap();

        first.register_workspace("C:\\first").unwrap();
        second.register_workspace("C:\\second").unwrap();

        let first_view = first.snapshot();
        let second_view = second.snapshot();
        assert_eq!(first_view.workspaces, vec!["C:\\first", "C:\\second"]);
        assert_eq!(second_view.workspaces, first_view.workspaces);
    }

    #[test]
    fn performance_registering_an_existing_workspace_does_not_rewrite_or_rotate_state() {
        let directory = TempDir::new().unwrap();
        let state = PersistentState::load(directory.path()).unwrap();
        state.register_workspace("C:\\same").unwrap();
        let primary = fs::read(directory.path().join("state.json")).unwrap();

        state.register_workspace("C:\\same").unwrap();

        assert_eq!(
            fs::read(directory.path().join("state.json")).unwrap(),
            primary
        );
        assert!(!directory.path().join("state.backup.json").exists());
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
    fn schema_five_state_is_migrated_to_the_current_schema_and_immediately_rewritten() {
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
        assert_eq!(rewritten["schemaVersion"], 7);
        assert!(rewritten.get("repositories").is_none());
        assert!(rewritten.get("managedChanges").is_none());
        assert_eq!(
            rewritten["pendingWorkspaceOperations"],
            serde_json::json!({})
        );
    }

    #[test]
    fn schema_six_state_adds_the_workspace_operation_journal_without_losing_state() {
        let directory = TempDir::new().unwrap();
        let previous = serde_json::json!({
            "schemaVersion": 6,
            "nextGeneration": 9,
            "workspaces": ["C:\\repo"],
            "workspaceChanges": {},
            "pendingGitOperations": {},
            "recentFiles": ["C:\\repo\nnotes/day.md"],
            "credentialRefs": {}
        });
        fs::write(
            directory.path().join("state.json"),
            serde_json::to_vec(&previous).unwrap(),
        )
        .unwrap();

        let migrated = PersistentState::load(directory.path()).unwrap().snapshot();
        assert_eq!(migrated.workspaces, vec!["C:\\repo"]);
        assert_eq!(migrated.recent_files, vec!["C:\\repo\nnotes/day.md"]);
        let rewritten: serde_json::Value =
            serde_json::from_slice(&fs::read(directory.path().join("state.json")).unwrap())
                .unwrap();
        assert_eq!(rewritten["schemaVersion"], 7);
        assert_eq!(
            rewritten["pendingWorkspaceOperations"],
            serde_json::json!({})
        );
    }

    #[test]
    fn newer_search_generation_cancels_the_previous_scan() {
        let runtime = WorkspaceRuntime::default();

        let first = runtime.search_session("workspace", "test");
        let second = runtime.search_session("workspace", "test");

        assert!(!first.is_current());
        assert!(second.is_current());
    }

    #[test]
    fn recent_file_limit_is_published_and_enforced_by_the_same_owner() {
        let directory = TempDir::new().unwrap();
        let state = PersistentState::load(directory.path()).unwrap();

        for index in 0..=RECENT_FILE_LIMIT {
            state
                .remember_file("C:\\repo", &format!("note-{index}.md"))
                .unwrap();
        }

        let startup = state.startup_state().unwrap();
        assert_eq!(startup.recent_file_limit, RECENT_FILE_LIMIT);
        assert_eq!(startup.recent_files.len(), RECENT_FILE_LIMIT);
        assert_eq!(
            startup.recent_files[0],
            format!("C:\\repo\nnote-{RECENT_FILE_LIMIT}.md")
        );
        assert!(!startup
            .recent_files
            .contains(&"C:\\repo\nnote-0.md".to_owned()));
    }

    #[test]
    fn read_only_state_reloads_changes_made_after_the_cli_view_was_created() {
        let directory = TempDir::new().unwrap();
        let writer = PersistentState::load(directory.path()).unwrap();
        let reader = PersistentState::load_read_only(directory.path()).unwrap();

        writer.register_workspace("C:\\fresh").unwrap();

        assert_eq!(
            reader.startup_state().unwrap().workspaces,
            vec!["C:\\fresh"]
        );
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
        state.seed_workspace_changes(main, &[WorkspaceChangeIntent::upsert("main.md", "main")]);
        state.seed_workspace_changes(
            worktree,
            &[WorkspaceChangeIntent::upsert("draft.md", "draft")],
        );
        state.remember_file(main, "main.md").unwrap();
        state.remember_file(worktree, "draft.md").unwrap();
        state
            .set_credential_ref(credential_key, "credential")
            .unwrap();
        let main_search = runtime.search_session(main, "test");
        let worktree_search = runtime.search_session(worktree, "test");

        state
            .forget_workspace(main, &roots, credential_key)
            .unwrap();
        runtime.forget_roots(&roots, credential_key);
        let snapshot = state.snapshot();

        assert!(snapshot.workspaces.is_empty());
        assert!(snapshot.workspace_changes.is_empty());
        assert!(snapshot.recent_files.is_empty());
        assert!(snapshot.credential_refs.is_empty());
        assert!(!main_search.is_current());
        assert!(!worktree_search.is_current());
        assert!(runtime.search_session(main, "test").is_current());
        assert!(runtime.search_session(worktree, "test").is_current());
    }
}
