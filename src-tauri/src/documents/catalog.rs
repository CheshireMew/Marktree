use std::{
    collections::BTreeMap,
    fs,
    io::Read,
    path::Path,
    time::{Duration, Instant},
};

use globset::GlobSet;
use walkdir::{DirEntry, WalkDir};

use super::{
    config::{build_ignore_set, read_workspace_config},
    content::modified_ms,
};
use crate::{
    content_policy::{
        document_kind, is_observable_workspace_path, is_visible_workspace_path,
        VERSIONED_WORKSPACE_CONFIG,
    },
    error::{AppError, AppResult},
    paths::{canonical_root, normalize_relative, path_to_slashes, resolve_existing_file},
    types::{
        DocumentKind, DocumentSearchMatchType, DocumentSearchResponse, DocumentSearchResult,
        GitFileStatus, GitStatusSnapshot, SearchStatistics, WorkspaceEntriesPatch, WorkspaceEntry,
        WorkspaceEntryType,
    },
};

const MAX_SEARCH_FILE_BYTES: u64 = 8 * 1024 * 1024;
const MAX_SEARCH_TOTAL_BYTES: u64 = 64 * 1024 * 1024;
const MAX_SEARCH_DURATION: Duration = Duration::from_secs(3);

pub(crate) struct SearchBudget {
    started: Instant,
    statistics: SearchStatistics,
}

pub(crate) struct SearchCriteria<'a> {
    pub(crate) query: &'a str,
    pub(crate) limit: usize,
    pub(crate) path_prefix: Option<&'a str>,
    pub(crate) file_kinds: &'a [DocumentKind],
    pub(crate) modified_after_ms: Option<u64>,
}

impl Default for SearchBudget {
    fn default() -> Self {
        Self {
            started: Instant::now(),
            statistics: SearchStatistics::default(),
        }
    }
}

impl SearchBudget {
    fn should_stop(&mut self) -> bool {
        if self.started.elapsed() >= MAX_SEARCH_DURATION
            || self.statistics.scanned_bytes >= MAX_SEARCH_TOTAL_BYTES
        {
            self.statistics.truncated = true;
            true
        } else {
            false
        }
    }

    fn reserve_file(&mut self, bytes: u64) -> bool {
        if bytes > MAX_SEARCH_FILE_BYTES {
            self.statistics.skipped_large_files += 1;
            self.statistics.truncated = true;
            return false;
        }
        if self.statistics.scanned_bytes.saturating_add(bytes) > MAX_SEARCH_TOTAL_BYTES {
            self.statistics.truncated = true;
            return false;
        }
        self.statistics.scanned_files += 1;
        self.statistics.scanned_bytes = self.statistics.scanned_bytes.saturating_add(bytes);
        true
    }

    pub(crate) fn statistics(&self) -> SearchStatistics {
        self.statistics.clone()
    }
}

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
            let metadata = match entry.metadata() {
                Ok(metadata) => metadata,
                Err(error) if walkdir_error_is_not_found(&error) => return Ok(true),
                Err(error) => return Err(AppError::Io(error.into())),
            };
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

pub fn list_workspace_directories(root: &str) -> AppResult<Vec<String>> {
    let mut directories = Vec::new();
    scan_workspace_entries(
        root,
        || true,
        |entry, relative| {
            if entry.file_type().is_dir() {
                directories.push(relative.to_owned());
            }
            Ok(true)
        },
    )?;
    directories.sort_by_key(|path| path.to_lowercase());
    Ok(directories)
}

