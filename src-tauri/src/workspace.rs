use std::{collections::BTreeSet, fs, path::Path};

use walkdir::WalkDir;

use crate::{
    documents,
    error::{AppError, AppResult},
    file_version::hash_bytes,
    git,
    paths::{canonical_root, path_to_slashes},
    state::PersistentState,
    types::{CredentialRecord, GitBaselinePreview, WorkspaceDescriptor},
    workspace_operation,
};

pub fn open_workspace(path: &str, state: &PersistentState) -> AppResult<WorkspaceDescriptor> {
    let root = canonical_root(path)?;
    let descriptor = descriptor_for_root(&root, false)?;
    state.register_workspace(&descriptor.root)?;
    Ok(descriptor)
}

pub fn create_workspace(path: &str, state: &PersistentState) -> AppResult<WorkspaceDescriptor> {
    fs::create_dir_all(path)?;
    open_workspace(path, state)
}

#[cfg(test)]
pub fn clone_workspace(
    remote_url: &str,
    path: &str,
    credential: Option<CredentialRecord>,
    state: &PersistentState,
) -> AppResult<WorkspaceDescriptor> {
    let descriptor = clone_workspace_unregistered(remote_url, path, credential)?;
    state.register_workspace(&descriptor.root)?;
    Ok(descriptor)
}

pub(crate) fn clone_workspace_unregistered(
    remote_url: &str,
    path: &str,
    credential: Option<CredentialRecord>,
) -> AppResult<WorkspaceDescriptor> {
    git::clone_repository(remote_url, path, credential)?;
    descriptor_for_root(&canonical_root(path)?, true)
}

pub fn refresh_workspace(root: &str) -> AppResult<WorkspaceDescriptor> {
    descriptor_for_root(&canonical_root(root)?, true)
}

pub fn preview_git_baseline(root: &str) -> AppResult<GitBaselinePreview> {
    if git::has_git_capability(root) {
        return Err(AppError::Message(
            "Version management is already enabled for this workspace.".to_owned(),
        ));
    }
    let mut versioned = BTreeSet::new();
    let mut total_bytes = 0u64;
    documents::scan_versioned_workspace_files(root, |path, relative| {
        versioned.insert(relative.to_owned());
        let metadata = path.metadata()?;
        total_bytes = total_bytes.saturating_add(metadata.len());
        Ok(())
    })?;
    let root_path = canonical_root(root)?;
    let mut ignored_count = 0usize;
    for entry in WalkDir::new(&root_path).follow_links(false) {
        let entry = entry.map_err(|error| AppError::Io(error.into()))?;
        if !entry.file_type().is_file() {
            continue;
        }
        let relative = entry
            .path()
            .strip_prefix(&root_path)
            .map(path_to_slashes)
            .map_err(|_| AppError::InvalidPath(entry.path().display().to_string()))?;
        let internal = relative.split('/').any(|part| {
            part.eq_ignore_ascii_case(".git") || part.eq_ignore_ascii_case(".marktree")
        });
        if !internal && !versioned.contains(&relative) {
            ignored_count = ignored_count.saturating_add(1);
        }
    }
    let config = documents::read_workspace_config(root)?.config;
    Ok(GitBaselinePreview {
        file_count: versioned.len(),
        total_bytes,
        ignored_count,
        ignore_rules: config.ignore_rules,
    })
}

pub fn enable_git(root: &str, state: &PersistentState) -> AppResult<WorkspaceDescriptor> {
    let preview = preview_git_baseline(root)?;
    let mut paths = Vec::with_capacity(preview.file_count);
    documents::scan_versioned_workspace_files(root, |_path, relative| {
        paths.push(relative.to_owned());
        Ok(())
    })?;
    workspace_operation::enable_git(root, paths, state)?;
    refresh_workspace(root)
}

fn descriptor_for_root(root: &Path, include_status: bool) -> AppResult<WorkspaceDescriptor> {
    let root_string = root.to_string_lossy().into_owned();
    let id = hash_bytes(normalized_identity(&root_string).as_bytes())[..16].to_owned();
    let name = root
        .file_name()
        .map(|value| value.to_string_lossy().into_owned())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| root_string.clone());
    Ok(WorkspaceDescriptor {
        id,
        name,
        root: root_string.clone(),
        git: if include_status {
            git::git_capability(&root_string)?
        } else {
            git::git_capability_metadata(&root_string)?
        },
    })
}

