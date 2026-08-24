use std::{
    collections::BTreeSet,
    fs::{self, File, OpenOptions},
    io::{self, Write},
    path::{Component, Path, PathBuf},
};

use walkdir::WalkDir;
use zip::{write::SimpleFileOptions, CompressionMethod, ZipArchive, ZipWriter};

use crate::{
    content_policy::{
        is_portable_workspace_path, portable_workspace_path_violation, PortablePathViolation,
    },
    error::{AppError, AppResult},
    paths::{canonical_root, path_to_slashes},
    state::PersistentState,
    types::{WorkspaceArchiveExportResult, WorkspaceArchiveImportResult},
    workspace,
};

const MAX_ARCHIVE_ENTRIES: usize = 20_000;
const MAX_UNCOMPRESSED_BYTES: u64 = 2 * 1024 * 1024 * 1024;
const STALE_STAGING_AGE: std::time::Duration = std::time::Duration::from_secs(24 * 60 * 60);

struct StagingDirectory {
    path: PathBuf,
    published: bool,
}

impl StagingDirectory {
    fn create(path: PathBuf) -> AppResult<Self> {
        fs::create_dir_all(&path)?;
        Ok(Self {
            path,
            published: false,
        })
    }

    fn path(&self) -> &Path {
        &self.path
    }

    fn publish(mut self, destination: &Path) -> AppResult<()> {
        fs::rename(&self.path, destination)?;
        self.published = true;
        Ok(())
    }
}

impl Drop for StagingDirectory {
    fn drop(&mut self) {
        if !self.published {
            let _ = fs::remove_dir_all(&self.path);
        }
    }
}

pub(crate) fn cleanup_stale_import_staging(app_data_dir: &Path) -> AppResult<()> {
    let root = app_data_dir.join("import-staging");
    let entries = match fs::read_dir(&root) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error.into()),
    };
    let now_micros = chrono::Utc::now().timestamp_micros();
    let stale_micros = STALE_STAGING_AGE.as_micros().min(i64::MAX as u128) as i64;
    for entry in entries {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().into_owned();
        let Some(created_micros) = owned_staging_timestamp(&name) else {
            continue;
        };
        if !entry.file_type()?.is_dir() {
            continue;
        }
        if now_micros.saturating_sub(created_micros) >= stale_micros {
            fs::remove_dir_all(entry.path())?;
        }
    }
    Ok(())
}

fn owned_staging_timestamp(name: &str) -> Option<i64> {
    let mut parts = name.split('-');
    let (Some(timestamp), Some(pid), Some(suffix), None) =
        (parts.next(), parts.next(), parts.next(), parts.next())
    else {
        return None;
    };
    pid.parse::<u32>().ok()?;
    suffix.parse::<usize>().ok()?;
    timestamp.parse::<i64>().ok()
}

pub(crate) fn export_workspace(
    root: &str,
    output_path: &Path,
) -> AppResult<WorkspaceArchiveExportResult> {
    let root_path = canonical_root(root)?;
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent)?;
    }
    let output = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(output_path)?;
    let mut writer = ZipWriter::new(output);
    let file_options = SimpleFileOptions::default()
        .compression_method(CompressionMethod::Deflated)
        .unix_permissions(0o644);
    let directory_options = SimpleFileOptions::default()
        .compression_method(CompressionMethod::Stored)
        .unix_permissions(0o755);
    let mut file_count = 0usize;
    let mut total_bytes = 0u64;

    for entry in WalkDir::new(&root_path).follow_links(false) {
        let entry = entry.map_err(|error| AppError::Io(error.into()))?;
        if entry.depth() == 0 {
            continue;
        }
        let relative = entry
            .path()
            .strip_prefix(&root_path)
            .map_err(|_| AppError::InvalidPath(entry.path().display().to_string()))?;
        if !is_portable_workspace_path(&path_to_slashes(relative), entry.file_type().is_dir()) {
            continue;
        }
        if entry.file_type().is_symlink() {
            return Err(AppError::Message(format!(
                "Workspace archives do not support symbolic links: {}",
                relative.display()
            )));
        }
        let name = path_to_slashes(relative);
        if entry.file_type().is_dir() {
            writer
                .add_directory(
                    format!("{}/", name.trim_end_matches('/')),
                    directory_options,
                )
                .map_err(zip_error)?;
            continue;
        }
        if !entry.file_type().is_file() {
            continue;
        }
        let metadata = entry
            .metadata()
            .map_err(|error| AppError::Io(error.into()))?;
        total_bytes = total_bytes.saturating_add(metadata.len());
        writer.start_file(name, file_options).map_err(zip_error)?;
        let mut input = File::open(entry.path())?;
        io::copy(&mut input, &mut writer)?;
        file_count += 1;
    }
    writer.finish().map_err(zip_error)?;
    Ok(WorkspaceArchiveExportResult {
        file_count,
        total_bytes,
    })
}

