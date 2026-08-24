#![cfg_attr(target_env = "msvc", allow(linker_messages))]

use std::{
    fs,
    io::Write,
    path::Path,
    process::{Command, Stdio},
};

#[cfg(feature = "test-local-remotes")]
use git2::build::CheckoutBuilder;
use git2::{IndexAddOption, Repository, Signature};
use serde_json::Value;
use tempfile::TempDir;

fn run_cli(state_dir: &Path, arguments: &[&str], stdin: Option<&str>) -> (i32, Value) {
    let mut command = Command::new(env!("CARGO_BIN_EXE_marktree-cli"));
    command
        .arg("--state-dir")
        .arg(state_dir)
        .args(arguments)
        .stdin(if stdin.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = command.spawn().unwrap();
    if let Some(input) = stdin {
        child
            .stdin
            .take()
            .unwrap()
            .write_all(input.as_bytes())
            .unwrap();
    }
    let output = child.wait_with_output().unwrap();
    let stdout = String::from_utf8(output.stdout).unwrap();
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.is_empty(), "unexpected CLI stderr: {stderr}");
    let payload = serde_json::from_str(&stdout)
        .unwrap_or_else(|error| panic!("invalid CLI JSON ({error}): {stdout}"));
    (output.status.code().unwrap(), payload)
}

#[test]
fn read_only_command_does_not_create_the_state_directory() {
    let directory = tempfile::TempDir::new().unwrap();
    let workspace = directory.path().join("workspace");
    fs::create_dir(&workspace).unwrap();
    fs::write(workspace.join("notes.md"), "hello").unwrap();
    let state_dir = directory.path().join("state-does-not-exist");

    let (code, output) = run_cli(
        &state_dir,
        &[
            "document",
            "read",
            "--root",
            workspace.to_str().unwrap(),
            "--path",
            "notes.md",
        ],
        None,
    );

    assert_eq!(code, 0, "{output}");
    assert!(!state_dir.exists());
}

#[test]
fn plain_workspace_cli_writes_real_bytes_and_reports_no_git_manifest() {
    let workspace = TempDir::new().unwrap();
    let state = TempDir::new().unwrap();
    let root = workspace.path().to_str().unwrap();

    let (code, folder) = run_cli(
        state.path(),
        &["folder", "create", "--root", root, "--path", "10-Knowledge"],
        None,
    );
    assert_eq!(code, 0);
    assert_eq!(folder["ok"], true);

    let content = "# CLI knowledge\n\nsource faithful\n";
    let (code, written) = run_cli(
        state.path(),
        &[
            "document",
            "write",
            "--root",
            root,
            "--path",
            "10-Knowledge/CLI.md",
            "--expected-missing",
        ],
        Some(content),
    );
    assert_eq!(code, 0);
    assert_eq!(written["command"], "document.write");
    assert_eq!(
        fs::read(workspace.path().join("10-Knowledge/CLI.md")).unwrap(),
        content.as_bytes()
    );

    let (_, read) = run_cli(
        state.path(),
        &[
            "document",
            "read",
            "--root",
            root,
            "--path",
            "10-Knowledge/CLI.md",
        ],
        None,
    );
    assert_eq!(read["data"]["content"], content);

    let (_, changes) = run_cli(state.path(), &["changes", "--root", root], None);
    assert_eq!(changes["data"], serde_json::json!([]));
}