pub fn workspace_entries_patch(
    root: &str,
    paths: &[String],
    status: Option<GitStatusSnapshot>,
) -> AppResult<WorkspaceEntriesPatch> {
    let root_path = canonical_root(root)?;
    let config = read_workspace_config(root)?.config;
    let ignore_set = build_ignore_set(&config.ignore_rules)?;
    let statuses = status
        .as_ref()
        .map(|snapshot| snapshot.files.as_slice())
        .unwrap_or_default();
    let status_map: BTreeMap<&str, &GitFileStatus> = statuses
        .iter()
        .map(|item| (item.path.as_str(), item))
        .collect();
    let mut normalized = paths
        .iter()
        .map(|path| normalize_relative(path))
        .collect::<AppResult<Vec<_>>>()?;
    normalized.sort_by_key(|path| path.len());
    normalized.dedup();
    let mut roots = Vec::<String>::new();
    for path in normalized {
        if roots
            .iter()
            .any(|parent| path == *parent || path.starts_with(&format!("{parent}/")))
        {
            continue;
        }
        roots.push(path);
    }

    if roots.iter().any(|path| path == VERSIONED_WORKSPACE_CONFIG) {
        return Ok(WorkspaceEntriesPatch {
            entries: Vec::new(),
            removed_paths: roots,
            status,
            full_reload_required: true,
        });
    }

    let mut entries = BTreeMap::<String, WorkspaceEntry>::new();
    for relative_root in &roots {
        let path = root_path.join(relative_root);
        let metadata = match fs::symlink_metadata(&path) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => return Err(error.into()),
        };
        let is_directory = metadata.is_dir();
        if !is_observable_workspace_path(relative_root, is_directory, &ignore_set) {
            continue;
        }
        if is_directory {
            for entry in WalkDir::new(&path)
                .follow_links(false)
                .into_iter()
                .filter_entry(|entry| {
                    if entry.depth() == 0 {
                        return true;
                    }
                    entry
                        .path()
                        .strip_prefix(&root_path)
                        .map(path_to_slashes)
                        .ok()
                        .is_some_and(|relative| {
                            is_visible_workspace_path(
                                &relative,
                                entry.file_type().is_dir(),
                                &ignore_set,
                            )
                        })
                })
            {
                let entry = match entry {
                    Ok(entry) => entry,
                    Err(error) if walkdir_error_is_not_found(&error) => continue,
                    Err(error) => return Err(AppError::Io(error.into())),
                };
                let relative = entry
                    .path()
                    .strip_prefix(&root_path)
                    .map(path_to_slashes)
                    .map_err(|_| AppError::InvalidPath(entry.path().display().to_string()))?;
                if let Some(value) = workspace_entry(entry.path(), &relative, &status_map)? {
                    entries.insert(relative, value);
                }
            }
        } else if let Some(value) = workspace_entry(&path, relative_root, &status_map)? {
            entries.insert(relative_root.clone(), value);
        }
    }

    Ok(WorkspaceEntriesPatch {
        entries: entries.into_values().collect(),
        removed_paths: roots,
        status,
        full_reload_required: false,
    })
}

fn workspace_entry(
    path: &Path,
    relative: &str,
    statuses: &BTreeMap<&str, &GitFileStatus>,
) -> AppResult<Option<WorkspaceEntry>> {
    let metadata = match fs::metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error.into()),
    };
    let is_directory = metadata.is_dir();
    let kind = (!is_directory).then(|| document_kind(path));
    Ok(Some(WorkspaceEntry {
        path: relative.to_owned(),
        name: path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| relative.to_owned()),
        entry_type: if is_directory {
            WorkspaceEntryType::Directory
        } else {
            WorkspaceEntryType::File
        },
        file_kind: kind.clone(),
        size: if is_directory { 0 } else { metadata.len() },
        modified_ms: modified_ms(&metadata),
        read_only: !matches!(kind, Some(DocumentKind::Markdown | DocumentKind::Text)),
        git_status: (!is_directory)
            .then(|| statuses.get(relative).map(|value| (*value).clone()))
            .flatten(),
    }))
}

pub fn search_documents(
    root: &str,
    query: &str,
    limit: usize,
    is_current: impl Fn() -> bool,
) -> AppResult<DocumentSearchResponse> {
    search_documents_filtered(root, query, limit, None, &[], None, is_current)
}

pub fn search_documents_filtered(
    root: &str,
    query: &str,
    limit: usize,
    path_prefix: Option<&str>,
    file_kinds: &[DocumentKind],
    modified_after_ms: Option<u64>,
    is_current: impl Fn() -> bool,
) -> AppResult<DocumentSearchResponse> {
    let mut budget = SearchBudget::default();
    let results = search_documents_filtered_with_budget(
        root,
        SearchCriteria {
            query,
            limit,
            path_prefix,
            file_kinds,
            modified_after_ms,
        },
        is_current,
        &mut budget,
    )?;
    Ok(DocumentSearchResponse {
        results,
        statistics: budget.statistics(),
    })
}