pub(crate) fn import_workspace(
    archive_path: &Path,
    app_data_dir: &Path,
    preferred_name: &str,
    state: &PersistentState,
) -> AppResult<WorkspaceArchiveImportResult> {
    let mut archive = ZipArchive::new(File::open(archive_path)?).map_err(zip_error)?;
    if archive.is_empty() {
        return Err(AppError::Message(
            "The workspace archive is empty.".to_owned(),
        ));
    }
    if archive.len() > MAX_ARCHIVE_ENTRIES {
        return Err(AppError::Message(
            "The workspace archive contains too many entries.".to_owned(),
        ));
    }

    let mut entries = Vec::with_capacity(archive.len());
    let mut total_bytes = 0u64;
    for index in 0..archive.len() {
        let file = archive.by_index(index).map_err(zip_error)?;
        let path = safe_archive_path(&file)?;
        if path.as_os_str().is_empty() {
            continue;
        }
        validate_import_path(&path, file.is_dir())?;
        total_bytes = total_bytes.saturating_add(file.size());
        if total_bytes > MAX_UNCOMPRESSED_BYTES {
            return Err(AppError::Message(
                "The workspace archive is too large.".to_owned(),
            ));
        }
        entries.push((index, path, file.is_dir(), file.size()));
    }

    let common_root = common_archive_root(&entries);
    let mut prepared = Vec::with_capacity(entries.len());
    let mut written = BTreeSet::new();
    for (index, stored_path, is_directory, size) in entries {
        let relative = common_root
            .as_ref()
            .and_then(|root| stored_path.strip_prefix(root).ok())
            .unwrap_or(&stored_path)
            .to_path_buf();
        if relative.as_os_str().is_empty() {
            continue;
        }
        validate_import_path(&relative, is_directory)?;
        let relative_key = path_to_slashes(&relative);
        if !written.insert(relative_key.clone()) {
            return Err(AppError::Message(format!(
                "The workspace archive contains a duplicate path: {relative_key}"
            )));
        }
        prepared.push((index, relative, is_directory, size));
    }
    let destination = unique_workspace_destination(app_data_dir, preferred_name);
    let staging = StagingDirectory::create(unique_staging_directory(app_data_dir))?;
    let mut file_count = 0usize;
    for (index, relative, is_directory, _size) in prepared {
        let target = staging.path().join(&relative);
        if is_directory {
            fs::create_dir_all(&target)?;
            continue;
        }
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut source = archive.by_index(index).map_err(zip_error)?;
        let mut output = OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&target)?;
        io::copy(&mut source, &mut output)?;
        output.flush()?;
        file_count += 1;
    }
    if file_count == 0 {
        return Err(AppError::Message(
            "The workspace archive does not contain any files.".to_owned(),
        ));
    }
    fs::create_dir_all(destination.parent().unwrap_or(app_data_dir))?;
    staging.publish(&destination)?;
    let descriptor = workspace::open_workspace(&destination.to_string_lossy(), state)?;
    Ok(WorkspaceArchiveImportResult {
        workspace: descriptor,
        file_count,
        total_bytes,
    })
}

fn safe_archive_path(file: &zip::read::ZipFile<'_, File>) -> AppResult<PathBuf> {
    if file
        .unix_mode()
        .is_some_and(|mode| mode & 0o170000 == 0o120000)
    {
        return Err(AppError::Message(
            "Workspace archives do not support symbolic links.".to_owned(),
        ));
    }
    let path = file
        .enclosed_name()
        .ok_or_else(|| AppError::InvalidPath(file.name().to_owned()))?;
    if path
        .components()
        .any(|component| !matches!(component, Component::Normal(_) | Component::CurDir))
    {
        return Err(AppError::InvalidPath(file.name().to_owned()));
    }
    Ok(path.to_path_buf())
}

