use std::{fs, path::Path};

use sha2::{Digest, Sha256};

use crate::error::{AppError, AppResult};

pub fn hash_bytes(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
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
        if hash_bytes(&fs::read(path)?) != expected_sha256 {
            return Err(AppError::ExternalChange);
        }
    } else if expected_sha256.is_some() || !expected_missing {
        return Err(AppError::ExternalChange);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
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
