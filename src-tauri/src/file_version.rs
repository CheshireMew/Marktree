use std::{
    fs::{File, OpenOptions},
    io::{BufReader, Read, Seek, SeekFrom},
    path::Path,
};

#[cfg(target_os = "windows")]
use std::os::windows::fs::OpenOptionsExt;

use sha2::{Digest, Sha256};

use crate::error::{AppError, AppResult};

pub fn hash_bytes(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

pub fn hash_file(path: &Path) -> AppResult<String> {
    hash_open_file(File::open(path)?)
}

fn hash_open_file(file: File) -> AppResult<String> {
    let mut reader = BufReader::new(file);
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 64 * 1024];
    loop {
        let count = reader.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        hasher.update(&buffer[..count]);
    }
    Ok(hex::encode(hasher.finalize()))
}

/// Keeps the exact file version used by an optimistic write protected while it
/// is checked. On Windows the handle denies concurrent writers. The publishing
/// layer then releases it into a replace-with-backup transaction and rolls back
/// if a different version won that release boundary.
pub struct FileVersionGuard {
    file: Option<File>,
}

impl FileVersionGuard {
    pub fn existing(&self) -> bool {
        self.file.is_some()
    }

    pub fn read_bytes(&self) -> AppResult<Vec<u8>> {
        let file = self.file.as_ref().ok_or(AppError::ExternalChange)?;
        let mut file = file.try_clone()?;
        file.seek(SeekFrom::Start(0))?;
        let mut bytes = Vec::new();
        file.read_to_end(&mut bytes)?;
        Ok(bytes)
    }
}

pub fn guard_expected_version(
    path: &Path,
    expected_sha256: Option<&str>,
    expected_missing: bool,
) -> AppResult<FileVersionGuard> {
    match open_version_guard(path) {
        Ok(file) => {
            if expected_missing {
                return Err(AppError::ExternalChange);
            }
            let expected_sha256 = expected_sha256.ok_or(AppError::ExternalChange)?;
            if hash_open_file(file.try_clone()?)? != expected_sha256 {
                return Err(AppError::ExternalChange);
            }
            Ok(FileVersionGuard { file: Some(file) })
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            if expected_sha256.is_some() || !expected_missing {
                Err(AppError::ExternalChange)
            } else {
                Ok(FileVersionGuard { file: None })
            }
        }
        #[cfg(target_os = "windows")]
        Err(error) if matches!(error.raw_os_error(), Some(32) | Some(33)) => {
            Err(AppError::ExternalChange)
        }
        Err(error) => Err(error.into()),
    }
}

#[cfg(target_os = "windows")]
fn open_version_guard(path: &Path) -> std::io::Result<File> {
    use windows_sys::Win32::Storage::FileSystem::{FILE_SHARE_DELETE, FILE_SHARE_READ};

    OpenOptions::new()
        .read(true)
        .share_mode(FILE_SHARE_READ | FILE_SHARE_DELETE)
        .open(path)
}

#[cfg(not(target_os = "windows"))]
fn open_version_guard(path: &Path) -> std::io::Result<File> {
    OpenOptions::new().read(true).open(path)
}

pub fn verify_expected_version(
    path: &Path,
    expected_sha256: Option<&str>,
    expected_missing: bool,
) -> AppResult<()> {
    if path.exists() {
        if expected_missing {
            return Err(AppError::ExternalChange);
        }
        let expected_sha256 = expected_sha256.ok_or(AppError::ExternalChange)?;
        if hash_file(path)? != expected_sha256 {
            return Err(AppError::ExternalChange);
        }
    } else if expected_sha256.is_some() || !expected_missing {
        return Err(AppError::ExternalChange);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;
    use tempfile::TempDir;

    #[test]
    fn expected_version_covers_existing_missing_and_reappeared_files() {
        let directory = TempDir::new().unwrap();
        let path = directory.path().join("document.md");

        assert!(verify_expected_version(&path, None, true).is_ok());
        assert!(verify_expected_version(&path, None, false).is_err());

        fs::write(&path, b"current").unwrap();
        let current = hash_bytes(b"current");
        assert!(verify_expected_version(&path, Some(&current), false).is_ok());
        assert!(verify_expected_version(&path, None, true).is_err());
        assert!(verify_expected_version(&path, Some(&hash_bytes(b"stale")), false).is_err());
    }
}