fn common_archive_root(entries: &[(usize, PathBuf, bool, u64)]) -> Option<PathBuf> {
    let first = entries.first()?.1.components().next()?;
    let Component::Normal(first_name) = first else {
        return None;
    };
    if first_name.to_string_lossy().starts_with('.') {
        return None;
    }
    if entries.iter().all(|(_, path, _, _)| {
        path.components().next().is_some_and(
            |component| matches!(component, Component::Normal(name) if name == first_name),
        )
    }) && entries
        .iter()
        .any(|(_, path, _, _)| path.components().count() > 1)
    {
        Some(PathBuf::from(first_name))
    } else {
        None
    }
}

fn validate_import_path(path: &Path, is_directory: bool) -> AppResult<()> {
    let relative = path_to_slashes(path);
    match portable_workspace_path_violation(&relative, is_directory) {
        Some(PortablePathViolation::GitMetadata) => {
            return Err(AppError::Message(
                "A plain workspace archive cannot contain Git metadata.".to_owned(),
            ));
        }
        Some(
            PortablePathViolation::MarktreeInternal | PortablePathViolation::TransactionArtifact,
        ) => {
            return Err(AppError::Message(
                "The archive contains unsupported Marktree internal data.".to_owned(),
            ));
        }
        None => {}
    }
    Ok(())
}

fn unique_workspace_destination(app_data_dir: &Path, preferred_name: &str) -> PathBuf {
    let base = sanitize_workspace_name(preferred_name);
    let workspaces = app_data_dir.join("workspaces");
    for suffix in 1usize.. {
        let name = if suffix == 1 {
            base.clone()
        } else {
            format!("{base}-{suffix}")
        };
        let candidate = workspaces.join(name);
        if !candidate.exists() {
            return candidate;
        }
    }
    unreachable!()
}

fn unique_staging_directory(app_data_dir: &Path) -> PathBuf {
    let base = format!(
        "{}-{}",
        chrono::Utc::now().timestamp_micros(),
        std::process::id()
    );
    let root = app_data_dir.join("import-staging");
    for suffix in 1usize.. {
        let candidate = root.join(format!("{base}-{suffix}"));
        if !candidate.exists() {
            return candidate;
        }
    }
    unreachable!()
}

fn sanitize_workspace_name(value: &str) -> String {
    let normalized = value
        .trim()
        .trim_end_matches(".zip")
        .chars()
        .map(|character| {
            if character.is_alphanumeric() || matches!(character, '-' | '_' | ' ') {
                character
            } else {
                '-'
            }
        })
        .collect::<String>()
        .trim_matches(|character: char| character == '-' || character.is_whitespace())
        .to_owned();
    if normalized.is_empty() {
        "Imported workspace".to_owned()
    } else {
        normalized
    }
}

