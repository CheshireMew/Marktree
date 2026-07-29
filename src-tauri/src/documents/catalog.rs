use std::{collections::BTreeMap, fs, path::Path};

use globset::GlobSet;
use walkdir::{DirEntry, WalkDir};

use super::{
    config::{build_ignore_set, read_repository_config},
    content::modified_ms,
};
use crate::{
    content_policy::document_kind,
    error::{AppError, AppResult},
    paths::{canonical_root, path_to_slashes},
    types::{DocumentDescriptor, DocumentKind, GitFileStatus},
};

pub fn list_documents(
    root: &str,
    statuses: &[GitFileStatus],
) -> AppResult<Vec<DocumentDescriptor>> {
    let status_map: BTreeMap<&str, &GitFileStatus> = statuses
        .iter()
        .map(|status| (status.path.as_str(), status))
        .collect();
    let mut documents = Vec::new();
    scan_repository_files(
        root,
        || true,
        |entry, relative_string| {
            if matches!(relative_string, ".git" | ".marktree/config.json") {
                return Ok(true);
            }
            let metadata = entry
                .metadata()
                .map_err(|error| AppError::Io(error.into()))?;
            let kind = document_kind(entry.path());
            let read_only = kind != DocumentKind::Markdown;
            let name = entry.file_name().to_string_lossy().into_owned();
            let extension = entry
                .path()
                .extension()
                .map(|value| value.to_string_lossy().to_ascii_lowercase())
                .unwrap_or_default();

            documents.push(DocumentDescriptor {
                path: relative_string.to_owned(),
                name,
                extension,
                size: metadata.len(),
                modified_ms: modified_ms(&metadata),
                read_only,
                kind,
                git_status: status_map
                    .get(relative_string)
                    .map(|value| (*value).clone()),
            });
            Ok(true)
        },
    )?;

    documents.sort_by(|left, right| {
        let left_rank = kind_rank(&left.kind);
        let right_rank = kind_rank(&right.kind);
        left_rank
            .cmp(&right_rank)
            .then_with(|| left.path.to_lowercase().cmp(&right.path.to_lowercase()))
    });
    Ok(documents)
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
    scan_repository_files(root, is_current, |entry, relative| {
        if document_kind(entry.path()) != DocumentKind::Markdown {
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

fn scan_repository_files(
    root: &str,
    is_current: impl Fn() -> bool,
    mut visit: impl FnMut(&DirEntry, &str) -> AppResult<bool>,
) -> AppResult<()> {
    let root_path = canonical_root(root)?;
    let config = read_repository_config(root)?.config;
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
        if !entry.file_type().is_file() {
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

fn should_descend(entry: &DirEntry, root: &Path, ignore_set: &GlobSet) -> bool {
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
    if !entry.file_type().is_dir() {
        return true;
    }
    !matches!(
        entry.file_name().to_string_lossy().as_ref(),
        ".git" | "node_modules" | "target" | "dist" | ".gradle" | ".idea"
    )
}

fn kind_rank(kind: &DocumentKind) -> u8 {
    match kind {
        DocumentKind::Markdown => 0,
        DocumentKind::Image => 1,
        DocumentKind::Text => 2,
        DocumentKind::Other => 3,
    }
}
