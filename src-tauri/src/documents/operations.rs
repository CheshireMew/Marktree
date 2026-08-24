use std::{fs, path::Path};

#[cfg(target_os = "android")]
use chrono::Utc;
#[cfg(target_os = "android")]
use serde::{Deserialize, Serialize};
#[cfg(target_os = "android")]
use std::path::PathBuf;
use walkdir::WalkDir;

use super::config::{build_ignore_set, read_workspace_config};
#[cfg(target_os = "android")]
use crate::file_version::hash_bytes;
use crate::{
    content_policy::is_visible_workspace_path,
    error::{AppError, AppResult},
    file_version::hash_file,
    paths::{
        canonical_root, normalize_content_relative, path_to_slashes, resolve_existing_entry,
        resolve_for_write,
    },
    state::PersistentState,
    types::{
        TrashEntry, WorkspaceEntryDuplicateResult, WorkspaceEntryMoveResult, WorkspacePathMove,
    },
    workspace_operation::{
        execute_mutation, WorkspaceChangeIntent, WorkspaceCopyFile, WorkspaceOperationKind,
    },
};

pub fn create_folder(root: &str, path: &str, state: &PersistentState) -> AppResult<String> {
    let root_path = canonical_root(root)?;
    let relative = editable_relative(path)?;
    let destination = resolve_for_write(&root_path, &relative)?;
    if destination.exists() {
        return Err(AppError::Message(
            "A file or folder already exists at that path.".to_owned(),
        ));
    }
    execute_mutation(
        root,
        WorkspaceOperationKind::CreateFolder {
            path: relative.clone(),
        },
        Vec::new(),
        state,
        (),
        |_| {
            fs::create_dir(&destination)?;
            Ok(())
        },
    )?;
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
    let moved_files = collect_path_moves(root, &source, &source_relative, &destination_relative)?;
    let mut changes = Vec::with_capacity(moved_files.len().saturating_mul(2));
    for moved in &moved_files {
        let source_file = resolve_existing_entry(&root_path, &moved.old_path)?;
        let version = hash_file(&source_file)?;
        changes.push(WorkspaceChangeIntent::delete(&moved.old_path));
        changes.push(WorkspaceChangeIntent::upsert(&moved.new_path, version));
    }
    execute_mutation(
        root,
        WorkspaceOperationKind::MoveEntry {
            source_path: source_relative.clone(),
            destination_path: destination_relative.clone(),
            moved_files: moved_files.clone(),
        },
        changes,
        state,
        (),
        |_| {
            if let Some(parent) = destination.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::rename(&source, &destination)?;
            Ok(())
        },
    )?;
    Ok(WorkspaceEntryMoveResult {
        old_path: source_relative,
        new_path: destination_relative,
        moved_files,
    })
}

