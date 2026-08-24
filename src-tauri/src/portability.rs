use std::path::Path;

#[cfg(test)]
use std::fs;

use crate::{
    android_bridge::IncomingShare,
    error::{AppError, AppResult},
    paths::{canonical_root, normalize_content_relative, path_to_slashes, resolve_existing_entry},
    state::{PersistentState, WorkspaceRuntime},
    transfer_cache,
    types::{
        AndroidShareImportResult, AndroidShareKind, ImportAndroidShareRequest, PendingAndroidShare,
        SaveDocumentRequest, TextEncoding, WorkspaceDescriptor,
    },
    workspace_service::WorkspaceService,
};

pub(crate) fn describe_incoming_share(share: IncomingShare) -> PendingAndroidShare {
    let kind = if share
        .file_name
        .as_deref()
        .is_some_and(|name| name.to_lowercase().ends_with(".zip"))
        || share.media_type.as_deref().is_some_and(|media_type| {
            matches!(
                media_type,
                "application/zip" | "application/x-zip-compressed"
            )
        }) {
        AndroidShareKind::Archive
    } else if share.file_path.is_none() {
        AndroidShareKind::Text
    } else if is_image_share(
        share.file_name.as_deref().unwrap_or_default(),
        share.media_type.as_deref(),
    ) {
        AndroidShareKind::Image
    } else if is_markdown_share(
        share.file_name.as_deref().unwrap_or_default(),
        share.media_type.as_deref(),
    ) {
        AndroidShareKind::Markdown
    } else {
        AndroidShareKind::Attachment
    };
    PendingAndroidShare {
        text: share.text,
        subject: share.subject,
        file_path: share.file_path,
        file_name: share.file_name,
        media_type: share.media_type,
        kind,
    }
}

pub(crate) fn import_incoming_share(
    request: ImportAndroidShareRequest,
    app_data_dir: &Path,
    app_cache_dir: &Path,
    state: &PersistentState,
    runtime: &WorkspaceRuntime,
) -> AppResult<AndroidShareImportResult> {
    let ImportAndroidShareRequest {
        share,
        root: requested_root,
        target_directory,
        document_path,
    } = request;
    let service = WorkspaceService::new(state, runtime);
    let incoming_path = share
        .file_path
        .as_deref()
        .map(|path| transfer_cache::incoming_file(path, app_cache_dir))
        .transpose()?;
    if share.kind == AndroidShareKind::Archive {
        let archive_path = incoming_path.as_ref().ok_or_else(|| {
            AppError::Message("The shared archive is no longer available.".to_owned())
        })?;
        let preferred_name = share.file_name.as_deref().unwrap_or("Imported workspace");
        let imported =
            service.import_workspace_archive(archive_path, app_data_dir, preferred_name)?;
        let result = AndroidShareImportResult {
            workspace: imported.workspace,
            open_path: None,
            insert_markdown: None,
            archive_imported: true,
        };
        transfer_cache::consume_incoming_file(archive_path, app_cache_dir);
        return Ok(result);
    }

    let workspace = share_workspace(requested_root.as_deref(), app_data_dir, &service)?;
    let root = workspace.root.clone();
    let target_directory = validated_target_directory(&root, &target_directory)?;
    if share.file_path.is_some() {
        let incoming_path = incoming_path.as_ref().ok_or_else(|| {
            AppError::Message("The shared file is no longer available.".to_owned())
        })?;
        let file_name = sanitize_file_name(
            share
                .file_name
                .as_deref()
                .or_else(|| incoming_path.file_name().and_then(|name| name.to_str()))
                .unwrap_or("shared-file"),
        );
        if let Some(document_path) = document_path
            .as_deref()
            .filter(|document| is_markdown_path(document))
            .filter(|_| share.kind == AndroidShareKind::Image)
        {
            let result = import_shared_image(
                &service,
                workspace,
                document_path,
                &file_name,
                incoming_path,
                share.text.as_deref(),
            )?;
            transfer_cache::consume_incoming_file(incoming_path, app_cache_dir);
            return Ok(result);
        }
        let path = unique_relative_file(&root, &target_directory, &file_name)?;
        service.import_file_from_path(&root, &path, incoming_path)?;
        let insert_markdown = document_path
            .as_deref()
            .filter(|document| is_markdown_path(document))
            .map(|document| {
                attachment_markdown(
                    document,
                    &path,
                    &file_name,
                    share.kind == AndroidShareKind::Image,
                )
            });
        let result = AndroidShareImportResult {
            workspace: service.inspect_workspace(&root)?,
            open_path: insert_markdown.is_none().then_some(path),
            insert_markdown,
            archive_imported: false,
        };
        transfer_cache::consume_incoming_file(incoming_path, app_cache_dir);
        return Ok(result);
    }

    let text = share
        .text
        .filter(|text| !text.trim().is_empty())
        .ok_or_else(|| AppError::Message("The shared content is empty.".to_owned()))?;
    if document_path.as_deref().is_some_and(is_markdown_path) {
        return Ok(AndroidShareImportResult {
            workspace: service.inspect_workspace(&root)?,
            open_path: None,
            insert_markdown: Some(text),
            archive_imported: false,
        });
    }
    let preferred = share
        .subject
        .as_deref()
        .filter(|subject| !subject.trim().is_empty())
        .map(|subject| format!("{}.md", sanitize_stem(subject)))
        .unwrap_or_else(timestamped_markdown_name);
    let path = unique_relative_file(&root, &target_directory, &preferred)?;
    let created = service.create_document(&root, &path)?;
    service.save_document(SaveDocumentRequest {
        root: root.clone(),
        path: path.clone(),
        content: text,
        expected_sha256: Some(created.sha256),
        expected_missing: false,
        encoding: TextEncoding::Utf8,
    })?;
    Ok(AndroidShareImportResult {
        workspace: service.inspect_workspace(&root)?,
        open_path: Some(path),
        insert_markdown: None,
        archive_imported: false,
    })
}