pub(crate) fn search_documents_filtered_with_budget(
    root: &str,
    criteria: SearchCriteria<'_>,
    is_current: impl Fn() -> bool,
    budget: &mut SearchBudget,
) -> AppResult<Vec<DocumentSearchResult>> {
    let SearchCriteria {
        query,
        limit,
        path_prefix,
        file_kinds,
        modified_after_ms,
    } = criteria;
    let query = query.trim();
    let needle = query.to_lowercase();
    if needle.is_empty() {
        return Ok(Vec::new());
    }
    let path_prefix = path_prefix
        .map(str::trim)
        .filter(|prefix| !prefix.is_empty())
        .map(|prefix| prefix.replace('\\', "/").trim_matches('/').to_lowercase());
    let mut matches = Vec::new();
    let is_current = &is_current;
    scan_workspace_files(root, is_current, |entry, relative| {
        if budget.should_stop() || !is_current() {
            return Ok(false);
        }
        let file_kind = document_kind(entry.path());
        if !matches!(file_kind, DocumentKind::Markdown | DocumentKind::Text) {
            return Ok(true);
        }
        if !file_kinds.is_empty() && !file_kinds.contains(&file_kind) {
            return Ok(true);
        }
        if path_prefix.as_deref().is_some_and(|prefix| {
            let path = relative.to_lowercase();
            path != prefix && !path.starts_with(&format!("{prefix}/"))
        }) {
            return Ok(true);
        }
        let metadata = match entry.metadata() {
            Ok(metadata) => metadata,
            Err(error) if walkdir_error_is_not_found(&error) => return Ok(true),
            Err(error) => return Err(AppError::Io(error.into())),
        };
        let modified_ms = metadata
            .modified()
            .ok()
            .and_then(|modified| modified.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|duration| duration.as_millis() as u64)
            .unwrap_or_default();
        if modified_after_ms.is_some_and(|after| modified_ms < after) {
            return Ok(true);
        }
        if let Some(column) = match_column(relative, &needle) {
            matches.push(DocumentSearchResult {
                path: relative.to_owned(),
                line: None,
                column: Some(column),
                snippet: relative.to_owned(),
                match_type: DocumentSearchMatchType::Path,
                file_kind: file_kind.clone(),
                modified_ms,
            });
        }
        if matches.len() < limit.max(1) && budget.reserve_file(metadata.len()) {
            let mut bytes = Vec::with_capacity(metadata.len() as usize);
            let file = match fs::File::open(entry.path()) {
                Ok(file) => file,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(true),
                Err(error) => return Err(error.into()),
            };
            if let Err(error) = file
                .take(metadata.len().saturating_add(1))
                .read_to_end(&mut bytes)
            {
                if error.kind() == std::io::ErrorKind::NotFound {
                    return Ok(true);
                }
                return Err(error.into());
            }
            let content = match String::from_utf8(bytes) {
                Ok(content) => content,
                Err(_) => return Ok(true),
            };
            for (line_index, line) in content.lines().enumerate() {
                if budget.should_stop() || !is_current() {
                    return Ok(false);
                }
                let Some(column) = match_column(line, &needle) else {
                    continue;
                };
                matches.push(DocumentSearchResult {
                    path: relative.to_owned(),
                    line: Some(line_index + 1),
                    column: Some(column),
                    snippet: search_snippet(line, column, query.chars().count()),
                    match_type: DocumentSearchMatchType::Content,
                    file_kind: file_kind.clone(),
                    modified_ms,
                });
                if matches.len() >= limit.max(1) {
                    return Ok(false);
                }
            }
        }
        if matches.len() >= limit.max(1) {
            return Ok(false);
        }
        Ok(true)
    })?;
    Ok(matches)
}

fn match_column(value: &str, lowercase_needle: &str) -> Option<usize> {
    if value.is_ascii() && lowercase_needle.is_ascii() {
        let needle = lowercase_needle.as_bytes();
        if needle.is_empty() {
            return Some(1);
        }
        return value
            .as_bytes()
            .windows(needle.len())
            .position(|window| {
                window
                    .iter()
                    .zip(needle)
                    .all(|(left, right)| left.to_ascii_lowercase() == *right)
            })
            .map(|index| index + 1);
    }
    let mut lowercase = String::with_capacity(value.len());
    let mut original_columns = Vec::with_capacity(value.chars().count());
    for (character_index, character) in value.chars().enumerate() {
        for lowercase_character in character.to_lowercase() {
            let start = lowercase.len();
            lowercase.push(lowercase_character);
            original_columns.extend(std::iter::repeat_n(
                character_index + 1,
                lowercase.len().saturating_sub(start),
            ));
        }
    }
    lowercase
        .find(lowercase_needle)
        .and_then(|byte_index| original_columns.get(byte_index).copied())
}

