use std::{
    fs,
    path::{Path, PathBuf},
    time::{Duration, SystemTime},
};

use walkdir::WalkDir;

use crate::{
    error::{AppError, AppResult},
    paths::paths_equal,
};

const SHARE_INBOX: &str = "share-inbox";
const WORKSPACE_EXPORTS: &str = "workspace-exports";
const CACHE_RETENTION: Duration = Duration::from_secs(24 * 60 * 60);
const MAX_TRANSFER_CACHE_BYTES: u64 = 512 * 1024 * 1024;

pub(crate) fn share_inbox(app_cache_dir: &Path) -> PathBuf {
    app_cache_dir.join(SHARE_INBOX)
}

pub(crate) fn workspace_exports(app_cache_dir: &Path) -> PathBuf {
    app_cache_dir.join(WORKSPACE_EXPORTS)
}

pub(crate) fn prepare(app_cache_dir: &Path) -> AppResult<()> {
    let inbox = share_inbox(app_cache_dir);
    let exports = workspace_exports(app_cache_dir);
    fs::create_dir_all(&inbox)?;
    fs::create_dir_all(&exports)?;
    cleanup_roots(
        &[inbox, exports],
        SystemTime::now(),
        MAX_TRANSFER_CACHE_BYTES,
    )
}

pub(crate) fn incoming_file(path: &str, app_cache_dir: &Path) -> AppResult<PathBuf> {
    let path = PathBuf::from(path);
    if !path.is_file() {
        return Err(AppError::Message(
            "The shared file is no longer available.".to_owned(),
        ));
    }
    let canonical = path.canonicalize()?;
    let inbox = share_inbox(app_cache_dir).canonicalize()?;
    if !canonical.starts_with(&inbox)
        || paths_equal(&canonical.to_string_lossy(), &inbox.to_string_lossy())
    {
        return Err(AppError::InvalidPath(canonical.display().to_string()));
    }
    Ok(canonical)
}

pub(crate) fn consume_incoming_file(path: &Path, app_cache_dir: &Path) {
    let Ok(inbox) = share_inbox(app_cache_dir).canonicalize() else {
        return;
    };
    let Ok(path) = path.canonicalize() else {
        return;
    };
    if !path.starts_with(&inbox) {
        return;
    }
    let Some(transfer_directory) = path.parent() else {
        return;
    };
    if transfer_directory
        .parent()
        .is_some_and(|parent| paths_equal(&parent.to_string_lossy(), &inbox.to_string_lossy()))
    {
        let _ = fs::remove_dir_all(transfer_directory);
    }
}

fn cleanup_roots(roots: &[PathBuf], now: SystemTime, max_bytes: u64) -> AppResult<()> {
    let mut entries = Vec::new();
    for root in roots {
        for entry in fs::read_dir(root)? {
            let entry = entry?;
            let path = entry.path();
            let (bytes, modified) = entry_usage(&path)?;
            entries.push((path, bytes, modified));
        }
    }
    entries.sort_by_key(|(_, _, modified)| *modified);

    let cutoff = now
        .checked_sub(CACHE_RETENTION)
        .unwrap_or(SystemTime::UNIX_EPOCH);
    let mut retained_bytes = entries.iter().map(|(_, bytes, _)| *bytes).sum::<u64>();
    for (path, bytes, modified) in entries {
        if (modified < cutoff || retained_bytes > max_bytes) && remove_cache_entry(&path).is_ok() {
            retained_bytes = retained_bytes.saturating_sub(bytes);
        }
    }
    Ok(())
}

fn entry_usage(path: &Path) -> AppResult<(u64, SystemTime)> {
    let metadata = fs::metadata(path)?;
    if metadata.is_file() {
        return Ok((
            metadata.len(),
            metadata.modified().unwrap_or(SystemTime::UNIX_EPOCH),
        ));
    }
    let mut bytes = 0u64;
    let mut modified = metadata.modified().unwrap_or(SystemTime::UNIX_EPOCH);
    for entry in WalkDir::new(path).follow_links(false).min_depth(1) {
        let entry = entry.map_err(|error| AppError::Io(error.into()))?;
        let metadata = entry
            .metadata()
            .map_err(|error| AppError::Io(error.into()))?;
        modified = modified.max(metadata.modified().unwrap_or(SystemTime::UNIX_EPOCH));
        if metadata.is_file() {
            bytes = bytes.saturating_add(metadata.len());
        }
    }
    Ok((bytes, modified))
}

fn remove_cache_entry(path: &Path) -> std::io::Result<()> {
    if path.is_dir() {
        fs::remove_dir_all(path)
    } else {
        fs::remove_file(path)
    }
}

#[cfg(test)]
mod tests {
    use std::{fs::File, io::Write};

    use tempfile::TempDir;

    use super::*;

    #[test]
    fn consumed_incoming_file_removes_only_its_transfer_directory() {
        let cache = TempDir::new().unwrap();
        prepare(cache.path()).unwrap();
        let first = share_inbox(cache.path()).join("first");
        let second = share_inbox(cache.path()).join("second");
        fs::create_dir_all(&first).unwrap();
        fs::create_dir_all(&second).unwrap();
        let consumed = first.join("note.md");
        fs::write(&consumed, b"note").unwrap();
        fs::write(second.join("keep.md"), b"keep").unwrap();

        consume_incoming_file(&consumed, cache.path());

        assert!(!first.exists());
        assert!(second.join("keep.md").exists());
    }

    #[test]
    fn cleanup_removes_expired_entries_and_enforces_the_total_budget() {
        let cache = TempDir::new().unwrap();
        prepare(cache.path()).unwrap();
        let expired = workspace_exports(cache.path()).join("expired.zip");
        let budget_old = workspace_exports(cache.path()).join("budget-old.zip");
        let budget_new = workspace_exports(cache.path()).join("budget-new.zip");
        fs::write(&expired, [0u8; 4]).unwrap();
        fs::write(&budget_old, [1u8; 8]).unwrap();
        fs::write(&budget_new, [2u8; 8]).unwrap();
        let now = SystemTime::now();
        for (path, modified) in [
            (&expired, now - CACHE_RETENTION - Duration::from_secs(1)),
            (&budget_old, now - Duration::from_secs(2)),
            (&budget_new, now - Duration::from_secs(1)),
        ] {
            File::options()
                .write(true)
                .open(path)
                .unwrap()
                .set_modified(modified)
                .unwrap();
        }

        cleanup_roots(
            &[share_inbox(cache.path()), workspace_exports(cache.path())],
            now,
            8,
        )
        .unwrap();

        assert!(!expired.exists());
        assert!(!budget_old.exists());
        assert!(budget_new.exists());
    }

    #[test]
    fn incoming_file_rejects_paths_outside_the_managed_inbox() {
        let cache = TempDir::new().unwrap();
        prepare(cache.path()).unwrap();
        let outside = cache.path().join("outside.bin");
        let mut file = File::create(&outside).unwrap();
        file.write_all(b"outside").unwrap();

        assert!(incoming_file(&outside.to_string_lossy(), cache.path()).is_err());
    }
}
