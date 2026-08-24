use crate::{error::AppResult, state::PersistentState, types::TrashEntry};

#[cfg(target_os = "android")]
use crate::error::AppError;

#[cfg(target_os = "android")]
fn android_trash_directory(state: &PersistentState, entry: &TrashEntry) -> std::path::PathBuf {
    let workspace_id =
        crate::file_version::hash_bytes(entry.workspace_root.as_bytes())[..16].to_owned();
    state
        .app_data_dir()
        .join("trash")
        .join(workspace_id)
        .join(&entry.id)
}

#[cfg(target_os = "android")]
pub(super) fn finish_applied_android_trash(
    state: &PersistentState,
    entry: &Option<TrashEntry>,
) -> AppResult<()> {
    let Some(entry) = entry else {
        return Ok(());
    };
    let directory = android_trash_directory(state, entry);
    if !directory.join("payload").exists() {
        return Err(AppError::Message(format!(
            "The trash payload for '{}' is missing.",
            entry.original_path
        )));
    }
    let metadata = serde_json::json!({ "entry": entry });
    crate::paths::atomic_write(
        &directory.join("metadata.json"),
        &serde_json::to_vec_pretty(&metadata)?,
    )
}

#[cfg(not(target_os = "android"))]
pub(super) fn finish_applied_android_trash(
    _state: &PersistentState,
    _entry: &Option<TrashEntry>,
) -> AppResult<()> {
    Ok(())
}

#[cfg(target_os = "android")]
pub(super) fn cleanup_unapplied_android_trash(
    state: &PersistentState,
    entry: &Option<TrashEntry>,
) -> AppResult<()> {
    let Some(entry) = entry else {
        return Ok(());
    };
    let directory = android_trash_directory(state, entry);
    if directory.exists() && !directory.join("payload").exists() {
        let metadata = directory.join("metadata.json");
        if metadata.exists() {
            std::fs::remove_file(metadata)?;
        }
        std::fs::remove_dir(directory)?;
    }
    Ok(())
}

#[cfg(not(target_os = "android"))]
pub(super) fn cleanup_unapplied_android_trash(
    _state: &PersistentState,
    _entry: &Option<TrashEntry>,
) -> AppResult<()> {
    Ok(())
}

#[cfg(target_os = "android")]
pub(super) fn restored_android_payload_exists(state: &PersistentState, entry: &TrashEntry) -> bool {
    android_trash_directory(state, entry)
        .join("payload")
        .exists()
}

#[cfg(not(target_os = "android"))]
pub(super) fn restored_android_payload_exists(
    _state: &PersistentState,
    _entry: &TrashEntry,
) -> bool {
    false
}

#[cfg(target_os = "android")]
pub(super) fn cleanup_restored_android_trash(
    state: &PersistentState,
    entry: &TrashEntry,
) -> AppResult<()> {
    let directory = android_trash_directory(state, entry);
    let metadata = directory.join("metadata.json");
    if metadata.exists() {
        std::fs::remove_file(metadata)?;
    }
    if directory.exists() {
        std::fs::remove_dir(directory)?;
    }
    Ok(())
}

#[cfg(not(target_os = "android"))]
pub(super) fn cleanup_restored_android_trash(
    _state: &PersistentState,
    _entry: &TrashEntry,
) -> AppResult<()> {
    Ok(())
}