fn import_shared_image(
    service: &WorkspaceService<'_>,
    workspace: WorkspaceDescriptor,
    document_path: &str,
    file_name: &str,
    source_path: &Path,
    caption: Option<&str>,
) -> AppResult<AndroidShareImportResult> {
    let root = workspace.root.clone();
    let document_sha256 = service.read_document(&root, document_path)?.sha256;
    let asset = service.write_asset(
        &root,
        document_path,
        file_name,
        source_path,
        None,
        &document_sha256,
    )?;
    Ok(AndroidShareImportResult {
        workspace: service.inspect_workspace(&root)?,
        open_path: None,
        insert_markdown: Some(image_markdown(file_name, &asset.markdown_path, caption)),
        archive_imported: false,
    })
}

fn share_workspace(
    requested_root: Option<&str>,
    app_data_dir: &Path,
    service: &WorkspaceService<'_>,
) -> AppResult<WorkspaceDescriptor> {
    if let Some(root) = requested_root.filter(|root| !root.trim().is_empty()) {
        return service.inspect_workspace(root);
    }
    let root = app_data_dir.join("workspaces").join("Shared");
    if root.is_dir() {
        service.open_workspace(&root.to_string_lossy())
    } else {
        service.create_workspace(&root.to_string_lossy())
    }
}

fn unique_relative_file(
    root: &str,
    target_directory: &str,
    preferred_name: &str,
) -> AppResult<String> {
    let root_path = canonical_root(root)?;
    let file_name = sanitize_file_name(preferred_name);
    let path = Path::new(&file_name);
    let stem = path
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("Shared");
    let extension = path.extension().and_then(|value| value.to_str());
    for suffix in 1usize.. {
        let candidate = if suffix == 1 {
            file_name.clone()
        } else if let Some(extension) = extension {
            format!("{stem}-{suffix}.{extension}")
        } else {
            format!("{stem}-{suffix}")
        };
        let relative = if target_directory.is_empty() {
            candidate
        } else {
            format!("{target_directory}/{candidate}")
        };
        if !root_path.join(&relative).exists() {
            return Ok(relative);
        }
    }
    unreachable!()
}

fn validated_target_directory(root: &str, value: &str) -> AppResult<String> {
    if value.trim().is_empty() {
        return Ok(String::new());
    }
    let relative = normalize_content_relative(value)?;
    let absolute = resolve_existing_entry(&canonical_root(root)?, &relative)?;
    if !absolute.is_dir() {
        return Err(AppError::InvalidPath(value.to_owned()));
    }
    Ok(relative)
}

fn sanitize_file_name(value: &str) -> String {
    let normalized = value
        .trim()
        .chars()
        .map(|character| {
            if matches!(
                character,
                '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|'
            ) {
                '-'
            } else {
                character
            }
        })
        .collect::<String>()
        .trim_matches(|character: char| character == '.' || character.is_whitespace())
        .to_owned();
    if normalized.is_empty() {
        "shared-file".to_owned()
    } else {
        normalized
    }
}

fn sanitize_stem(value: &str) -> String {
    sanitize_file_name(value).trim_end_matches('.').to_owned()
}

fn timestamped_markdown_name() -> String {
    format!("Shared-{}.md", chrono::Local::now().format("%Y%m%d-%H%M%S"))
}

fn is_markdown_path(path: &str) -> bool {
    matches!(
        Path::new(path)
            .extension()
            .and_then(|extension| extension.to_str())
            .map(str::to_lowercase)
            .as_deref(),
        Some("md" | "markdown" | "mdx")
    )
}