#[test]
fn cli_sync_rejects_a_plain_workspace_inside_a_parent_repository() {
    let parent = TempDir::new().unwrap();
    let state = TempDir::new().unwrap();
    let repository = initialize_repository(parent.path());
    let original_head = repository.head().unwrap().target();
    let workspace = parent.path().join("plain-workspace");
    fs::create_dir(&workspace).unwrap();
    fs::write(workspace.join("note.md"), b"# Plain\n").unwrap();
    let root = workspace.to_str().unwrap();

    let (plan_code, plan) = run_cli(state.path(), &["sync", "plan", "--root", root], None);
    assert_eq!(plan_code, 1, "{plan}");
    assert_eq!(plan["ok"], false);
    assert!(plan["error"]["message"]
        .as_str()
        .unwrap()
        .contains("does not own a Git repository"));

    let (run_code, run) = run_cli(state.path(), &["sync", "run", "--root", root], None);
    assert_eq!(run_code, 1, "{run}");
    assert_eq!(run["ok"], false);
    assert_eq!(repository.head().unwrap().target(), original_head);
    let status = repository
        .status_file(Path::new("plain-workspace/note.md"))
        .unwrap();
    assert!(status.contains(git2::Status::WT_NEW));
    assert!(!status.intersects(
        git2::Status::INDEX_NEW | git2::Status::INDEX_MODIFIED | git2::Status::INDEX_DELETED
    ));
}

#[test]
fn cli_rejects_a_stale_write_without_touching_the_file() {
    let workspace = TempDir::new().unwrap();
    let state = TempDir::new().unwrap();
    let root = workspace.path().to_str().unwrap();

    let (_, first) = run_cli(
        state.path(),
        &[
            "document",
            "write",
            "--root",
            root,
            "--path",
            "note.md",
            "--expected-missing",
        ],
        Some("first\n"),
    );
    assert_eq!(first["ok"], true);

    let (code, stale) = run_cli(
        state.path(),
        &[
            "document",
            "write",
            "--root",
            root,
            "--path",
            "note.md",
            "--expected-sha256",
            &"0".repeat(64),
        ],
        Some("second\n"),
    );
    assert_eq!(code, 1);
    assert_eq!(stale["error"]["code"], "externalChange");
    assert_eq!(
        fs::read(workspace.path().join("note.md")).unwrap(),
        b"first\n"
    );
}

#[test]
fn cli_batch_validates_every_version_before_the_first_write() {
    let workspace = TempDir::new().unwrap();
    let state = TempDir::new().unwrap();
    let root = workspace.path().to_str().unwrap();
    fs::write(workspace.path().join("existing.md"), b"current\n").unwrap();
    let input = serde_json::json!({
        "root": root,
        "writes": [
            {
                "path": "new.md",
                "content": "new\n",
                "expectedMissing": true
            },
            {
                "path": "existing.md",
                "content": "replacement\n",
                "expectedSha256": "0".repeat(64)
            }
        ]
    });

    let (code, rejected) = run_cli(
        state.path(),
        &["document", "write-batch"],
        Some(&input.to_string()),
    );

    assert_eq!(code, 1);
    assert_eq!(rejected["error"]["code"], "externalChange");
    assert!(!workspace.path().join("new.md").exists());
    assert_eq!(
        fs::read(workspace.path().join("existing.md")).unwrap(),
        b"current\n"
    );
}

#[test]
fn concurrent_cli_processes_preserve_both_git_change_records() {
    let workspace = TempDir::new().unwrap();
    let state = TempDir::new().unwrap();
    initialize_repository(workspace.path());
    let root = workspace.path().to_str().unwrap().to_owned();
    let state_path = state.path().to_path_buf();

    let spawn_write = |path: &'static str, content: &'static str| {
        let root = root.clone();
        let state_path = state_path.clone();
        std::thread::spawn(move || {
            run_cli(
                &state_path,
                &[
                    "document",
                    "write",
                    "--root",
                    &root,
                    "--path",
                    path,
                    "--expected-missing",
                ],
                Some(content),
            )
        })
    };

    let left_process = spawn_write("left.md", "left\n");
    let right_process = spawn_write("right.md", "right\n");
    let (left_code, left) = left_process.join().unwrap();
    let (right_code, right) = right_process.join().unwrap();
    assert_eq!(left_code, 0, "{left}");
    assert_eq!(right_code, 0, "{right}");

    let (_, changes) = run_cli(state.path(), &["changes", "--root", &root], None);
    let mut paths = changes["data"]
        .as_array()
        .unwrap()
        .iter()
        .map(|change| change["path"].as_str().unwrap())
        .collect::<Vec<_>>();
    paths.sort();
    assert_eq!(paths, vec!["left.md", "right.md"]);
}