pub fn duplicate_entry(
    root: &str,
    source_path: &str,
    destination_path: &str,
    state: &PersistentState,
) -> AppResult<WorkspaceEntryDuplicateResult> {
    let root_path = canonical_root(root)?;
    let source_relative = editable_relative(source_path)?;
    let destination_relative = editable_relative(destination_path)?;
    let source = resolve_existing_entry(&root_path, &source_relative)?;
    let destination = resolve_for_write(&root_path, &destination_relative)?;
    if destination.exists() {
        return Err(AppError::Message(
            "The duplicate destination already exists.".to_owned(),
        ));
    }
    if source.is_dir() && destination.starts_with(&source) {
        return Err(AppError::Message(
            "A folder cannot be duplicated inside itself.".to_owned(),
        ));
    }
    let (copied_files, directories) =
        collect_copy_plan(root, &source, &source_relative, &destination_relative)?;
    let changes = copied_files
        .iter()
        .map(|copied| WorkspaceChangeIntent::upsert(&copied.destination_path, &copied.version))
        .collect();
    let result_files = copied_files
        .iter()
        .map(|copied| WorkspacePathMove {
            old_path: copied.source_path.clone(),
            new_path: copied.destination_path.clone(),
        })
        .collect::<Vec<_>>();
    execute_mutation(
        root,
        WorkspaceOperationKind::DuplicateEntry {
            source_path: source_relative.clone(),
            destination_path: destination_relative.clone(),
            copied_files: copied_files.clone(),
            directories: directories.clone(),
        },
        changes,
        state,
        (),
        |operation| {
            for directory in &directories {
                fs::create_dir_all(resolve_for_write(&root_path, directory)?)?;
            }
            for copied in &copied_files {
                let source_file = resolve_existing_entry(&root_path, &copied.source_path)?;
                let destination_file = resolve_for_write(&root_path, &copied.destination_path)?;
                if let Some(parent) = destination_file.parent() {
                    fs::create_dir_all(parent)?;
                }
                crate::paths::atomic_copy_for_operation(
                    &source_file,
                    &destination_file,
                    &operation.id,
                )?;
                crate::workspace_operation::test_abrupt_stop(
                    crate::workspace_operation::TestCrashPoint::CopyFileApplied,
                );
            }
            Ok(())
        },
    )?;
    Ok(WorkspaceEntryDuplicateResult {
        source_path: source_relative,
        new_path: destination_relative,
        copied_files: result_files,
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
    let deleted_files = collect_relative_files(root, &source, &relative)?;
    #[cfg(target_os = "android")]
    let prepared_trash = Some(prepare_android_trash_entry(root, &relative));
    #[cfg(not(target_os = "android"))]
    let prepared_trash: Option<TrashEntry> = None;
    let changes = deleted_files
        .into_iter()
        .map(WorkspaceChangeIntent::delete)
        .collect();
    execute_mutation(
        root,
        WorkspaceOperationKind::TrashEntry {
            path: relative,
            trash_entry: prepared_trash.clone(),
        },
        changes,
        state,
        prepared_trash.clone(),
        |_| {
            #[cfg(target_os = "windows")]
            {
                let _ = app_data_dir;
                trash::delete(&source).map_err(|error| AppError::Message(error.to_string()))?;
                Ok(None)
            }
            #[cfg(target_os = "android")]
            {
                let entry = prepared_trash
                    .as_ref()
                    .expect("Android trash operation must have metadata");
                move_to_android_trash(&source, entry, app_data_dir)?;
                Ok(Some(entry.clone()))
            }
            #[cfg(not(any(target_os = "windows", target_os = "android")))]
            {
                let _ = (app_data_dir, source);
                Err(AppError::Message(
                    "Recoverable deletion is not available on this platform.".to_owned(),
                ))
            }
        },
    )
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
        let payload = directory.join("payload");
        let restored_files = collect_relative_files(
            &metadata.entry.workspace_root,
            &payload,
            &metadata.entry.original_path,
        )?;
        let mut changes = Vec::with_capacity(restored_files.len());
        for relative in restored_files {
            let suffix = relative
                .strip_prefix(&metadata.entry.original_path)
                .unwrap_or_default()
                .trim_start_matches('/');
            let absolute = if suffix.is_empty() {
                payload.clone()
            } else {
                payload.join(suffix)
            };
            changes.push(WorkspaceChangeIntent::upsert(
                relative,
                hash_file(&absolute)?,
            ));
        }
        let restored = metadata.entry.clone();
        let restored_root = restored.workspace_root.clone();
        execute_mutation(
            &restored_root,
            WorkspaceOperationKind::RestoreTrash {
                trash_entry: restored.clone(),
            },
            changes,
            state,
            restored.clone(),
            |_| {
                if let Some(parent) = destination.parent() {
                    fs::create_dir_all(parent)?;
                }
                fs::rename(&payload, &destination)?;
                fs::remove_file(directory.join("metadata.json"))?;
                fs::remove_dir(&directory)?;
                Ok(restored)
            },
        )
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
    normalize_content_relative(path)
}

fn collect_path_moves(
    root: &str,
    source: &Path,
    source_relative: &str,
    destination_relative: &str,
) -> AppResult<Vec<WorkspacePathMove>> {
    collect_relative_files(root, source, source_relative).map(|files| {
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

fn collect_copy_plan(
    root: &str,
    source: &Path,
    source_relative: &str,
    destination_relative: &str,
) -> AppResult<(Vec<WorkspaceCopyFile>, Vec<String>)> {
    let config = read_workspace_config(root)?.config;
    let ignore_set = build_ignore_set(&config.ignore_rules)?;
    if source.is_file() {
        return Ok((
            vec![WorkspaceCopyFile {
                source_path: source_relative.to_owned(),
                destination_path: destination_relative.to_owned(),
                version: hash_file(source)?,
            }],
            Vec::new(),
        ));
    }
    let mut files = Vec::new();
    let mut directories = Vec::new();
    for entry in WalkDir::new(source)
        .follow_links(false)
        .into_iter()
        .filter_entry(|entry| {
            if entry.depth() == 0 {
                return true;
            }
            let suffix = entry
                .path()
                .strip_prefix(source)
                .map(path_to_slashes)
                .unwrap_or_default();
            let relative = format!("{source_relative}/{suffix}");
            is_visible_workspace_path(&relative, entry.file_type().is_dir(), &ignore_set)
        })
    {
        let entry = entry.map_err(|error| AppError::Io(error.into()))?;
        if entry.file_type().is_symlink() {
            return Err(AppError::Message(
                "Symbolic links cannot be duplicated by Marktree.".to_owned(),
            ));
        }
        let suffix = entry
            .path()
            .strip_prefix(source)
            .map(path_to_slashes)
            .map_err(|_| AppError::InvalidPath(entry.path().display().to_string()))?;
        let destination = if suffix.is_empty() {
            destination_relative.to_owned()
        } else {
            format!("{destination_relative}/{suffix}")
        };
        if entry.file_type().is_dir() {
            directories.push(destination);
        } else if entry.file_type().is_file() {
            files.push(WorkspaceCopyFile {
                source_path: if suffix.is_empty() {
                    source_relative.to_owned()
                } else {
                    format!("{source_relative}/{suffix}")
                },
                destination_path: destination,
                version: hash_file(entry.path())?,
            });
        }
    }
    Ok((files, directories))
}

fn collect_relative_files(
    root: &str,
    source: &Path,
    source_relative: &str,
) -> AppResult<Vec<String>> {
    let config = read_workspace_config(root)?.config;
    let ignore_set = build_ignore_set(&config.ignore_rules)?;
    if source.is_file() {
        return if is_visible_workspace_path(source_relative, false, &ignore_set) {
            Ok(vec![source_relative.to_owned()])
        } else {
            Ok(Vec::new())
        };
    }
    let mut files = Vec::new();
    for entry in WalkDir::new(source)
        .follow_links(false)
        .into_iter()
        .filter_entry(|entry| {
            if entry.depth() == 0 {
                return true;
            }
            let suffix = entry
                .path()
                .strip_prefix(source)
                .map(path_to_slashes)
                .unwrap_or_default();
            let relative = if suffix.is_empty() {
                source_relative.to_owned()
            } else {
                format!("{source_relative}/{suffix}")
            };
            is_visible_workspace_path(&relative, entry.file_type().is_dir(), &ignore_set)
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
fn move_to_android_trash(source: &Path, entry: &TrashEntry, app_data_dir: &Path) -> AppResult<()> {
    let directory = android_trash_directory(app_data_dir, entry);
    fs::create_dir_all(&directory)?;
    fs::rename(source, directory.join("payload"))?;
    let metadata = AndroidTrashMetadata {
        entry: entry.clone(),
    };
    crate::paths::atomic_write(
        &directory.join("metadata.json"),
        &serde_json::to_vec_pretty(&metadata)?,
    )?;
    Ok(())
}

#[cfg(target_os = "android")]
fn prepare_android_trash_entry(workspace_root: &str, original_path: &str) -> TrashEntry {
    let seed = format!(
        "{workspace_root}\n{original_path}\n{}",
        Utc::now().timestamp_nanos_opt().unwrap_or_default()
    );
    let id = hash_bytes(seed.as_bytes())[..24].to_owned();
    TrashEntry {
        id,
        workspace_root: workspace_root.to_owned(),
        original_path: original_path.to_owned(),
        name: Path::new(original_path)
            .file_name()
            .map(|value| value.to_string_lossy().into_owned())
            .unwrap_or_else(|| original_path.to_owned()),
        deleted_at: Utc::now().to_rfc3339(),
    }
}

#[cfg(target_os = "android")]
fn android_trash_directory(app_data_dir: &Path, entry: &TrashEntry) -> PathBuf {
    let workspace_id = hash_bytes(entry.workspace_root.as_bytes())[..16].to_owned();
    app_data_dir
        .join("trash")
        .join(workspace_id)
        .join(&entry.id)
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