fn normalized_identity(path: &str) -> String {
    if cfg!(target_os = "windows") {
        path.to_lowercase()
    } else {
        path.to_owned()
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use git2::Repository;
    use tempfile::TempDir;

    use super::*;
    use crate::{
        documents,
        types::{SaveDocumentRequest, TextEncoding, WorkspaceChangeOperation},
    };

    #[test]
    fn ordinary_parent_stays_plain_while_nested_repository_content_is_visible() {
        let parent = TempDir::new().unwrap();
        let app_data = TempDir::new().unwrap();
        let state = PersistentState::load(app_data.path()).unwrap();
        let nested = parent.path().join("nested-notes");
        fs::create_dir(&nested).unwrap();
        Repository::init(&nested).unwrap();
        fs::write(nested.join("nested.md"), "# Nested\n").unwrap();
        fs::write(parent.path().join("plain.txt"), "before\r\n").unwrap();

        let parent_descriptor = open_workspace(parent.path().to_str().unwrap(), &state).unwrap();
        assert!(parent_descriptor.git.is_none());
        let entries = documents::list_workspace_entries(&parent_descriptor.root, &[]).unwrap();
        assert!(entries.iter().any(|entry| entry.path == "nested-notes"));
        assert!(entries
            .iter()
            .any(|entry| entry.path == "nested-notes/nested.md"));
        assert!(!entries.iter().any(|entry| entry.path.contains("/.git")));

        let opened =
            documents::open_document(&parent_descriptor.root, "plain.txt", &state).unwrap();
        documents::save_document(
            SaveDocumentRequest {
                root: parent_descriptor.root.clone(),
                path: opened.path,
                content: "after\r\n".to_owned(),
                expected_sha256: Some(opened.sha256),
                expected_missing: false,
                encoding: TextEncoding::Utf8,
            },
            &state,
        )
        .unwrap();
        assert_eq!(
            fs::read(parent.path().join("plain.txt")).unwrap(),
            b"after\r\n"
        );
        assert!(state.workspace_changes(&parent_descriptor.root).is_empty());

        let nested_descriptor = open_workspace(nested.to_str().unwrap(), &state).unwrap();
        assert!(nested_descriptor.git.is_some());
    }

    #[test]
    fn enabling_git_creates_a_complete_versioned_baseline_and_tracks_later_moves() {
        let directory = TempDir::new().unwrap();
        let app_data = TempDir::new().unwrap();
        let state = PersistentState::load(app_data.path()).unwrap();
        fs::write(directory.path().join("note.md"), "# Baseline\n").unwrap();
        fs::create_dir(directory.path().join("target")).unwrap();
        fs::write(
            directory.path().join("target").join("ignored.txt"),
            "ignored",
        )
        .unwrap();
        let root = directory.path().to_string_lossy().into_owned();

        let preview = preview_git_baseline(&root).unwrap();
        assert_eq!(preview.file_count, 1);
        assert_eq!(preview.ignored_count, 1);
        let descriptor = enable_git(&root, &state).unwrap();
        assert!(descriptor.git.is_some());
        let repository = Repository::open(&root).unwrap();
        let baseline = repository.head().unwrap().peel_to_commit().unwrap();
        assert!(baseline
            .tree()
            .unwrap()
            .get_path(Path::new("note.md"))
            .is_ok());
        assert!(baseline
            .tree()
            .unwrap()
            .get_path(Path::new("target/ignored.txt"))
            .is_err());

        documents::create_folder(&root, "archive", &state).unwrap();
        documents::move_entry(&root, "note.md", "archive/note.md", &state).unwrap();
        let changes = state.workspace_changes(&root);
        assert!(changes.iter().any(|change| {
            change.path == "note.md" && change.operation == WorkspaceChangeOperation::Delete
        }));
        assert!(changes.iter().any(|change| {
            change.path == "archive/note.md"
                && change.operation == WorkspaceChangeOperation::Upsert
                && change.version.is_some()
        }));
    }
}
