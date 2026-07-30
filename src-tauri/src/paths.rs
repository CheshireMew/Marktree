use std::{
    collections::BTreeSet,
    fs::{self, OpenOptions},
    io::Write,
    path::{Component, Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

use crate::error::{AppError, AppResult};

static TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(0);

pub fn canonical_root(root: &str) -> AppResult<PathBuf> {
    let path = fs::canonicalize(root)?;
    if !path.is_dir() {
        return Err(AppError::InvalidPath(root.to_owned()));
    }
    Ok(path)
}

pub fn normalize_relative(value: &str) -> AppResult<String> {
    let value = value.trim().replace('\\', "/");
    let path = Path::new(&value);
    if value.is_empty() || path.is_absolute() {
        return Err(AppError::InvalidPath(value));
    }

    let mut normalized = Vec::new();
    for component in path.components() {
        match component {
            Component::Normal(value) => normalized.push(value.to_string_lossy().into_owned()),
            Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                return Err(AppError::InvalidPath(value));
            }
        }
    }
    if normalized.is_empty()
        || normalized
            .first()
            .is_some_and(|part| part.eq_ignore_ascii_case(".git"))
    {
        return Err(AppError::InvalidPath(value));
    }
    Ok(normalized.join("/"))
}

pub fn normalize_relative_paths(paths: &[String]) -> AppResult<Vec<String>> {
    let mut normalized = BTreeSet::new();
    for path in paths {
        normalized.insert(normalize_relative(path)?);
    }
    Ok(normalized.into_iter().collect())
}

pub fn resolve_existing_file(root: &Path, relative: &str) -> AppResult<PathBuf> {
    let path = resolve_existing_entry(root, relative)?;
    if !path.is_file() {
        return Err(AppError::InvalidPath(path.display().to_string()));
    }
    Ok(path)
}

pub fn resolve_existing_entry(root: &Path, relative: &str) -> AppResult<PathBuf> {
    let relative = normalize_relative(relative)?;
    let path = fs::canonicalize(root.join(relative.replace('/', std::path::MAIN_SEPARATOR_STR)))
        .map_err(|error| {
            if error.kind() == std::io::ErrorKind::NotFound {
                AppError::FileNotFound {
                    path: relative.clone(),
                }
            } else {
                AppError::Io(error)
            }
        })?;
    if !path.starts_with(root) {
        return Err(AppError::InvalidPath(path.display().to_string()));
    }
    Ok(path)
}

pub fn resolve_for_write(root: &Path, relative: &str) -> AppResult<PathBuf> {
    let relative = normalize_relative(relative)?;
    let path = root.join(relative.replace('/', std::path::MAIN_SEPARATOR_STR));
    if path.exists() {
        let canonical = fs::canonicalize(&path)?;
        if !canonical.starts_with(root) {
            return Err(AppError::InvalidPath(relative));
        }
        return Ok(path);
    }

    let parent = path
        .parent()
        .ok_or_else(|| AppError::InvalidPath(relative.clone()))?;
    let mut existing = parent;
    while !existing.exists() {
        existing = existing
            .parent()
            .ok_or_else(|| AppError::InvalidPath(relative.clone()))?;
    }
    let canonical_parent = fs::canonicalize(existing)?;
    if !canonical_parent.starts_with(root) {
        return Err(AppError::InvalidPath(relative));
    }
    Ok(path)
}

pub fn path_to_slashes(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

pub fn paths_equal(left: &str, right: &str) -> bool {
    match (fs::canonicalize(left), fs::canonicalize(right)) {
        (Ok(left), Ok(right)) => left == right,
        _ if cfg!(target_os = "windows") => left.eq_ignore_ascii_case(right),
        _ => left == right,
    }
}

pub fn atomic_write(path: &Path, bytes: &[u8]) -> AppResult<()> {
    let parent = path
        .parent()
        .ok_or_else(|| AppError::InvalidPath(path.display().to_string()))?;
    fs::create_dir_all(parent)?;
    let sequence = TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let file_name = path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("marktree");
    let temporary = parent.join(format!(
        ".{file_name}.{}.{}.tmp",
        std::process::id(),
        sequence
    ));
    let result = (|| -> AppResult<()> {
        let mut file = OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&temporary)?;
        file.write_all(bytes)?;
        file.sync_all()?;
        drop(file);
        replace_file(&temporary, path)
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result?;
    if let Ok(directory) = OpenOptions::new().read(true).open(parent) {
        let _ = directory.sync_all();
    }
    Ok(())
}

pub fn atomic_create(path: &Path, bytes: &[u8]) -> AppResult<()> {
    let parent = path
        .parent()
        .ok_or_else(|| AppError::InvalidPath(path.display().to_string()))?;
    fs::create_dir_all(parent)?;
    let mut file = OpenOptions::new().write(true).create_new(true).open(path)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    if let Ok(directory) = OpenOptions::new().read(true).open(parent) {
        let _ = directory.sync_all();
    }
    Ok(())
}

#[cfg(target_os = "windows")]
fn replace_file(source: &Path, destination: &Path) -> AppResult<()> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Storage::FileSystem::{
        MoveFileExW, MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH,
    };

    let source: Vec<u16> = source.as_os_str().encode_wide().chain(Some(0)).collect();
    let destination: Vec<u16> = destination
        .as_os_str()
        .encode_wide()
        .chain(Some(0))
        .collect();
    let result = unsafe {
        MoveFileExW(
            source.as_ptr(),
            destination.as_ptr(),
            MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
        )
    };
    if result == 0 {
        return Err(std::io::Error::last_os_error().into());
    }
    Ok(())
}

#[cfg(not(target_os = "windows"))]
fn replace_file(source: &Path, destination: &Path) -> AppResult<()> {
    fs::rename(source, destination)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn one_path_boundary_rejects_parent_absolute_and_git_paths() {
        assert_eq!(
            normalize_relative(r"docs\hello.md").unwrap(),
            "docs/hello.md"
        );
        assert!(normalize_relative("../secret.md").is_err());
        assert!(normalize_relative("C:\\secret.md").is_err());
        assert!(normalize_relative(".git/config").is_err());
        assert!(normalize_relative(".GIT/config").is_err());
    }

    #[test]
    fn atomic_write_replaces_the_complete_file() {
        let directory = TempDir::new().unwrap();
        let path = directory.path().join("state.json");
        atomic_write(&path, b"first").unwrap();
        atomic_write(&path, b"second").unwrap();
        assert_eq!(fs::read(path).unwrap(), b"second");
    }

    #[test]
    fn failed_atomic_replace_does_not_leave_a_temporary_file() {
        let directory = TempDir::new().unwrap();
        let destination = directory.path().join("occupied");
        fs::create_dir(&destination).unwrap();

        assert!(atomic_write(&destination, b"content").is_err());
        let remaining = fs::read_dir(directory.path())
            .unwrap()
            .map(|entry| entry.unwrap().file_name())
            .collect::<Vec<_>>();

        assert_eq!(remaining, vec![destination.file_name().unwrap()]);
    }

    #[test]
    fn atomic_create_never_replaces_an_existing_file() {
        let directory = TempDir::new().unwrap();
        let path = directory.path().join("document.md");
        atomic_create(&path, b"first").unwrap();

        assert!(atomic_create(&path, b"second").is_err());
        assert_eq!(fs::read(path).unwrap(), b"first");
    }
}