fn zip_error(error: zip::result::ZipError) -> AppError {
    AppError::Message(format!("Workspace archive error: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn ordinary_workspace_round_trip_preserves_files_config_and_empty_directories() {
        let source = TempDir::new().unwrap();
        fs::create_dir_all(source.path().join("notes/empty")).unwrap();
        fs::create_dir_all(source.path().join("assets")).unwrap();
        fs::create_dir_all(source.path().join(".marktree")).unwrap();
        fs::create_dir_all(source.path().join(".git")).unwrap();
        fs::write(
            source.path().join("notes/readme.md"),
            b"# Notes\r\n\r\nExact bytes\r\n",
        )
        .unwrap();
        fs::write(source.path().join("assets/pixel.png"), [0, 1, 2, 3]).unwrap();
        fs::write(
            source.path().join(".marktree/config.json"),
            br#"{"assetsDir":"assets","ignoreRules":[]}"#,
        )
        .unwrap();
        fs::write(source.path().join(".git/config"), b"must not travel").unwrap();
        let archive_dir = TempDir::new().unwrap();
        let archive_path = archive_dir.path().join("notes.zip");

        let exported = export_workspace(source.path().to_str().unwrap(), &archive_path).unwrap();
        assert_eq!(exported.file_count, 3);

        let app_data = TempDir::new().unwrap();
        let state = PersistentState::load(app_data.path()).unwrap();
        let imported =
            import_workspace(&archive_path, app_data.path(), "Portable", &state).unwrap();
        let destination = Path::new(&imported.workspace.root);

        assert_eq!(
            fs::read(destination.join("notes/readme.md")).unwrap(),
            b"# Notes\r\n\r\nExact bytes\r\n"
        );
        assert_eq!(
            fs::read(destination.join("assets/pixel.png")).unwrap(),
            [0, 1, 2, 3]
        );
        assert!(destination.join("notes/empty").is_dir());
        assert!(destination.join(".marktree/config.json").is_file());
        assert!(!destination.join(".git").exists());
        assert!(imported.workspace.git.is_none());
        assert!(state.workspace_changes(&imported.workspace.root).is_empty());
    }

    #[test]
    fn export_excludes_operation_owned_transaction_artifacts() {
        let source = TempDir::new().unwrap();
        fs::create_dir_all(source.path().join("notes")).unwrap();
        fs::write(source.path().join("notes/day.md"), b"# Day\n").unwrap();
        for suffix in ["tmp", "previous", "rejected"] {
            fs::write(
                source.path().join(format!(
                    "notes/.day.md.marktree-0123456789abcdef01234567.{suffix}"
                )),
                b"internal operation state",
            )
            .unwrap();
        }
        let archive_dir = TempDir::new().unwrap();
        let archive_path = archive_dir.path().join("notes.zip");

        let exported = export_workspace(source.path().to_str().unwrap(), &archive_path).unwrap();

        assert_eq!(exported.file_count, 1);
        let mut archive = ZipArchive::new(File::open(archive_path).unwrap()).unwrap();
        let files = (0..archive.len())
            .filter_map(|index| {
                let file = archive.by_index(index).unwrap();
                (!file.is_dir()).then(|| file.name().to_owned())
            })
            .collect::<Vec<_>>();
        assert_eq!(files, vec!["notes/day.md"]);
    }

    #[test]
    fn wrapped_git_metadata_is_rejected_before_a_workspace_is_created() {
        let archive_dir = TempDir::new().unwrap();
        let archive_path = archive_dir.path().join("bad.zip");
        let mut writer = ZipWriter::new(File::create(&archive_path).unwrap());
        writer
            .start_file("wrapped/.git/config", SimpleFileOptions::default())
            .unwrap();
        writer.write_all(b"gitdir").unwrap();
        writer.finish().unwrap();
        let app_data = TempDir::new().unwrap();
        let state = PersistentState::load(app_data.path()).unwrap();

        let error = import_workspace(&archive_path, app_data.path(), "Bad", &state).unwrap_err();

        assert!(error.to_string().contains("Git metadata"));
        assert!(!app_data.path().join("workspaces/Bad").exists());
    }

    #[test]
    fn failed_import_removes_its_operation_owned_staging_directory() {
        let archive_dir = TempDir::new().unwrap();
        let archive_path = archive_dir.path().join("empty-folders.zip");
        let mut writer = ZipWriter::new(File::create(&archive_path).unwrap());
        writer
            .add_directory("notes/", SimpleFileOptions::default())
            .unwrap();
        writer.finish().unwrap();
        let app_data = TempDir::new().unwrap();
        let state = PersistentState::load(app_data.path()).unwrap();

        let error = import_workspace(&archive_path, app_data.path(), "Empty", &state).unwrap_err();

        assert!(error.to_string().contains("does not contain any files"));
        let staging_root = app_data.path().join("import-staging");
        assert!(!staging_root.exists() || fs::read_dir(staging_root).unwrap().next().is_none());
    }

    #[test]
    fn startup_cleanup_removes_only_stale_owned_import_staging() {
        let app_data = TempDir::new().unwrap();
        let staging = app_data.path().join("import-staging");
        let stale = staging.join("1-123-1");
        let fresh = staging.join(format!("{}-123-1", chrono::Utc::now().timestamp_micros()));
        let foreign = staging.join("keep-me");
        fs::create_dir_all(&stale).unwrap();
        fs::create_dir_all(&fresh).unwrap();
        fs::create_dir_all(&foreign).unwrap();

        cleanup_stale_import_staging(app_data.path()).unwrap();

        assert!(!stale.exists());
        assert!(fresh.exists());
        assert!(foreign.exists());
    }
}