fn search_snippet(line: &str, column: usize, query_length: usize) -> String {
    const MAX_CHARS: usize = 180;
    let normalized = line.replace('\t', "  ");
    let characters = normalized.chars().collect::<Vec<_>>();
    if characters.len() <= MAX_CHARS {
        return normalized.trim().to_owned();
    }
    let match_start = column.saturating_sub(1).min(characters.len());
    let match_end = (match_start + query_length).min(characters.len());
    let mut start = match_start.saturating_sub(MAX_CHARS / 3);
    let end = (start + MAX_CHARS).max(match_end).min(characters.len());
    start = end.saturating_sub(MAX_CHARS);
    let mut snippet = characters[start..end].iter().collect::<String>();
    if start > 0 {
        snippet.insert(0, '…');
    }
    if end < characters.len() {
        snippet.push('…');
    }
    snippet.trim().to_owned()
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

pub fn scan_versioned_workspace_files(
    root: &str,
    mut visit: impl FnMut(&Path, &str) -> AppResult<()>,
) -> AppResult<()> {
    scan_workspace_files(
        root,
        || true,
        |entry, relative| {
            visit(entry.path(), relative)?;
            Ok(true)
        },
    )?;
    let root_path = canonical_root(root)?;
    let config_path = root_path.join(VERSIONED_WORKSPACE_CONFIG);
    if config_path.is_file() {
        let config_path = resolve_existing_file(&root_path, VERSIONED_WORKSPACE_CONFIG)?;
        visit(&config_path, VERSIONED_WORKSPACE_CONFIG)?;
    }
    Ok(())
}

#[cfg(test)]
pub fn is_observable_path(root: &str, path: &Path) -> AppResult<bool> {
    let root_path = canonical_root(root)?;
    let relative = match path.strip_prefix(&root_path) {
        Ok(relative) => path_to_slashes(relative),
        Err(_) => return Ok(false),
    };
    if relative == VERSIONED_WORKSPACE_CONFIG {
        return Ok(true);
    }
    let config = read_workspace_config(root)?.config;
    let ignore_set = build_ignore_set(&config.ignore_rules)?;
    Ok(is_observable_workspace_path(
        &relative,
        path.is_dir(),
        &ignore_set,
    ))
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
        let entry = match entry {
            Ok(entry) => entry,
            Err(error) if walkdir_error_is_not_found(&error) => continue,
            Err(error) => return Err(AppError::Io(error.into())),
        };
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

fn walkdir_error_is_not_found(error: &walkdir::Error) -> bool {
    error
        .io_error()
        .is_some_and(|error| error.kind() == std::io::ErrorKind::NotFound)
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
    is_visible_workspace_path(&relative, entry.file_type().is_dir(), ignore_set)
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use super::*;

    #[test]
    fn match_column_preserves_original_unicode_columns() {
        assert_eq!(match_column("前缀Straße后缀", "straße"), Some(3));
        assert_eq!(match_column("İstanbul", &"İS".to_lowercase()), Some(1));
    }

    #[test]
    fn performance_ascii_match_is_case_insensitive_without_changing_columns() {
        assert_eq!(match_column("Prefix MARKTREE suffix", "marktree"), Some(8));
        assert_eq!(match_column("Prefix", "absent"), None);
    }

    #[test]
    fn non_matching_long_line_is_scanned_in_linear_time() {
        let value = "a".repeat(256 * 1024);
        let started = Instant::now();

        assert_eq!(match_column(&value, "absent"), None);
        assert!(
            started.elapsed() < Duration::from_secs(2),
            "long-line scan took {:?}",
            started.elapsed()
        );
    }

    #[test]
    fn workspace_scans_never_surface_operation_owned_files() {
        let workspace = tempfile::TempDir::new().unwrap();
        let root = workspace.path().to_string_lossy().into_owned();
        fs::write(workspace.path().join("note.md"), b"visible").unwrap();
        fs::write(
            workspace
                .path()
                .join(".note.md.marktree-0123456789abcdef01234567.tmp"),
            b"staged",
        )
        .unwrap();
        fs::write(workspace.path().join(".note.md.123.4.tmp"), b"legacy").unwrap();

        let mut paths = Vec::new();
        scan_versioned_workspace_files(&root, |_, relative| {
            paths.push(relative.to_owned());
            Ok(())
        })
        .unwrap();

        assert_eq!(paths, vec!["note.md"]);
    }
}
