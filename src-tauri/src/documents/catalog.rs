use std::{collections::BTreeMap, fs, path::Path};

use globset::GlobSet;
use walkdir::{DirEntry, WalkDir};

use super::{
    config::{build_ignore_set, read_workspace_config},
    content::modified_ms,
};
use crate::{
    content_policy::document_kind,
    error::{AppError, AppResult},
    paths::{canonical_root, path_to_slashes},
    types::{DocumentKind, GitFileStatus, WorkspaceEntry, WorkspaceEntryType},
};

pub fn list_workspace_entries(
    root: &str,
    statuses: &[GitFileStatus],
) -> AppResult<Vec<WorkspaceEntry>> {
    let status_map: BTreeMap<&str, &GitFileStatus> = statuses
        .iter()
        .map(|status| (status.path.as_str(), status))
        .collect();
    let mut entries = Vec::new();
    scan_workspace_entries(
        root,
        || true,
        |entry, relative_string| {
            let metadata = entry
                .metadata()
                .map_err(|error| AppError::Io(error.into()))?;
            let is_directory = entry.file_type().is_dir();
            let kind = (!is_directory).then(|| document_kind(entry.path()));
            let name = entry.file_name().to_string_lossy().into_owned();
            let read_only = !matches!(kind, Some(DocumentKind::Markdown | DocumentKind::Text));
            entries.push(WorkspaceEntry {
                path: relative_string.to_owned(),
                name,
                entry_type: if is_directory {
                    WorkspaceEntryType::Directory
                } else {
                    WorkspaceEntryType::File
                },
                file_kind: kind,
                size: if is_directory { 0 } else { metadata.len() },
                modified_ms: modified_ms(&metadata),
                read_only,
                git_status: (!is_directory)
                    .then(|| {
                        status_map
                            .get(relative_string)
                            .map(|value| (*value).clone())
                    })
                    .flatten(),
            });
            Ok(true)
        },
    )?;

    entries.sort_by(|left, right| {
        let left_rank = u8::from(left.entry_type == WorkspaceEntryType::File);
        let right_rank = u8::from(right.entry_type == WorkspaceEntryType::File);
        left_rank
            .cmp(&right_rank)
            .then_with(|| left.path.to_lowercase().cmp(&right.path.to_lowercase()))
    });
    Ok(entries)
}

pub fn search_documents(
    root: &str,
    query: &str,
    limit: usize,
    is_current: impl Fn() -> bool,
) -> AppResult<Vec<String>> {
    let needle = query.to_lowercase();
    if needle.trim().is_empty() {
        return Ok(Vec::new());
    }
    let mut matches = Vec::new();
    scan_workspace_files(root, is_current, |entry, relative| {
        if !matches!(
            document_kind(entry.path()),
            DocumentKind::Markdown | DocumentKind::Text
        ) {
            return Ok(true);
        }
        let path_match = relative.to_lowercase().contains(&needle);
        let content_match = if path_match {
            false
        } else {
            match fs::read_to_string(entry.path()) {
                Ok(content) => content.to_lowercase().contains(&needle),
                Err(error) if error.kind() == std::io::ErrorKind::InvalidData => false,
                Err(error) => return Err(error.into()),
            }
        };
        if path_match || content_match {
            matches.push(relative.to_owned());
            if matches.len() >= limit.max(1) {
                return Ok(false);
            }
        }
        Ok(true)
    })?;
    Ok(matches)
}

pub fn scan_workspace_files(
    root: &str,
    is_current: impl Fn() -> bool,
    mut visit: impl FnMut(&DirEntry, &str) -> AppResult<bool>,
) -> AppResult<()> {
    scan_workspace_entries(root, is_current, |entry, relative| {
        if entry.file_type().is_file() {
            visit(entry, relative)
        } else {
            Ok(true)
        }
    })
}

pub fn scan_workspace_entries(
    root: &str,
    is_current: impl Fn() -> bool,
    mut visit: impl FnMut(&DirEntry, &str) -> AppResult<bool>,
) -> AppResult<()> {
    let root_path = canonical_root(root)?;
    let config = read_workspace_config(root)?.config;
    let ignore_set = build_ignore_set(&config.ignore_rules)?;
    for entry in WalkDir::new(&root_path)
        .follow_links(false)
        .into_iter()
        .filter_entry(|entry| should_descend(entry, &root_path, &ignore_set))
    {
        if !is_current() {
            break;
        }
        let entry = entry.map_err(|error| AppError::Io(error.into()))?;
        if entry.depth() == 0 {
            continue;
        }
        let relative = entry
            .path()
            .strip_prefix(&root_path)
            .map(path_to_slashes)
            .map_err(|_| AppError::InvalidPath(entry.path().display().to_string()))?;
        if !visit(&entry, &relative)? {
            break;
        }
    }
    Ok(())
}

pub(super) fn should_descend(entry: &DirEntry, root: &Path, ignore_set: &GlobSet) -> bool {
    if entry.depth() == 0 {
        return true;
    }
    let relative = entry
        .path()
        .strip_prefix(root)
        .map(path_to_slashes)
        .unwrap_or_default();
    if ignore_set.is_match(&relative) {
        return false;
    }
    if relative
        .split('/')
        .next()
        .is_some_and(|part| matches!(part, ".git" | ".marktree"))
    {
        return false;
    }
    if !entry.file_type().is_dir() {
        return true;
    }
    !matches!(
        entry.file_name().to_string_lossy().as_ref(),
        ".git" | ".marktree" | "node_modules" | "target" | "dist" | ".gradle" | ".idea"
    )
}