#[test]
#[cfg(feature = "test-local-remotes")]
fn cli_write_sync_and_second_clone_read_use_the_real_git_chain() {
    let remote_dir = TempDir::new().unwrap();
    let seed_dir = TempDir::new().unwrap();
    let first_dir = TempDir::new().unwrap();
    let second_dir = TempDir::new().unwrap();
    let state = TempDir::new().unwrap();
    let remote = Repository::init_bare(remote_dir.path()).unwrap();
    let seed = initialize_repository(seed_dir.path());
    seed.remote("origin", remote_dir.path().to_str().unwrap())
        .unwrap();
    seed.find_remote("origin")
        .unwrap()
        .push(&["refs/heads/main:refs/heads/main"], None)
        .unwrap();
    remote.set_head("refs/heads/main").unwrap();

    let first = Repository::clone(remote_dir.path().to_str().unwrap(), first_dir.path()).unwrap();
    let second = Repository::clone(remote_dir.path().to_str().unwrap(), second_dir.path()).unwrap();
    configure_identity(&first);
    configure_identity(&second);
    let first_root = first_dir.path().to_str().unwrap();

    let (_, written) = run_cli(
        state.path(),
        &[
            "document",
            "write",
            "--root",
            first_root,
            "--path",
            "10-Knowledge/Agent.md",
            "--expected-missing",
        ],
        Some("# Agent managed\n\nsecond clone proof\n"),
    );
    assert_eq!(written["ok"], true);
    let (_, plan) = run_cli(state.path(), &["sync", "plan", "--root", first_root], None);
    assert_eq!(
        plan["data"]["changedPaths"],
        serde_json::json!(["10-Knowledge/Agent.md"])
    );

    let (sync_code, sync) = run_cli(state.path(), &["sync", "run", "--root", first_root], None);
    assert_eq!(sync_code, 0, "{sync}");
    assert_eq!(sync["data"]["pushed"], true, "{sync}");

    fast_forward_from_origin(&second);
    let (_, consumed) = run_cli(
        state.path(),
        &[
            "document",
            "read",
            "--root",
            second_dir.path().to_str().unwrap(),
            "--path",
            "10-Knowledge/Agent.md",
        ],
        None,
    );
    assert_eq!(
        consumed["data"]["content"],
        "# Agent managed\n\nsecond clone proof\n"
    );
}

fn initialize_repository(path: &Path) -> Repository {
    let repository = Repository::init(path).unwrap();
    configure_identity(&repository);
    repository.set_head("refs/heads/main").unwrap();
    fs::write(path.join("README.md"), b"# baseline\n").unwrap();
    let mut index = repository.index().unwrap();
    index
        .add_all(["README.md"], IndexAddOption::DEFAULT, None)
        .unwrap();
    index.write().unwrap();
    let tree_id = index.write_tree().unwrap();
    {
        let tree = repository.find_tree(tree_id).unwrap();
        let signature = Signature::now("Marktree Test", "marktree@example.invalid").unwrap();
        repository
            .commit(Some("HEAD"), &signature, &signature, "baseline", &tree, &[])
            .unwrap();
    }
    repository
}

fn configure_identity(repository: &Repository) {
    let mut config = repository.config().unwrap();
    config.set_str("user.name", "Marktree Test").unwrap();
    config
        .set_str("user.email", "marktree@example.invalid")
        .unwrap();
    config.set_bool("core.autocrlf", false).unwrap();
}

#[cfg(feature = "test-local-remotes")]
fn fast_forward_from_origin(repository: &Repository) {
    repository
        .find_remote("origin")
        .unwrap()
        .fetch(&["main"], None, None)
        .unwrap();
    let target = repository
        .find_reference("refs/remotes/origin/main")
        .unwrap()
        .target()
        .unwrap();
    repository
        .reference("refs/heads/main", target, true, "test fast-forward")
        .unwrap();
    repository.set_head("refs/heads/main").unwrap();
    let mut checkout = CheckoutBuilder::new();
    checkout.force();
    repository.checkout_head(Some(&mut checkout)).unwrap();
}
