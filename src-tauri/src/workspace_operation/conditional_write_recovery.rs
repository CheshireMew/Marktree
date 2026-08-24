use std::{fs, path::Path};

use crate::{error::AppResult, file_version::hash_file};

use super::{recovery_error, PendingWorkspaceOperation};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ConditionalWriteRecovery {
    NoArtifacts,
    Applied,
    Unapplied,
}

pub(super) fn reconcile_conditional_write_artifacts(
    operation: &PendingWorkspaceOperation,
    target: &Path,
    version: &str,
    previous_version: Option<&str>,
) -> AppResult<ConditionalWriteRecovery> {
    let (backup, rejected) = crate::paths::conditional_write_artifact_paths(target, &operation.id)?;
    let backup_exists = backup.is_file();
    let rejected_exists = rejected.is_file();
    if !backup_exists && !rejected_exists {
        return Ok(ConditionalWriteRecovery::NoArtifacts);
    }

    let target_version = target.is_file().then(|| hash_file(target)).transpose()?;
    if backup_exists {
        if rejected_exists {
            return Err(recovery_error(
                operation,
                "both conditional-write recovery files exist",
            ));
        }
        if target_version.as_deref() != Some(version) {
            return Err(recovery_error(
                operation,
                "the conditional-write backup exists but the saved file is not the expected version",
            ));
        }
        let Some(previous_version) = previous_version else {
            return Err(recovery_error(
                operation,
                "the conditional-write backup has no recorded previous version",
            ));
        };
        if hash_file(&backup)? == previous_version {
            fs::remove_file(&backup)?;
            return Ok(ConditionalWriteRecovery::Applied);
        }

        crate::paths::restore_conditional_write_backup(target, &backup, &rejected)?;
        if !target.is_file()
            || rejected
                .is_file()
                .then(|| hash_file(&rejected))
                .transpose()?
                .as_deref()
                != Some(version)
        {
            return Err(recovery_error(
                operation,
                "the conditional-write rollback could not be verified",
            ));
        }
        fs::remove_file(&rejected)?;
        return Ok(ConditionalWriteRecovery::Unapplied);
    }

    if target_version.is_none()
        || target_version.as_deref() == Some(version)
        || hash_file(&rejected)? != version
    {
        return Err(recovery_error(
            operation,
            "the conditional-write rollback state is ambiguous",
        ));
    }
    fs::remove_file(&rejected)?;
    Ok(ConditionalWriteRecovery::Unapplied)
}
