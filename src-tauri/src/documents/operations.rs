use std::{fs, path::Path};

#[cfg(target_os = "android")]
use chrono::Utc;
#[cfg(target_os = "android")]
use serde::{Deserialize, Serialize};
#[cfg(target_os = "android")]
use std::path::PathBuf;
use walkdir::WalkDir;

#[cfg(target_os = "android")]
use crate::file_version::hash_bytes;
use crate::{
    error::{AppError, AppResult},
    file_version::hash_file,
    git,
    paths::{
        canonical_root, normalize_relative, path_to_slashes, resolve_existing_entry,
        resolve_for_write,
    },
    state::PersistentState,
    types::{TrashEntry, WorkspaceChangeOperation, WorkspaceEntryMoveResult, WorkspacePathMove},
};

pub fn create_folder(root: &str, path: &str) -> AppResult<String> {
    let root_path = canonical_root(root)?;
    let relative = editable_relative(path)?;
    let destination = resolve_for_write(&root_path, &relative)?;
    if destination.exists() {
        return Err(AppError::Message(
            "A file or folder already exists at that path.".to_owned(),
        ));
    }
    fs::create_dir(&destination)?;
    Ok(relative)
}

pub fn move_entry(
    root: &str,
    source_path: &str,
    destination_path: &str,
    state: &PersistentState,
) -> AppResult<WorkspaceEntryMoveResult> {
    let root_path = canonical_root(root)?;
    let source_relative = editable_relative(source_path)?;
    let destination_relative = editable_relative(destination_path)?;
    let source = resolve_existing_entry(&root_path, &source_relative)?;
    let destination = resolve_for_write(&root_path, &destination_relative)?;
    if destination.exists() {
        return Err(AppError::Message(
            "The destination already exists.".to_owned(),
        ));
    }
    if source.is_dir() && destination.starts_with(&source) {
        return Err(AppError::Message(
            "A folder cannot be moved inside itself.".to_owned(),
        ));
    }
    let moved_files = collect_path_moves(&source, &source_relative, &destination_relative)?;
    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::rename(&source, &destination)?;
    if git::has_git_capability(root) {
        for moved in &moved_files {
            state.record_workspace_change(
                root,
                &moved.old_path,
                WorkspaceChangeOperation::Delete,
                None,
            )?;
            let destination_file = resolve_existing_entry(&root_path, &moved.new_path)?;
            let version = hash_file(&destination_file)?;
            state.record_workspace_change(
                root,
                &moved.new_path,
                WorkspaceChangeOperation::Upsert,
                Some(&version),
            )?;
        }
    }
    Ok(WorkspaceEntryMoveResult {
        old_path: source_relative,
        new_path: destination_relative,
        moved_files,
    })
}

pub fn trash_entry(
    root: &str,
    path: &str,
    app_data_dir: &Path,
    state: &PersistentState,
) -> AppResult<Option<TrashEntry>> {
    let root_path = canonical_root(root)?;
    let relative = editable_relative(path)?;
    let source = resolve_existing_entry(&root_path, &relative)?;
    let deleted_files = collect_relative_files(&source, &relative)?;
    #[cfg(target_os = "windows")]
    {
        let _ = app_data_dir;
        trash::delete(&source).map_err(|error| AppError::Message(error.to_string()))?;
    }
    #[cfg(target_os = "android")]
    let trash = move_to_android_trash(&source, root, &relative, app_data_dir)?;
    #[cfg(not(any(target_os = "windows", target_os = "android")))]
    {
        let _ = app_data_dir;
        return Err(AppError::Message(
            "Recoverable deletion is not available on this platform.".to_owned(),
        ));
    }
    if git::has_git_capability(root) {
        for file in deleted_files {
            state.record_workspace_change(root, &file, WorkspaceChangeOperation::Delete, None)?;
        }
    }
    #[cfg(target_os = "android")]
    {
        Ok(Some(trash))
    }
    #[cfg(not(target_os = "android"))]
    {
        Ok(None)
    }
}

pub fn list_android_trash(app_data_dir: &Path) -> AppResult<Vec<TrashEntry>> {
    #[cfg(target_os = "android")]
    {
        let trash_root = app_data_dir.join("trash");
        if !trash_root.exists() {
            return Ok(Vec::new());
        }
        let mut entries = Vec::new();
        for entry in WalkDir::new(trash_root)
            .min_depth(3)
            .max_depth(3)
            .into_iter()
        {
            let entry = entry.map_err(|error| AppError::Io(error.into()))?;
            if entry.file_name() != "metadata.json" {
                continue;
            }
            let metadata: AndroidTrashMetadata = serde_json::from_slice(&fs::read(entry.path())?)?;
            entries.push(metadata.entry);
        }
        entries.sort_by(|left, right| right.deleted_at.cmp(&left.deleted_at));
        Ok(entries)
    }
    #[cfg(not(target_os = "android"))]
    {
        let _ = app_data_dir;
        Ok(Vec::new())
    }
}

pub fn restore_android_trash(
    app_data_dir: &Path,
    id: &str,
    state: &PersistentState,
) -> AppResult<TrashEntry> {
    #[cfg(target_os = "android")]
    {
        let (directory, metadata) = find_android_trash(app_data_dir, id)?;
        let root_path = canonical_root(&metadata.entry.workspace_root)?;
        let destination = resolve_for_write(&root_path, &metadata.entry.original_path)?;
        if destination.exists() {
            return Err(AppError::Message(
                "The original location is no longer empty.".to_owned(),
            ));
        }
        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::rename(directory.join("payload"), &destination)?;
        if git::has_git_capability(&metadata.entry.workspace_root) {
            for relative in collect_relative_files(&destination, &metadata.entry.original_path)? {
                let absolute = resolve_existing_entry(&root_path, &relative)?;
                let version = hash_file(&absolute)?;
                state.record_workspace_change(
                    &metadata.entry.workspace_root,
                    &relative,
                    WorkspaceChangeOperation::Upsert,
                    Some(&version),
                )?;
            }
        }
        fs::remove_file(directory.join("metadata.json"))?;
        fs::remove_dir(directory)?;
        Ok(metadata.entry)
    }
    #[cfg(not(target_os = "android"))]
    {
        let _ = (app_data_dir, id, state);
        Err(AppError::Message(
            "Marktree-managed trash is only used on Android.".to_owned(),
        ))
    }
}

