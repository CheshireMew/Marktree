#[cfg(test)]
mod tests {
    use std::{
        fs,
        path::{Path, PathBuf},
    };

    use super::*;
    use base64::{engine::general_purpose::STANDARD, Engine};
    use chrono::Utc;
    use git2::{
        Oid, Repository, RepositoryInitOptions, Signature, StashApplyOptions, StashFlags,
    };
    use tempfile::TempDir;

    use crate::{
        file_version::hash_bytes,
        state::PersistentState,
        types::{
            ConflictChoice, ConflictKind, CreateWorktreeRequest, DiffMode, GitOperationKind,
            GitOperationPhase, PendingGitOperation, SyncStage,
        },
    };

    use super::{
        conflicts::recovery_metadata,
        remote::push_current_branch,
        stash::{
            find_operation_stash, operation_stash_index, stash_if_needed,
        },
    };

    fn test_signature() -> Signature<'static> {
        Signature::now("Marktree Test", "test@marktree.local").unwrap()
    }

    fn commit_file(repo: &Repository, path: &str, content: &str, message: &str) -> Oid {
        commit_bytes(repo, path, content.as_bytes(), message)
    }

    fn commit_bytes(repo: &Repository, path: &str, content: &[u8], message: &str) -> Oid {
        let full_path = repo.workdir().unwrap().join(path);
        if let Some(parent) = full_path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(full_path, content).unwrap();
        let mut index = repo.index().unwrap();
        index.add_path(Path::new(path)).unwrap();
        index.write().unwrap();
        let tree_id = index.write_tree().unwrap();
        let tree = repo.find_tree(tree_id).unwrap();
        let signature = test_signature();
        let parents = repo
            .head()
            .ok()
            .and_then(|head| head.peel_to_commit().ok())
            .into_iter()
            .collect::<Vec<_>>();
        let parent_refs = parents.iter().collect::<Vec<_>>();
        repo.commit(
            Some("HEAD"),
            &signature,
            &signature,
            message,
            &tree,
            &parent_refs,
        )
        .unwrap()
    }

    fn commit_deletion(repo: &Repository, path: &str, message: &str) -> Oid {
        fs::remove_file(repo.workdir().unwrap().join(path)).unwrap();
        let mut index = repo.index().unwrap();
        index.remove_path(Path::new(path)).unwrap();
        index.write().unwrap();
        let tree_id = index.write_tree().unwrap();
        let tree = repo.find_tree(tree_id).unwrap();
        let signature = test_signature();
        let parent = repo.head().unwrap().peel_to_commit().unwrap();
        repo.commit(
            Some("HEAD"),
            &signature,
            &signature,
            message,
            &tree,
            &[&parent],
        )
        .unwrap()
    }

    fn init_with_main(path: &Path) -> Repository {
        let mut options = RepositoryInitOptions::new();
        options.initial_head("main");
        Repository::init_opts(path, &options).unwrap()
    }

    fn seed_bare_remote(base: &Path, initial_content: &str) -> (PathBuf, PathBuf) {
        let remote_path = base.join("remote.git");
        Repository::init_bare(&remote_path).unwrap();
        let seed_path = base.join("seed");
        let seed = init_with_main(&seed_path);
        commit_file(&seed, "notes/shared.md", initial_content, "initial");
        seed.remote("origin", remote_path.to_str().unwrap())
            .unwrap();
        let mut remote = seed.find_remote("origin").unwrap();
        remote
            .push(&["refs/heads/main:refs/heads/main"], None)
            .unwrap();
        let bare = Repository::open_bare(&remote_path).unwrap();
        bare.set_head("refs/heads/main").unwrap();
        (remote_path, seed_path)
    }

    #[test]
    fn status_and_structured_diff_come_from_real_repository() {
        let directory = TempDir::new().unwrap();
        let repo = Repository::init(directory.path()).unwrap();
        commit_file(&repo, "notes/hello.md", "# Hello\n", "initial");
        fs::write(
            directory.path().join("notes/hello.md"),
            "# Hello\n\nChanged.\n",
        )
        .unwrap();

        let status = status_snapshot(&repo).unwrap();
        assert_eq!(status.changed_count, 1);
        assert_eq!(status.files[0].path, "notes/hello.md");
        let result = diff(directory.path().to_str().unwrap(), DiffMode::WorktreeToHead).unwrap();
        assert_eq!(result.files.len(), 1);
        assert!(result.insertions > 0);
    }

    #[test]
    fn rejects_non_https_remote_urls() {
        assert!(validate_remote_url("https://example.com/notes.git").is_ok());
        assert!(validate_remote_url("http://example.com/notes.git").is_err());
        assert!(validate_remote_url("git@example.com:notes.git").is_err());
        assert!(validate_remote_url("ssh://git@example.com/notes.git").is_err());
    }

    #[test]
    fn marktree_sync_stages_only_recorded_document_paths() {
        let directory = TempDir::new().unwrap();
        let app_data = TempDir::new().unwrap();
        let state = PersistentState::load(app_data.path()).unwrap();
        let repo = Repository::init(directory.path()).unwrap();
        commit_file(&repo, "notes/hello.md", "# Hello\n", "initial");
        commit_file(&repo, "src/code.rs", "fn main() {}\n", "code");
        fs::write(directory.path().join("notes/hello.md"), "# Updated\n").unwrap();
        fs::write(
            directory.path().join("src/code.rs"),
            "fn main() { println!(\"keep me local\"); }\n",
        )
        .unwrap();
        let root = directory.path().to_string_lossy().into_owned();
        state
            .record_workspace_change(
                &root,
                "notes/hello.md",
                crate::types::WorkspaceChangeOperation::Upsert,
                Some(&hash_bytes(b"# Updated\n")),
            )
            .unwrap();

        let plan = sync_plan(&root, &state).unwrap();
        assert_eq!(plan.changed_paths, vec!["notes/hello.md"]);
        stage_paths(&root, &plan.changed_paths).unwrap();
        let status = status_snapshot(&repo).unwrap();
        assert!(status
            .files
            .iter()
            .any(|entry| entry.path == "notes/hello.md" && entry.staged));
        assert!(status.files.iter().any(|entry| {
            entry.path == "src/code.rs" && !entry.staged && entry.worktree_status == "modified"
        }));
    }

    #[test]
    fn marktree_sync_does_not_commit_or_unstage_preexisting_staged_source_changes() {
        let sandbox = TempDir::new().unwrap();
        let app_data = TempDir::new().unwrap();
        let state = PersistentState::load(app_data.path()).unwrap();
        let (remote_path, seed_path) = seed_bare_remote(sandbox.path(), "# Shared\n");
        let seed = Repository::open(seed_path).unwrap();
        commit_file(&seed, "src/code.rs", "fn original() {}\n", "source");
        seed.find_remote("origin")
            .unwrap()
            .push(&["refs/heads/main:refs/heads/main"], None)
            .unwrap();

        let local = crate::workspace::clone_workspace(
            remote_path.to_str().unwrap(),
            sandbox.path().join("local").to_str().unwrap(),
            None,
            &state,
        )
        .unwrap();
        let opened = crate::documents::read_document(&local.root, "notes/shared.md").unwrap();
        crate::documents::save_document(
            crate::types::SaveDocumentRequest {
                root: local.root.clone(),
                path: "notes/shared.md".to_owned(),
                content: "# Updated\n".to_owned(),
                expected_sha256: Some(opened.sha256),
                expected_missing: false,
                encoding: opened.encoding,
            },
            &state,
        )
        .unwrap();
        fs::write(
            Path::new(&local.root).join("src/code.rs"),
            "fn staged_but_not_synced() {}\n",
        )
        .unwrap();
        stage_paths(&local.root, &["src/code.rs".to_owned()]).unwrap();

        let result = sync_workspace_changes(&local.root, None, &state).unwrap();
        assert!(result.pushed);
        let local_status = repository_status(&local.root).unwrap();
        assert!(local_status
            .files
            .iter()
            .any(|entry| entry.path == "src/code.rs" && entry.staged));

        let verification_state_dir = TempDir::new().unwrap();
        let verification_state = PersistentState::load(verification_state_dir.path()).unwrap();
        let verification = crate::workspace::clone_workspace(
            remote_path.to_str().unwrap(),
            sandbox.path().join("verification").to_str().unwrap(),
            None,
            &verification_state,
        )
        .unwrap();
        assert_eq!(
            fs::read_to_string(Path::new(&verification.root).join("src/code.rs"))
                .unwrap()
                .replace("\r\n", "\n"),
            "fn original() {}\n"
        );
        assert_eq!(
            crate::documents::read_document(&verification.root, "notes/shared.md")
                .unwrap()
                .content
                .replace("\r\n", "\n"),
            "# Updated\n"
        );
    }

    #[test]
    fn isolated_marktree_commit_never_rewrites_an_unmanaged_index_entry() {
        let directory = TempDir::new().unwrap();
        let repo = init_with_main(directory.path());
        commit_file(&repo, "notes/shared.md", "# Shared\n", "document");
        commit_file(&repo, "src/code.rs", "fn original() {}\n", "source");
        fs::write(directory.path().join("notes/shared.md"), "# Updated\n").unwrap();
        fs::write(
            directory.path().join("src/code.rs"),
            "fn staged_but_not_synced() {}\n",
        )
        .unwrap();
        stage_paths(
            directory.path().to_str().unwrap(),
            &["src/code.rs".to_owned()],
        )
        .unwrap();
        drop(repo);
        let repo = Repository::open(directory.path()).unwrap();
        let before = repo
            .index()
            .unwrap()
            .get_path(Path::new("src/code.rs"), 0)
            .unwrap()
            .id;
        let source_at_head = repo
            .head()
            .unwrap()
            .peel_to_tree()
            .unwrap()
            .get_path(Path::new("src/code.rs"))
            .unwrap()
            .id();
        assert_ne!(source_at_head, before);

        commit_only_paths(
            &repo,
            &["notes/shared.md".to_owned()],
            "Marktree sync [marktree-operation:index-isolation]",
        )
        .unwrap();

        let after = repo
            .index()
            .unwrap()
            .get_path(Path::new("src/code.rs"), 0)
            .unwrap()
            .id;
        let committed_source = repo
            .head()
            .unwrap()
            .peel_to_tree()
            .unwrap()
            .get_path(Path::new("src/code.rs"))
            .unwrap()
            .id();
        assert_eq!(after, before);
        assert_eq!(committed_source, source_at_head);
        assert!(repository_status(directory.path().to_str().unwrap())
            .unwrap()
            .files
            .iter()
            .any(|entry| entry.path == "src/code.rs" && entry.staged));
    }

    #[test]
    fn commit_phase_resume_recognizes_the_operation_commit_after_a_crash() {
        let directory = TempDir::new().unwrap();
        let state_dir = TempDir::new().unwrap();
        let state = PersistentState::load(state_dir.path()).unwrap();
        let repo = init_with_main(directory.path());
        commit_file(&repo, "notes/shared.md", "# Shared\n", "initial");
        fs::write(directory.path().join("notes/shared.md"), "# Updated\n").unwrap();
        let root = directory.path().to_string_lossy().into_owned();
        let managed = state
            .record_workspace_change(
                &root,
                "notes/shared.md",
                crate::types::WorkspaceChangeOperation::Upsert,
                Some(&hash_bytes(b"# Updated\n")),
            )
            .unwrap();
        let operation_id = "commit-crash-resume";
        let operation = PendingGitOperation {
            id: operation_id.to_owned(),
            root: root.clone(),
            kind: GitOperationKind::Sync,
            phase: GitOperationPhase::Commit,
            started_at: Utc::now().to_rfc3339(),
            workspace_changes: vec![managed],
            changed_paths: vec!["notes/shared.md".to_owned()],
            committed: false,
            commit_id: None,
            pulled: false,
            pushed: false,
            original_head_oid: None,
            stash_oid: None,
            aborting: false,
            stash_apply_started: false,
            stash_applied: false,
        };
        state.begin_git_operation(operation).unwrap();
        let committed = commit_only_paths(
            &repo,
            &["notes/shared.md".to_owned()],
            &format!("Marktree sync [marktree-operation:{operation_id}]"),
        )
        .unwrap();

        let result = resume_git_operation(&root, None, &state).unwrap();

        assert_eq!(result.failure_stage, Some(SyncStage::Fetch));
        let resumed = state.pending_git_operation(&root).unwrap();
        assert!(resumed.committed);
        assert_eq!(resumed.commit_id, Some(committed.to_string()));
        assert_eq!(
            find_operation_commit(&repo, operation_id).unwrap(),
            Some(committed)
        );
        assert_eq!(repo.head().unwrap().target(), Some(committed));
    }

    #[test]
    fn clean_pull_reads_a_remote_document_from_the_real_producer_chain() {
        let sandbox = TempDir::new().unwrap();
        let (remote_path, _) = seed_bare_remote(sandbox.path(), "# Shared\n");
        let local_state_dir = TempDir::new().unwrap();
        let local_state = PersistentState::load(local_state_dir.path()).unwrap();
        let other_state_dir = TempDir::new().unwrap();
        let other_state = PersistentState::load(other_state_dir.path()).unwrap();
        let local = crate::workspace::clone_workspace(
            remote_path.to_str().unwrap(),
            sandbox.path().join("local").to_str().unwrap(),
            None,
            &local_state,
        )
        .unwrap();
        let other = crate::workspace::clone_workspace(
            remote_path.to_str().unwrap(),
            sandbox.path().join("other").to_str().unwrap(),
            None,
            &other_state,
        )
        .unwrap();

        let opened =
            crate::documents::open_document(&other.root, "notes/shared.md", &other_state).unwrap();
        crate::documents::save_document(
            crate::types::SaveDocumentRequest {
                root: other.root.clone(),
                path: opened.path,
                content: "# Shared\n\nFrom the other device\n".to_owned(),
                expected_sha256: Some(opened.sha256),
                expected_missing: false,
                encoding: opened.encoding,
            },
            &other_state,
        )
        .unwrap();
        assert!(
            sync_workspace_changes(&other.root, None, &other_state)
                .unwrap()
                .pushed
        );

        let fetched = fetch(&local.root, None).unwrap();
        assert_eq!(fetched.behind, 1);
        let pulled = pull_rebase(&local.root, None, &local_state).unwrap();
        assert!(pulled.pulled);
        assert_eq!(
            crate::documents::open_document(&local.root, "notes/shared.md", &local_state,)
                .unwrap()
                .content
                .replace("\r\n", "\n"),
            "# Shared\n\nFrom the other device\n"
        );
    }

    #[test]
    fn successful_retry_clears_a_manifest_for_content_already_committed() {
        let sandbox = TempDir::new().unwrap();
        let (remote_path, _) = seed_bare_remote(sandbox.path(), "# Shared\n");
        let app_data = TempDir::new().unwrap();
        let state = PersistentState::load(app_data.path()).unwrap();
        let local = crate::workspace::clone_workspace(
            remote_path.to_str().unwrap(),
            sandbox.path().join("local").to_str().unwrap(),
            None,
            &state,
        )
        .unwrap();
        let current = fs::read(Path::new(&local.root).join("notes/shared.md")).unwrap();
        state
            .record_workspace_change(
                &local.root,
                "notes/shared.md",
                crate::types::WorkspaceChangeOperation::Upsert,
                Some(&hash_bytes(&current)),
            )
            .unwrap();

        let result = sync_workspace_changes(&local.root, None, &state).unwrap();
        assert!(result.pushed);
        assert!(result.changed_paths.is_empty());
        assert!(state.workspace_changes(&local.root).is_empty());
    }

    #[test]
    fn sync_refuses_content_changed_after_the_document_producer_saved_it() {
        let sandbox = TempDir::new().unwrap();
        let (remote_path, _) = seed_bare_remote(sandbox.path(), "# Shared\n");
        let state_dir = TempDir::new().unwrap();
        let state = PersistentState::load(state_dir.path()).unwrap();
        let local = crate::workspace::clone_workspace(
            remote_path.to_str().unwrap(),
            sandbox.path().join("local").to_str().unwrap(),
            None,
            &state,
        )
        .unwrap();
        let opened = crate::documents::read_document(&local.root, "notes/shared.md").unwrap();
        crate::documents::save_document(
            crate::types::SaveDocumentRequest {
                root: local.root.clone(),
                path: opened.path,
                content: "# Saved by Marktree\n".to_owned(),
                expected_sha256: Some(opened.sha256),
                expected_missing: false,
                encoding: opened.encoding,
            },
            &state,
        )
        .unwrap();
        fs::write(
            Path::new(&local.root).join("notes/shared.md"),
            "# Replaced externally\n",
        )
        .unwrap();

        let result = sync_workspace_changes(&local.root, None, &state).unwrap();

        assert_eq!(result.failure_stage, Some(SyncStage::Prepare));
        assert_eq!(
            result.error.as_ref().map(|error| &error.code),
            Some(&crate::error::ErrorCode::ManagedContentChanged)
        );
        assert!(!result.committed);
        assert!(!result.pushed);
        assert!(state.pending_git_operation(&local.root).is_none());
        assert_eq!(state.workspace_changes(&local.root).len(), 1);
        assert_eq!(
            Repository::open(&local.root)
                .unwrap()
                .head()
                .unwrap()
                .peel_to_commit()
                .unwrap()
                .message()
                .unwrap(),
            "initial"
        );
    }

    #[test]
    fn first_sync_pushes_an_unborn_main_branch_and_configures_its_upstream() {
        let sandbox = TempDir::new().unwrap();
        let remote_path = sandbox.path().join("empty.git");
        let remote = Repository::init_bare(&remote_path).unwrap();
        remote.set_head("refs/heads/main").unwrap();
        let state_dir = TempDir::new().unwrap();
        let state = PersistentState::load(state_dir.path()).unwrap();
        let local_path = sandbox.path().join("local");
        crate::workspace::create_workspace(local_path.to_str().unwrap(), &state).unwrap();
        let descriptor = crate::workspace::enable_git(local_path.to_str().unwrap()).unwrap();
        let repo = Repository::open(&descriptor.root).unwrap();
        repo.remote("origin", remote_path.to_str().unwrap())
            .unwrap();
        crate::documents::create_document(&descriptor.root, "README.md", &state).unwrap();
        let opened = crate::documents::read_document(&descriptor.root, "README.md").unwrap();
        crate::documents::save_document(
            crate::types::SaveDocumentRequest {
                root: descriptor.root.clone(),
                path: opened.path,
                content: "# First sync\n".to_owned(),
                expected_sha256: Some(opened.sha256),
                expected_missing: false,
                encoding: opened.encoding,
            },
            &state,
        )
        .unwrap();

        let plan = sync_plan(&descriptor.root, &state).unwrap();
        assert_eq!(plan.branch.as_deref(), Some("main"));
        assert!(plan.can_push);
        let result = sync_workspace_changes(&descriptor.root, None, &state).unwrap();
        assert!(result.committed);
        assert!(result.pushed);
        assert_eq!(
            repository_status(&descriptor.root)
                .unwrap()
                .upstream
                .as_deref(),
            Some("origin/main")
        );

        let verification_state_dir = TempDir::new().unwrap();
        let verification_state = PersistentState::load(verification_state_dir.path()).unwrap();
        let verification = crate::workspace::clone_workspace(
            remote_path.to_str().unwrap(),
            sandbox
                .path()
                .join("first-sync-verification")
                .to_str()
                .unwrap(),
            None,
            &verification_state,
        )
        .unwrap();
        assert_eq!(
            crate::documents::read_document(&verification.root, "README.md")
                .unwrap()
                .content
                .replace("\r\n", "\n"),
            "# First sync\n"
        );
    }

    #[test]
    fn document_asset_sync_reaches_remote_and_another_real_clone() {
        let sandbox = TempDir::new().unwrap();
        let app_data = TempDir::new().unwrap();
        let state = PersistentState::load(app_data.path()).unwrap();
        let (remote_path, _) = seed_bare_remote(sandbox.path(), "# Shared\n");
        let first_path = sandbox.path().join("first");
        let first = crate::workspace::clone_workspace(
            remote_path.to_str().unwrap(),
            first_path.to_str().unwrap(),
            None,
            &state,
        )
        .unwrap();

        let opened = crate::documents::read_document(&first.root, "notes/shared.md").unwrap();
        let image_bytes = b"real-image-bytes";
        let asset = crate::documents::write_asset(
            &first.root,
            "notes/shared.md",
            "diagram.png",
            &STANDARD.encode(image_bytes),
            None,
            &state,
        )
        .unwrap();
        crate::documents::save_document(
            crate::types::SaveDocumentRequest {
                root: first.root.clone(),
                path: "notes/shared.md".to_owned(),
                content: format!("# Shared\n\n![diagram]({})\n", asset.markdown_path),
                expected_sha256: Some(opened.sha256),
                expected_missing: false,
                encoding: opened.encoding,
            },
            &state,
        )
        .unwrap();

        let result = sync_workspace_changes(&first.root, None, &state).unwrap();
        assert!(result.committed);
        assert!(result.pushed);
        assert!(result.conflicts.is_empty());

        let second_state_dir = TempDir::new().unwrap();
        let second_state = PersistentState::load(second_state_dir.path()).unwrap();
        let second_path = sandbox.path().join("second");
        let second = crate::workspace::clone_workspace(
            remote_path.to_str().unwrap(),
            second_path.to_str().unwrap(),
            None,
            &second_state,
        )
        .unwrap();
        let received = crate::documents::read_document(&second.root, "notes/shared.md").unwrap();
        assert!(received.content.contains(&asset.markdown_path));
        assert_eq!(
            fs::read(Path::new(&second.root).join(asset.path)).unwrap(),
            image_bytes
        );
    }

    #[test]
    fn workspace_move_reaches_remote_as_exact_delete_and_upsert() {
        let sandbox = TempDir::new().unwrap();
        let state_dir = TempDir::new().unwrap();
        let state = PersistentState::load(state_dir.path()).unwrap();
        let (remote_path, _) = seed_bare_remote(sandbox.path(), "# Shared\n");
        let first = crate::workspace::clone_workspace(
            remote_path.to_str().unwrap(),
            sandbox.path().join("first-move").to_str().unwrap(),
            None,
            &state,
        )
        .unwrap();

        let moved = crate::documents::move_entry(
            &first.root,
            "notes/shared.md",
            "archive/shared.md",
            &state,
        )
        .unwrap();
        assert_eq!(moved.moved_files.len(), 1);
        assert_eq!(moved.moved_files[0].old_path, "notes/shared.md");
        assert_eq!(moved.moved_files[0].new_path, "archive/shared.md");
        let changes = state.workspace_changes(&first.root);
        assert!(changes.iter().any(|change| {
            change.path == "notes/shared.md"
                && change.operation == crate::types::WorkspaceChangeOperation::Delete
        }));
        assert!(changes.iter().any(|change| {
            change.path == "archive/shared.md"
                && change.operation == crate::types::WorkspaceChangeOperation::Upsert
        }));

        let result = sync_workspace_changes(&first.root, None, &state).unwrap();
        assert!(result.committed);
        assert!(result.pushed);
        assert!(state.workspace_changes(&first.root).is_empty());
        let local_repository = Repository::open(&first.root).unwrap();
        let local_tree = local_repository
            .head()
            .unwrap()
            .peel_to_commit()
            .unwrap()
            .tree()
            .unwrap();
        assert!(local_tree.get_path(Path::new("notes/shared.md")).is_err());
        assert!(local_tree.get_path(Path::new("archive/shared.md")).is_ok());

        let verification_state_dir = TempDir::new().unwrap();
        let verification_state = PersistentState::load(verification_state_dir.path()).unwrap();
        let second = crate::workspace::clone_workspace(
            remote_path.to_str().unwrap(),
            sandbox.path().join("second-move").to_str().unwrap(),
            None,
            &verification_state,
        )
        .unwrap();
        assert!(!Path::new(&second.root).join("notes/shared.md").exists());
        assert_eq!(
            crate::documents::read_document(&second.root, "archive/shared.md")
                .unwrap()
                .content
                .replace("\r\n", "\n"),
            "# Shared\n"
        );
    }

    #[test]
    fn worktree_is_created_by_libgit2_and_reads_the_same_document() {
        let sandbox = TempDir::new().unwrap();
        let repository_path = sandbox.path().join("repository");
        let repo = init_with_main(&repository_path);
        commit_file(&repo, "notes/shared.md", "# Main\n", "initial");
        let worktree_path = sandbox.path().join("repository-draft");
        let descriptor = create_worktree(CreateWorktreeRequest {
            root: repository_path.to_string_lossy().into_owned(),
            name: "draft".to_owned(),
            path: worktree_path.to_string_lossy().into_owned(),
            branch: "draft".to_owned(),
            start_point: Some("HEAD".to_owned()),
        })
        .unwrap();
        assert_eq!(descriptor.branch.as_deref(), Some("draft"));
        assert_eq!(
            crate::documents::read_document(worktree_path.to_str().unwrap(), "notes/shared.md")
                .unwrap()
                .content
                .replace("\r\n", "\n"),
            "# Main\n"
        );
    }

    #[test]
    fn worktree_comparison_reads_both_real_files() {
        let sandbox = TempDir::new().unwrap();
        let repository_path = sandbox.path().join("repository");
        let repo = init_with_main(&repository_path);
        commit_file(&repo, "notes/shared.md", "# Main\n", "initial");
        let worktree_path = sandbox.path().join("repository-draft");
        create_worktree(CreateWorktreeRequest {
            root: repository_path.to_string_lossy().into_owned(),
            name: "draft".to_owned(),
            path: worktree_path.to_string_lossy().into_owned(),
            branch: "draft".to_owned(),
            start_point: Some("HEAD".to_owned()),
        })
        .unwrap();
        fs::write(worktree_path.join("notes/shared.md"), "# Draft\n").unwrap();

        let comparison = compare_worktrees(
            repository_path.to_str().unwrap(),
            worktree_path.to_str().unwrap(),
            "notes/shared.md",
        )
        .unwrap();
        assert_eq!(comparison.left.replace("\r\n", "\n"), "# Main\n");
        assert_eq!(comparison.right.replace("\r\n", "\n"), "# Draft\n");
    }

    #[test]
    fn branch_management_uses_real_references_and_refuses_dirty_checkout() {
        let sandbox = TempDir::new().unwrap();
        let repository_path = sandbox.path().join("repository");
        let repo = init_with_main(&repository_path);
        commit_file(&repo, "notes/shared.md", "# Main\n", "initial");
        let root = repository_path.to_string_lossy().into_owned();

        let initial = list_branches(&root).unwrap();
        assert_eq!(initial.len(), 1);
        assert!(initial[0].is_current);
        assert!(initial[0].checked_out_path.is_some());

        let feature = create_branch(&root, "feature/writing", Some("HEAD"), true).unwrap();
        assert_eq!(feature.branch.as_deref(), Some("feature/writing"));
        fs::write(repository_path.join("notes/shared.md"), "# Dirty\n").unwrap();
        let error = checkout_branch(&root, "main").unwrap_err().to_string();
        assert!(error.contains("working tree changes"));

        fs::write(repository_path.join("notes/shared.md"), "# Main\n").unwrap();
        let main = checkout_branch(&root, "main").unwrap();
        assert_eq!(main.branch.as_deref(), Some("main"));
        let remaining = delete_branch(&root, "feature/writing").unwrap();
        assert_eq!(remaining.len(), 1);
        assert_eq!(remaining[0].name, "main");
    }

    #[test]
    fn conflicting_document_keeps_both_recovery_versions_then_syncs_choice() {
        let sandbox = TempDir::new().unwrap();
        let (remote_path, _) = seed_bare_remote(sandbox.path(), "# Shared\n\nBase\n");
        let local_state_dir = TempDir::new().unwrap();
        let local_state = PersistentState::load(local_state_dir.path()).unwrap();
        let remote_state_dir = TempDir::new().unwrap();
        let remote_state = PersistentState::load(remote_state_dir.path()).unwrap();
        let local = crate::workspace::clone_workspace(
            remote_path.to_str().unwrap(),
            sandbox.path().join("local").to_str().unwrap(),
            None,
            &local_state,
        )
        .unwrap();
        let other = crate::workspace::clone_workspace(
            remote_path.to_str().unwrap(),
            sandbox.path().join("other").to_str().unwrap(),
            None,
            &remote_state,
        )
        .unwrap();

        let local_document =
            crate::documents::read_document(&local.root, "notes/shared.md").unwrap();
        crate::documents::save_document(
            crate::types::SaveDocumentRequest {
                root: local.root.clone(),
                path: "notes/shared.md".to_owned(),
                content: "# Shared\n\nLocal version\n".to_owned(),
                expected_sha256: Some(local_document.sha256),
                expected_missing: false,
                encoding: local_document.encoding,
            },
            &local_state,
        )
        .unwrap();
        let other_document =
            crate::documents::read_document(&other.root, "notes/shared.md").unwrap();
        crate::documents::save_document(
            crate::types::SaveDocumentRequest {
                root: other.root.clone(),
                path: "notes/shared.md".to_owned(),
                content: "# Shared\n\nRemote version\n".to_owned(),
                expected_sha256: Some(other_document.sha256),
                expected_missing: false,
                encoding: other_document.encoding,
            },
            &remote_state,
        )
        .unwrap();
        let remote_result = sync_workspace_changes(&other.root, None, &remote_state).unwrap();
        assert!(remote_result.pushed);

        let local_result = sync_workspace_changes(&local.root, None, &local_state).unwrap();
        assert_eq!(local_result.conflicts.len(), 1);
        let conflict = &local_result.conflicts[0];
        assert!(conflict
            .local
            .as_deref()
            .is_some_and(|content| content.contains("Local version")));
        assert!(conflict
            .remote
            .as_deref()
            .is_some_and(|content| content.contains("Remote version")));
        let recovery_dir = local_state
            .recovery_dir()
            .unwrap()
            .join(&conflict.recovery_id);
        assert!(recovery_dir.join("local.bin").is_file());
        assert!(recovery_dir.join("remote.bin").is_file());
        drop(local_state);
        let local_state = PersistentState::load(local_state_dir.path()).unwrap();
        let pending = pending_conflicts(&local.root, &local_state).unwrap();
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].recovery_id, conflict.recovery_id);

        resolve_conflict(
            &local.root,
            &conflict.path,
            &conflict.recovery_id,
            ConflictChoice::Local,
            &local_state,
        )
        .unwrap();
        let metadata = recovery_metadata(&recovery_dir).unwrap();
        assert!(matches!(metadata.choice, Some(ConflictChoice::Local)));
        let continued = resume_git_operation(&local.root, None, &local_state).unwrap();
        assert!(continued.pushed);
        assert!(continued.conflicts.is_empty());

        let verification_state_dir = TempDir::new().unwrap();
        let verification_state = PersistentState::load(verification_state_dir.path()).unwrap();
        let verification = crate::workspace::clone_workspace(
            remote_path.to_str().unwrap(),
            sandbox.path().join("verification").to_str().unwrap(),
            None,
            &verification_state,
        )
        .unwrap();
        assert!(
            crate::documents::read_document(&verification.root, "notes/shared.md")
                .unwrap()
                .content
                .contains("Local version")
        );
    }

    #[test]
    fn aborting_a_conflicted_sync_restores_local_work_and_keeps_remote_unchanged() {
        let sandbox = TempDir::new().unwrap();
        let (remote_path, _) = seed_bare_remote(sandbox.path(), "# Shared\n\nBase\n");
        let local_state_dir = TempDir::new().unwrap();
        let local_state = PersistentState::load(local_state_dir.path()).unwrap();
        let other_state_dir = TempDir::new().unwrap();
        let other_state = PersistentState::load(other_state_dir.path()).unwrap();
        let local = crate::workspace::clone_workspace(
            remote_path.to_str().unwrap(),
            sandbox.path().join("local").to_str().unwrap(),
            None,
            &local_state,
        )
        .unwrap();
        let other = crate::workspace::clone_workspace(
            remote_path.to_str().unwrap(),
            sandbox.path().join("other").to_str().unwrap(),
            None,
            &other_state,
        )
        .unwrap();

        let local_document =
            crate::documents::read_document(&local.root, "notes/shared.md").unwrap();
        crate::documents::save_document(
            crate::types::SaveDocumentRequest {
                root: local.root.clone(),
                path: "notes/shared.md".to_owned(),
                content: "# Shared\n\nLocal version\n".to_owned(),
                expected_sha256: Some(local_document.sha256),
                expected_missing: false,
                encoding: local_document.encoding,
            },
            &local_state,
        )
        .unwrap();
        fs::write(
            Path::new(&local.root).join("scratch.txt"),
            "unmanaged local work",
        )
        .unwrap();
        let remote_document =
            crate::documents::read_document(&other.root, "notes/shared.md").unwrap();
        crate::documents::save_document(
            crate::types::SaveDocumentRequest {
                root: other.root.clone(),
                path: "notes/shared.md".to_owned(),
                content: "# Shared\n\nRemote version\n".to_owned(),
                expected_sha256: Some(remote_document.sha256),
                expected_missing: false,
                encoding: remote_document.encoding,
            },
            &other_state,
        )
        .unwrap();
        assert!(
            sync_workspace_changes(&other.root, None, &other_state)
                .unwrap()
                .pushed
        );

        let result = sync_workspace_changes(&local.root, None, &local_state).unwrap();
        let conflict = result.conflicts.first().unwrap();
        let operation = local_state.pending_git_operation(&local.root).unwrap();
        assert!(operation.stash_oid.is_some());
        assert!(local_state
            .recovery_dir()
            .unwrap()
            .join(&conflict.recovery_id)
            .is_dir());

        let status = abort_git_operation(&local.root, &local_state).unwrap();

        assert_eq!(status.conflicted_count, 0);
        assert!(local_state.pending_git_operation(&local.root).is_none());
        assert_eq!(local_state.workspace_changes(&local.root).len(), 1);
        assert_eq!(
            crate::documents::read_document(&local.root, "notes/shared.md")
                .unwrap()
                .content
                .replace("\r\n", "\n"),
            "# Shared\n\nLocal version\n"
        );
        assert_eq!(
            fs::read_to_string(Path::new(&local.root).join("scratch.txt")).unwrap(),
            "unmanaged local work"
        );
        assert!(local_state
            .recovery_dir()
            .unwrap()
            .join("archive")
            .join(&operation.id)
            .join(&conflict.recovery_id)
            .is_dir());

        let verification_state_dir = TempDir::new().unwrap();
        let verification_state = PersistentState::load(verification_state_dir.path()).unwrap();
        let verification = crate::workspace::clone_workspace(
            remote_path.to_str().unwrap(),
            sandbox.path().join("abort-verification").to_str().unwrap(),
            None,
            &verification_state,
        )
        .unwrap();
        assert_eq!(
            crate::documents::read_document(&verification.root, "notes/shared.md")
                .unwrap()
                .content
                .replace("\r\n", "\n"),
            "# Shared\n\nRemote version\n"
        );
        assert!(!Path::new(&verification.root).join("scratch.txt").exists());
    }

    #[test]
    fn binary_conflict_preserves_raw_sides_and_pushes_the_selected_bytes() {
        let sandbox = TempDir::new().unwrap();
        let (remote_path, seed_path) = seed_bare_remote(sandbox.path(), "# Shared\n");
        let seed = Repository::open(seed_path).unwrap();
        commit_bytes(&seed, "assets/shared.png", b"\0initial", "binary asset");
        seed.find_remote("origin")
            .unwrap()
            .push(&["refs/heads/main:refs/heads/main"], None)
            .unwrap();
        let local_state_dir = TempDir::new().unwrap();
        let local_state = PersistentState::load(local_state_dir.path()).unwrap();
        let other_state_dir = TempDir::new().unwrap();
        let other_state = PersistentState::load(other_state_dir.path()).unwrap();
        let local = crate::workspace::clone_workspace(
            remote_path.to_str().unwrap(),
            sandbox.path().join("local").to_str().unwrap(),
            None,
            &local_state,
        )
        .unwrap();
        let other = crate::workspace::clone_workspace(
            remote_path.to_str().unwrap(),
            sandbox.path().join("other").to_str().unwrap(),
            None,
            &other_state,
        )
        .unwrap();
        let local_bytes = b"\0local\xff";
        fs::write(
            Path::new(&local.root).join("assets/shared.png"),
            local_bytes,
        )
        .unwrap();
        local_state
            .record_workspace_change(
                &local.root,
                "assets/shared.png",
                crate::types::WorkspaceChangeOperation::Upsert,
                Some(&hash_bytes(local_bytes)),
            )
            .unwrap();
        let remote_bytes = b"\0remote\xfe";
        fs::write(
            Path::new(&other.root).join("assets/shared.png"),
            remote_bytes,
        )
        .unwrap();
        other_state
            .record_workspace_change(
                &other.root,
                "assets/shared.png",
                crate::types::WorkspaceChangeOperation::Upsert,
                Some(&hash_bytes(remote_bytes)),
            )
            .unwrap();
        assert!(
            sync_workspace_changes(&other.root, None, &other_state)
                .unwrap()
                .pushed
        );

        let result = sync_workspace_changes(&local.root, None, &local_state).unwrap();
        let conflict = result.conflicts.first().unwrap();
        assert_eq!(conflict.kind, ConflictKind::Binary);
        assert!(conflict.local.is_none());
        assert!(conflict.remote.is_none());
        let recovery = local_state
            .recovery_dir()
            .unwrap()
            .join(&conflict.recovery_id);
        assert_eq!(fs::read(recovery.join("local.bin")).unwrap(), local_bytes);
        assert_eq!(fs::read(recovery.join("remote.bin")).unwrap(), remote_bytes);

        resolve_conflict(
            &local.root,
            &conflict.path,
            &conflict.recovery_id,
            ConflictChoice::Local,
            &local_state,
        )
        .unwrap();
        assert!(
            resume_git_operation(&local.root, None, &local_state)
                .unwrap()
                .pushed
        );

        let verification_state_dir = TempDir::new().unwrap();
        let verification_state = PersistentState::load(verification_state_dir.path()).unwrap();
        let verification = crate::workspace::clone_workspace(
            remote_path.to_str().unwrap(),
            sandbox.path().join("binary-verification").to_str().unwrap(),
            None,
            &verification_state,
        )
        .unwrap();
        assert_eq!(
            fs::read(Path::new(&verification.root).join("assets/shared.png")).unwrap(),
            local_bytes
        );
    }

    #[test]
    fn delete_modify_conflict_removes_the_file_instead_of_writing_empty_content() {
        let sandbox = TempDir::new().unwrap();
        let (remote_path, _) = seed_bare_remote(sandbox.path(), "# Shared\n");
        let local_state_dir = TempDir::new().unwrap();
        let local_state = PersistentState::load(local_state_dir.path()).unwrap();
        let other_state_dir = TempDir::new().unwrap();
        let other_state = PersistentState::load(other_state_dir.path()).unwrap();
        let local = crate::workspace::clone_workspace(
            remote_path.to_str().unwrap(),
            sandbox.path().join("local").to_str().unwrap(),
            None,
            &local_state,
        )
        .unwrap();
        let other = crate::workspace::clone_workspace(
            remote_path.to_str().unwrap(),
            sandbox.path().join("other").to_str().unwrap(),
            None,
            &other_state,
        )
        .unwrap();
        let opened = crate::documents::read_document(&local.root, "notes/shared.md").unwrap();
        crate::documents::save_document(
            crate::types::SaveDocumentRequest {
                root: local.root.clone(),
                path: opened.path,
                content: "# Shared\n\nKeep locally\n".to_owned(),
                expected_sha256: Some(opened.sha256),
                expected_missing: false,
                encoding: opened.encoding,
            },
            &local_state,
        )
        .unwrap();
        let other_repo = Repository::open(&other.root).unwrap();
        commit_deletion(&other_repo, "notes/shared.md", "delete shared note");
        push_current_branch(&other_repo, None).unwrap();

        let result = sync_workspace_changes(&local.root, None, &local_state).unwrap();
        let conflict = result.conflicts.first().unwrap();
        assert_eq!(conflict.kind, ConflictKind::DeleteModify);
        assert!(conflict.local_exists);
        assert!(!conflict.remote_exists);

        resolve_conflict(
            &local.root,
            &conflict.path,
            &conflict.recovery_id,
            ConflictChoice::Remote,
            &local_state,
        )
        .unwrap();
        assert!(
            resume_git_operation(&local.root, None, &local_state)
                .unwrap()
                .pushed
        );
        assert!(!Path::new(&local.root).join("notes/shared.md").exists());

        let verification_state_dir = TempDir::new().unwrap();
        let verification_state = PersistentState::load(verification_state_dir.path()).unwrap();
        let verification = crate::workspace::clone_workspace(
            remote_path.to_str().unwrap(),
            sandbox
                .path()
                .join("deletion-verification")
                .to_str()
                .unwrap(),
            None,
            &verification_state,
        )
        .unwrap();
        assert!(!Path::new(&verification.root)
            .join("notes/shared.md")
            .exists());
    }

    #[test]
    fn pull_conflict_resumes_as_pull_without_pushing_or_clearing_sync_state() {
        let sandbox = TempDir::new().unwrap();
        let (remote_path, _) = seed_bare_remote(sandbox.path(), "# Shared\n");
        let local_state_dir = TempDir::new().unwrap();
        let local_state = PersistentState::load(local_state_dir.path()).unwrap();
        let other_state_dir = TempDir::new().unwrap();
        let other_state = PersistentState::load(other_state_dir.path()).unwrap();
        let local = crate::workspace::clone_workspace(
            remote_path.to_str().unwrap(),
            sandbox.path().join("local").to_str().unwrap(),
            None,
            &local_state,
        )
        .unwrap();
        let other = crate::workspace::clone_workspace(
            remote_path.to_str().unwrap(),
            sandbox.path().join("other").to_str().unwrap(),
            None,
            &other_state,
        )
        .unwrap();
        commit_file(
            &Repository::open(&local.root).unwrap(),
            "notes/shared.md",
            "# Shared\n\nLocal commit\n",
            "local",
        );
        commit_file(
            &Repository::open(&other.root).unwrap(),
            "notes/shared.md",
            "# Shared\n\nRemote commit\n",
            "remote",
        );
        push_current_branch(&Repository::open(&other.root).unwrap(), None).unwrap();
        local_state
            .record_workspace_change(
                &local.root,
                "draft.md",
                crate::types::WorkspaceChangeOperation::Upsert,
                Some("newer-save"),
            )
            .unwrap();

        let result = pull_rebase(&local.root, None, &local_state).unwrap();
        let conflict = result.conflicts.first().unwrap();
        assert_eq!(
            local_state.pending_git_operation(&local.root).unwrap().kind,
            GitOperationKind::Pull
        );
        resolve_conflict(
            &local.root,
            &conflict.path,
            &conflict.recovery_id,
            ConflictChoice::Local,
            &local_state,
        )
        .unwrap();
        let resumed = resume_git_operation(&local.root, None, &local_state).unwrap();
        assert!(resumed.pulled);
        assert!(!resumed.pushed);
        assert_eq!(local_state.workspace_changes(&local.root).len(), 1);

        let verification_state_dir = TempDir::new().unwrap();
        let verification_state = PersistentState::load(verification_state_dir.path()).unwrap();
        let verification = crate::workspace::clone_workspace(
            remote_path.to_str().unwrap(),
            sandbox.path().join("pull-verification").to_str().unwrap(),
            None,
            &verification_state,
        )
        .unwrap();
        assert!(
            crate::documents::read_document(&verification.root, "notes/shared.md")
                .unwrap()
                .content
                .contains("Remote commit")
        );
    }

    #[test]
    fn sync_restores_only_its_exact_stash_and_leaves_an_older_stash_untouched() {
        let sandbox = TempDir::new().unwrap();
        let (remote_path, _) = seed_bare_remote(sandbox.path(), "# Shared\n");
        let state_dir = TempDir::new().unwrap();
        let state = PersistentState::load(state_dir.path()).unwrap();
        let local = crate::workspace::clone_workspace(
            remote_path.to_str().unwrap(),
            sandbox.path().join("local").to_str().unwrap(),
            None,
            &state,
        )
        .unwrap();
        let mut repo = Repository::open(&local.root).unwrap();
        fs::write(Path::new(&local.root).join("manual.txt"), "manual stash").unwrap();
        let manual_stash = repo
            .stash_save(
                &test_signature(),
                "manual snapshot",
                Some(StashFlags::INCLUDE_UNTRACKED),
            )
            .unwrap();
        let opened = crate::documents::read_document(&local.root, "notes/shared.md").unwrap();
        crate::documents::save_document(
            crate::types::SaveDocumentRequest {
                root: local.root.clone(),
                path: opened.path,
                content: "# Shared\n\nSynced\n".to_owned(),
                expected_sha256: Some(opened.sha256),
                expected_missing: false,
                encoding: opened.encoding,
            },
            &state,
        )
        .unwrap();
        fs::write(Path::new(&local.root).join("scratch.txt"), "keep visible").unwrap();

        assert!(
            sync_workspace_changes(&local.root, None, &state)
                .unwrap()
                .pushed
        );
        assert_eq!(
            fs::read_to_string(Path::new(&local.root).join("scratch.txt")).unwrap(),
            "keep visible"
        );
        let mut remaining = Vec::new();
        repo.stash_foreach(|_, _, oid| {
            remaining.push(*oid);
            true
        })
        .unwrap();
        assert_eq!(remaining, vec![manual_stash]);
    }

    #[test]
    fn resuming_after_stash_apply_crash_does_not_apply_the_snapshot_twice() {
        let directory = TempDir::new().unwrap();
        let state_dir = TempDir::new().unwrap();
        let state = PersistentState::load(state_dir.path()).unwrap();
        let repo = init_with_main(directory.path());
        commit_file(&repo, "notes/shared.md", "# Shared\n", "initial");
        fs::write(
            directory.path().join("notes/shared.md"),
            "# Shared\n\nLocal draft\n",
        )
        .unwrap();
        fs::write(directory.path().join("scratch.txt"), "untracked draft").unwrap();

        let operation_id = "stash-apply-crash";
        let mut repo = Repository::open(directory.path()).unwrap();
        let stash_oid = stash_if_needed(&mut repo, operation_id).unwrap().unwrap();
        let operation = PendingGitOperation {
            id: operation_id.to_owned(),
            root: directory.path().to_string_lossy().into_owned(),
            kind: GitOperationKind::Pull,
            phase: GitOperationPhase::RestoreWorkingTree,
            started_at: Utc::now().to_rfc3339(),
            workspace_changes: Vec::new(),
            changed_paths: Vec::new(),
            committed: false,
            commit_id: None,
            pulled: true,
            pushed: false,
            original_head_oid: repo.head().unwrap().target().map(|oid| oid.to_string()),
            stash_oid: Some(stash_oid.to_string()),
            aborting: false,
            stash_apply_started: true,
            stash_applied: false,
        };
        state.begin_git_operation(operation.clone()).unwrap();

        let stash_index = operation_stash_index(&mut repo, &operation).unwrap();
        let mut options = StashApplyOptions::new();
        options.reinstantiate_index();
        repo.stash_apply(stash_index, Some(&mut options)).unwrap();
        drop(repo);

        let result = resume_git_operation(&operation.root, None, &state).unwrap();

        assert!(result.error.is_none(), "{:?}", result.error);
        assert!(state.pending_git_operation(&operation.root).is_none());
        assert_eq!(
            fs::read_to_string(directory.path().join("notes/shared.md"))
                .unwrap()
                .replace("\r\n", "\n"),
            "# Shared\n\nLocal draft\n"
        );
        assert_eq!(
            fs::read_to_string(directory.path().join("scratch.txt")).unwrap(),
            "untracked draft"
        );
    }

    #[test]
    fn resuming_before_stash_apply_restores_the_preserved_snapshot() {
        let directory = TempDir::new().unwrap();
        let state_dir = TempDir::new().unwrap();
        let state = PersistentState::load(state_dir.path()).unwrap();
        let repo = init_with_main(directory.path());
        commit_file(&repo, "notes/shared.md", "# Shared\n", "initial");
        fs::write(
            directory.path().join("notes/shared.md"),
            "# Shared\n\nLocal draft\n",
        )
        .unwrap();

        let operation_id = "before-stash-apply";
        let mut repo = Repository::open(directory.path()).unwrap();
        let stash_oid = stash_if_needed(&mut repo, operation_id).unwrap().unwrap();
        let operation = PendingGitOperation {
            id: operation_id.to_owned(),
            root: directory.path().to_string_lossy().into_owned(),
            kind: GitOperationKind::Pull,
            phase: GitOperationPhase::RestoreWorkingTree,
            started_at: Utc::now().to_rfc3339(),
            workspace_changes: Vec::new(),
            changed_paths: Vec::new(),
            committed: false,
            commit_id: None,
            pulled: true,
            pushed: false,
            original_head_oid: repo.head().unwrap().target().map(|oid| oid.to_string()),
            stash_oid: Some(stash_oid.to_string()),
            aborting: false,
            stash_apply_started: true,
            stash_applied: false,
        };
        state.begin_git_operation(operation.clone()).unwrap();
        drop(repo);

        let result = resume_git_operation(&operation.root, None, &state).unwrap();

        assert!(result.error.is_none(), "{:?}", result.error);
        assert_eq!(
            fs::read_to_string(directory.path().join("notes/shared.md"))
                .unwrap()
                .replace("\r\n", "\n"),
            "# Shared\n\nLocal draft\n"
        );
    }

    #[test]
    fn interrupted_stash_recovery_refuses_to_overwrite_a_new_external_edit() {
        let directory = TempDir::new().unwrap();
        let state_dir = TempDir::new().unwrap();
        let state = PersistentState::load(state_dir.path()).unwrap();
        let repo = init_with_main(directory.path());
        commit_file(&repo, "notes/shared.md", "# Shared\n", "initial");
        fs::write(
            directory.path().join("notes/shared.md"),
            "# Shared\n\nPreserved draft\n",
        )
        .unwrap();

        let operation_id = "ambiguous-stash-apply";
        let mut repo = Repository::open(directory.path()).unwrap();
        let stash_oid = stash_if_needed(&mut repo, operation_id).unwrap().unwrap();
        let operation = PendingGitOperation {
            id: operation_id.to_owned(),
            root: directory.path().to_string_lossy().into_owned(),
            kind: GitOperationKind::Pull,
            phase: GitOperationPhase::RestoreWorkingTree,
            started_at: Utc::now().to_rfc3339(),
            workspace_changes: Vec::new(),
            changed_paths: Vec::new(),
            committed: false,
            commit_id: None,
            pulled: true,
            pushed: false,
            original_head_oid: repo.head().unwrap().target().map(|oid| oid.to_string()),
            stash_oid: Some(stash_oid.to_string()),
            aborting: false,
            stash_apply_started: true,
            stash_applied: false,
        };
        state.begin_git_operation(operation.clone()).unwrap();
        fs::write(
            directory.path().join("notes/shared.md"),
            "# Shared\n\nExternal edit after the crash\n",
        )
        .unwrap();
        drop(repo);

        let result = resume_git_operation(&operation.root, None, &state).unwrap();

        assert_eq!(result.failure_stage, Some(SyncStage::RestoreWorkingTree));
        assert!(state.pending_git_operation(&operation.root).is_some());
        assert_eq!(
            fs::read_to_string(directory.path().join("notes/shared.md")).unwrap(),
            "# Shared\n\nExternal edit after the crash\n"
        );
        let mut repo = Repository::open(directory.path()).unwrap();
        assert_eq!(
            find_operation_stash(&mut repo, operation_id).unwrap(),
            Some(stash_oid)
        );
    }

    #[test]
    fn abort_resume_finishes_after_the_stash_was_applied_before_a_crash() {
        let directory = TempDir::new().unwrap();
        let state_dir = TempDir::new().unwrap();
        let state = PersistentState::load(state_dir.path()).unwrap();
        let repo = init_with_main(directory.path());
        commit_file(&repo, "notes/shared.md", "# Shared\n", "initial");
        fs::write(
            directory.path().join("notes/shared.md"),
            "# Shared\n\nLocal draft\n",
        )
        .unwrap();
        fs::write(directory.path().join("scratch.txt"), "untracked draft").unwrap();

        let operation_id = "abort-apply-crash";
        let mut repo = Repository::open(directory.path()).unwrap();
        let original_head_oid = repo.head().unwrap().target().map(|oid| oid.to_string());
        let stash_oid = stash_if_needed(&mut repo, operation_id).unwrap().unwrap();
        let operation = PendingGitOperation {
            id: operation_id.to_owned(),
            root: directory.path().to_string_lossy().into_owned(),
            kind: GitOperationKind::Pull,
            phase: GitOperationPhase::Rebase,
            started_at: Utc::now().to_rfc3339(),
            workspace_changes: Vec::new(),
            changed_paths: Vec::new(),
            committed: false,
            commit_id: None,
            pulled: false,
            pushed: false,
            original_head_oid,
            stash_oid: Some(stash_oid.to_string()),
            aborting: true,
            stash_apply_started: true,
            stash_applied: false,
        };
        state.begin_git_operation(operation.clone()).unwrap();
        let stash_index = operation_stash_index(&mut repo, &operation).unwrap();
        let mut options = StashApplyOptions::new();
        options.reinstantiate_index();
        repo.stash_apply(stash_index, Some(&mut options)).unwrap();
        drop(repo);

        abort_git_operation(&operation.root, &state).unwrap();

        assert!(state.pending_git_operation(&operation.root).is_none());
        assert_eq!(
            fs::read_to_string(directory.path().join("notes/shared.md"))
                .unwrap()
                .replace("\r\n", "\n"),
            "# Shared\n\nLocal draft\n"
        );
        assert_eq!(
            fs::read_to_string(directory.path().join("scratch.txt")).unwrap(),
            "untracked draft"
        );
        let mut repo = Repository::open(directory.path()).unwrap();
        assert_eq!(find_operation_stash(&mut repo, operation_id).unwrap(), None);
    }
}