fn is_markdown_share(file_name: &str, media_type: Option<&str>) -> bool {
    media_type == Some("text/markdown")
        || matches!(
            Path::new(file_name)
                .extension()
                .and_then(|extension| extension.to_str())
                .map(str::to_lowercase)
                .as_deref(),
            Some("md" | "markdown" | "mdx")
        )
}

fn attachment_markdown(
    document_path: &str,
    attachment_path: &str,
    file_name: &str,
    image: bool,
) -> String {
    let directory = Path::new(document_path)
        .parent()
        .unwrap_or_else(|| Path::new(""));
    let relative = pathdiff::diff_paths(Path::new(attachment_path), directory)
        .unwrap_or_else(|| Path::new(attachment_path).to_path_buf());
    let relative = path_to_slashes(&relative);
    let destination = if relative.chars().any(|character| character.is_whitespace()) {
        format!("<{relative}>")
    } else {
        relative
    };
    let label = Path::new(file_name)
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or(file_name)
        .replace(['[', ']'], "-");
    if image {
        format!("![{label}]({destination})")
    } else {
        format!("[{label}]({destination})")
    }
}

fn is_image_share(file_name: &str, media_type: Option<&str>) -> bool {
    media_type.is_some_and(|media_type| media_type.starts_with("image/"))
        || matches!(
            Path::new(file_name)
                .extension()
                .and_then(|extension| extension.to_str())
                .map(str::to_lowercase)
                .as_deref(),
            Some("png" | "jpg" | "jpeg" | "gif" | "webp" | "svg")
        )
}