pub fn empty_android_trash(app_data_dir: &Path) -> AppResult<()> {
    #[cfg(target_os = "android")]
    {
        let trash_root = app_data_dir.join("trash");
        if trash_root.exists() {
            fs::remove_dir_all(&trash_root)?;
        }
        fs::create_dir_all(trash_root)?;
        Ok(())
    }
    #[cfg(not(target_os = "android"))]
    {
        let _ = app_data_dir;
        Err(AppError::Message(
            "Marktree-managed trash is only used on Android.".to_owned(),
        ))
    }
}

fn editable_relative(path: &str) -> AppResult<String> {
    let relative = normalize_relative(path)?;
    if relative
        .split('/')
        .next()
        .is_some_and(|part| part.eq_ignore_ascii_case(".marktree"))
    {
        return Err(AppError::InvalidPath(relative));
    }
    Ok(relative)
}

fn collect_path_moves(
    source: &Path,
    source_relative: &str,
    destination_relative: &str,
) -> AppResult<Vec<WorkspacePathMove>> {
    collect_relative_files(source, source_relative).map(|files| {
        files
            .into_iter()
            .map(|old_path| {
                let suffix = old_path
                    .strip_prefix(source_relative)
                    .unwrap_or_default()
                    .trim_start_matches('/');
                let new_path = if suffix.is_empty() {
                    destination_relative.to_owned()
                } else {
                    format!("{destination_relative}/{suffix}")
                };
                WorkspacePathMove { old_path, new_path }
            })
            .collect()
    })
}

fn collect_relative_files(source: &Path, source_relative: &str) -> AppResult<Vec<String>> {
    if source.is_file() {
        return Ok(vec![source_relative.to_owned()]);
    }
    let mut files = Vec::new();
    for entry in WalkDir::new(source)
        .follow_links(false)
        .into_iter()
        .filter_entry(|entry| {
            entry.depth() == 0
                || !matches!(
                    entry.file_name().to_string_lossy().as_ref(),
                    ".git" | ".marktree"
                )
        })
    {
        let entry = entry.map_err(|error| AppError::Io(error.into()))?;
        if entry.file_type().is_symlink() {
            return Err(AppError::Message(
                "Symbolic links cannot be moved or deleted by Marktree.".to_owned(),
            ));
        }
        if !entry.file_type().is_file() {
            continue;
        }
        let suffix = entry
            .path()
            .strip_prefix(source)
            .map(path_to_slashes)
            .map_err(|_| AppError::InvalidPath(entry.path().display().to_string()))?;
        files.push(if suffix.is_empty() {
            source_relative.to_owned()
        } else {
            format!("{source_relative}/{suffix}")
        });
    }
    Ok(files)
}

#[cfg(target_os = "android")]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AndroidTrashMetadata {
    entry: TrashEntry,
}

#[cfg(target_os = "android")]
fn move_to_android_trash(
    source: &Path,
    workspace_root: &str,
    original_path: &str,
    app_data_dir: &Path,
) -> AppResult<TrashEntry> {
    let seed = format!(
        "{workspace_root}\n{original_path}\n{}",
        Utc::now().timestamp_nanos_opt().unwrap_or_default()
    );
    let id = hash_bytes(seed.as_bytes())[..24].to_owned();
    let workspace_id = hash_bytes(workspace_root.as_bytes())[..16].to_owned();
    let directory = app_data_dir.join("trash").join(workspace_id).join(&id);
    fs::create_dir_all(&directory)?;
    fs::rename(source, directory.join("payload"))?;
    let entry = TrashEntry {
        id,
        workspace_root: workspace_root.to_owned(),
        original_path: original_path.to_owned(),
        name: Path::new(original_path)
            .file_name()
            .map(|value| value.to_string_lossy().into_owned())
            .unwrap_or_else(|| original_path.to_owned()),
        deleted_at: Utc::now().to_rfc3339(),
    };
    let metadata = AndroidTrashMetadata {
        entry: entry.clone(),
    };
    fs::write(
        directory.join("metadata.json"),
        serde_json::to_vec_pretty(&metadata)?,
    )?;
    Ok(entry)
}

#[cfg(target_os = "android")]
fn find_android_trash(app_data_dir: &Path, id: &str) -> AppResult<(PathBuf, AndroidTrashMetadata)> {
    for entry in WalkDir::new(app_data_dir.join("trash"))
        .min_depth(3)
        .max_depth(3)
        .into_iter()
    {
        let entry = entry.map_err(|error| AppError::Io(error.into()))?;
        if entry.file_name() != "metadata.json" {
            continue;
        }
        let metadata: AndroidTrashMetadata = serde_json::from_slice(&fs::read(entry.path())?)?;
        if metadata.entry.id == id {
            return Ok((
                entry
                    .path()
                    .parent()
                    .ok_or_else(|| AppError::InvalidPath(id.to_owned()))?
                    .to_path_buf(),
                metadata,
            ));
        }
    }
    Err(AppError::FileNotFound {
        path: id.to_owned(),
    })
}