fn image_markdown(file_name: &str, markdown_path: &str, caption: Option<&str>) -> String {
    let alt = Path::new(file_name)
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("image");
    match caption.filter(|caption| !caption.trim().is_empty()) {
        Some(caption) => format!("![{alt}]({markdown_path})\n\n{caption}"),
        None => format!("![{alt}]({markdown_path})"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn shared_text_uses_workspace_service_and_plain_workspaces_get_no_git_manifest() {
        let workspace = TempDir::new().unwrap();
        let app_data = TempDir::new().unwrap();
        let state = PersistentState::load(app_data.path()).unwrap();
        let runtime = WorkspaceRuntime::default();
        let service = WorkspaceService::new(&state, &runtime);
        let descriptor = service
            .create_workspace(workspace.path().to_str().unwrap())
            .unwrap();

        let result = import_incoming_share(
            ImportAndroidShareRequest {
                share: describe_incoming_share(IncomingShare {
                    text: Some("Shared line one\r\nline two".to_owned()),
                    subject: Some("Meeting note".to_owned()),
                    file_path: None,
                    file_name: None,
                    media_type: Some("text/plain".to_owned()),
                }),
                root: Some(descriptor.root.clone()),
                target_directory: String::new(),
                document_path: None,
            },
            app_data.path(),
            app_data.path(),
            &state,
            &runtime,
        )
        .unwrap();
        let path = result.open_path.unwrap();

        assert_eq!(
            fs::read(Path::new(&descriptor.root).join(path)).unwrap(),
            b"Shared line one\r\nline two"
        );
        assert!(state.workspace_changes(&descriptor.root).is_empty());
    }

    #[test]
    fn shared_text_in_a_real_git_root_becomes_an_exact_workspace_change() {
        let workspace = TempDir::new().unwrap();
        git2::Repository::init(workspace.path()).unwrap();
        let app_data = TempDir::new().unwrap();
        let state = PersistentState::load(app_data.path()).unwrap();
        let runtime = WorkspaceRuntime::default();
        let descriptor = WorkspaceService::new(&state, &runtime)
            .open_workspace(workspace.path().to_str().unwrap())
            .unwrap();

        let result = import_incoming_share(
            ImportAndroidShareRequest {
                share: describe_incoming_share(IncomingShare {
                    text: Some("Git share".to_owned()),
                    subject: Some("Inbox".to_owned()),
                    file_path: None,
                    file_name: None,
                    media_type: Some("text/plain".to_owned()),
                }),
                root: Some(descriptor.root.clone()),
                target_directory: String::new(),
                document_path: None,
            },
            app_data.path(),
            app_data.path(),
            &state,
            &runtime,
        )
        .unwrap();
        let path = result.open_path.unwrap();
        let changes = state.workspace_changes(&descriptor.root);

        assert_eq!(
            fs::read_to_string(workspace.path().join(&path)).unwrap(),
            "Git share"
        );
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].path, path);
        assert_eq!(
            changes[0].operation,
            crate::types::WorkspaceChangeOperation::Upsert
        );
    }

    #[test]
    fn shared_attachment_keeps_exact_bytes_and_returns_a_relative_markdown_link() {
        let workspace = TempDir::new().unwrap();
        let app_data = TempDir::new().unwrap();
        let share_inbox = app_data.path().join("share-inbox");
        let incoming_directory = share_inbox.join("incoming-1");
        fs::create_dir_all(&incoming_directory).unwrap();
        let incoming = incoming_directory.join("source data.bin");
        let bytes = b"\x00\x01shared\xffbytes";
        fs::write(&incoming, bytes).unwrap();
        let state = PersistentState::load(app_data.path()).unwrap();
        let runtime = WorkspaceRuntime::default();
        let service = WorkspaceService::new(&state, &runtime);
        let descriptor = service
            .create_workspace(workspace.path().to_str().unwrap())
            .unwrap();
        fs::create_dir(workspace.path().join("notes")).unwrap();
        fs::create_dir(workspace.path().join("attachments")).unwrap();
        service
            .create_document(&descriptor.root, "notes/current.md")
            .unwrap();

        let result = import_incoming_share(
            ImportAndroidShareRequest {
                share: describe_incoming_share(IncomingShare {
                    text: None,
                    subject: None,
                    file_path: Some(incoming.to_string_lossy().into_owned()),
                    file_name: Some("source data.bin".to_owned()),
                    media_type: Some("application/octet-stream".to_owned()),
                }),
                root: Some(descriptor.root.clone()),
                target_directory: "attachments".to_owned(),
                document_path: Some("notes/current.md".to_owned()),
            },
            app_data.path(),
            app_data.path(),
            &state,
            &runtime,
        )
        .unwrap();

        assert_eq!(
            fs::read(workspace.path().join("attachments/source data.bin")).unwrap(),
            bytes
        );
        assert_eq!(
            result.insert_markdown.as_deref(),
            Some("[source data](<../attachments/source data.bin>)")
        );
        assert!(result.open_path.is_none());
        assert!(state.workspace_changes(&descriptor.root).is_empty());
        assert!(!incoming_directory.exists());
    }

    #[test]
    fn shared_text_can_be_inserted_without_creating_a_second_content_file() {
        let workspace = TempDir::new().unwrap();
        let app_data = TempDir::new().unwrap();
        let state = PersistentState::load(app_data.path()).unwrap();
        let runtime = WorkspaceRuntime::default();
        let service = WorkspaceService::new(&state, &runtime);
        let descriptor = service
            .create_workspace(workspace.path().to_str().unwrap())
            .unwrap();
        service
            .create_document(&descriptor.root, "current.md")
            .unwrap();

        let result = import_incoming_share(
            ImportAndroidShareRequest {
                share: describe_incoming_share(IncomingShare {
                    text: Some("inserted text".to_owned()),
                    subject: Some("Ignored file name".to_owned()),
                    file_path: None,
                    file_name: None,
                    media_type: Some("text/plain".to_owned()),
                }),
                root: Some(descriptor.root.clone()),
                target_directory: String::new(),
                document_path: Some("current.md".to_owned()),
            },
            app_data.path(),
            app_data.path(),
            &state,
            &runtime,
        )
        .unwrap();

        assert_eq!(result.insert_markdown.as_deref(), Some("inserted text"));
        assert!(result.open_path.is_none());
        assert_eq!(fs::read_dir(workspace.path()).unwrap().count(), 1);
    }

    #[test]
    fn failed_shared_file_import_keeps_the_managed_input_for_retry() {
        let workspace = TempDir::new().unwrap();
        let app_data = TempDir::new().unwrap();
        transfer_cache::prepare(app_data.path()).unwrap();
        let incoming_directory = transfer_cache::share_inbox(app_data.path()).join("retry");
        fs::create_dir_all(&incoming_directory).unwrap();
        let incoming = incoming_directory.join("note.md");
        fs::write(&incoming, b"retry me").unwrap();
        let state = PersistentState::load(app_data.path()).unwrap();
        let runtime = WorkspaceRuntime::default();
        let descriptor = WorkspaceService::new(&state, &runtime)
            .create_workspace(workspace.path().to_str().unwrap())
            .unwrap();

        let result = import_incoming_share(
            ImportAndroidShareRequest {
                share: describe_incoming_share(IncomingShare {
                    text: None,
                    subject: None,
                    file_path: Some(incoming.to_string_lossy().into_owned()),
                    file_name: Some("note.md".to_owned()),
                    media_type: Some("text/markdown".to_owned()),
                }),
                root: Some(descriptor.root),
                target_directory: "missing-directory".to_owned(),
                document_path: None,
            },
            app_data.path(),
            app_data.path(),
            &state,
            &runtime,
        );

        assert!(result.is_err());
        assert_eq!(fs::read(&incoming).unwrap(), b"retry me");
    }
}
