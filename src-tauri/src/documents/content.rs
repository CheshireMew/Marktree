use std::{fs, path::Path, time::UNIX_EPOCH};

use crate::{
    content_policy::document_kind,
    error::{AppError, AppResult},
    file_version::{hash_bytes, hash_file, verify_expected_version},
    paths::{
        atomic_copy_if_version, atomic_write_if_version, canonical_root,
        normalize_content_relative, resolve_existing_file, resolve_for_write,
    },
    state::PersistentState,
    types::{
        DocumentContent, DocumentKind, LineEnding, SaveDocumentRequest, SaveDocumentResult,
        TextEncoding,
    },
    workspace_operation::{execute_mutation, WorkspaceChangeIntent, WorkspaceOperationKind},
};

pub const MAX_EDITABLE_DOCUMENT_BYTES: u64 = 32 * 1024 * 1024;

fn ensure_editable_size(bytes: u64) -> AppResult<()> {
    if bytes > MAX_EDITABLE_DOCUMENT_BYTES {
        return Err(AppError::Message(format!(
            "This text file is too large to edit in Marktree (maximum {} MiB).",
            MAX_EDITABLE_DOCUMENT_BYTES / 1024 / 1024
        )));
    }
    Ok(())
}

pub fn read_document(root: &str, path: &str) -> AppResult<DocumentContent> {
    let root_path = canonical_root(root)?;
    let relative = normalize_content_relative(path)?;
    let file_path = resolve_existing_file(&root_path, &relative)?;
    let kind = document_kind(&file_path);
    if !matches!(kind, DocumentKind::Markdown | DocumentKind::Text) {
        return Err(AppError::Message(
            "This file type cannot be opened as text.".to_owned(),
        ));
    }
    let metadata = fs::metadata(&file_path)?;
    ensure_editable_size(metadata.len())?;
    let bytes = fs::read(&file_path)?;
    let encoding = text_encoding(&bytes);
    let text_bytes = strip_utf8_bom(&bytes);
    let content = String::from_utf8(text_bytes.to_vec())
        .map_err(|_| AppError::Message("The file is not valid UTF-8 text.".to_owned()))?;
    Ok(DocumentContent {
        path: relative,
        content,
        modified_ms: modified_ms(&metadata),
        sha256: hash_bytes(&bytes),
        read_only: encoding == TextEncoding::Unsupported,
        encoding,
        line_ending: detect_line_ending(text_bytes),
    })
}

pub fn open_document(
    root: &str,
    path: &str,
    app_state: &PersistentState,
) -> AppResult<DocumentContent> {
    let content = read_document(root, path)?;
    let _ = app_state.remember_file(root, &content.path);
    Ok(content)
}

pub fn save_document(
    request: SaveDocumentRequest,
    app_state: &PersistentState,
) -> AppResult<SaveDocumentResult> {
    validate_save_document(&request)?;
    let root_path = canonical_root(&request.root)?;
    let relative = normalize_content_relative(&request.path)?;
    let file_path = resolve_for_write(&root_path, &relative)?;
    let bytes = encode_text(&request.content, request.encoding);
    let sha256 = hash_bytes(&bytes);
    execute_mutation(
        &request.root,
        WorkspaceOperationKind::WriteFile {
            path: relative.clone(),
            version: sha256.clone(),
            previous_version: request.expected_sha256.clone(),
            replace_existing: false,
        },
        vec![WorkspaceChangeIntent::upsert(&relative, &sha256)],
        app_state,
        (),
        |operation| {
            atomic_write_if_version(
                &file_path,
                &bytes,
                request.expected_sha256.as_deref(),
                request.expected_missing,
                &operation.id,
            )
        },
    )?;
    let metadata = fs::metadata(&file_path)?;
    let _ = app_state.remember_file(&request.root, &relative);
    Ok(SaveDocumentResult {
        path: relative,
        modified_ms: modified_ms(&metadata),
        sha256,
        encoding: request.encoding,
        line_ending: detect_line_ending(request.content.as_bytes()),
    })
}

pub fn validate_save_document(request: &SaveDocumentRequest) -> AppResult<()> {
    ensure_editable_size(request.content.len() as u64)?;
    let root_path = canonical_root(&request.root)?;
    let relative = normalize_content_relative(&request.path)?;
    let file_path = resolve_for_write(&root_path, &relative)?;
    if !matches!(
        document_kind(&file_path),
        DocumentKind::Markdown | DocumentKind::Text
    ) {
        return Err(AppError::Message(
            "Only Markdown and supported plain-text files can be edited.".to_owned(),
        ));
    }
    verify_expected_version(
        &file_path,
        request.expected_sha256.as_deref(),
        request.expected_missing,
    )?;
    if request.encoding == TextEncoding::Unsupported {
        return Err(AppError::Message(
            "Unsupported text encodings cannot be saved.".to_owned(),
        ));
    }
    Ok(())
}

pub fn create_document(
    root: &str,
    path: &str,
    app_state: &PersistentState,
) -> AppResult<DocumentContent> {
    let (relative, file_path) = prepare_new_file_destination(root, path)?;
    if !matches!(
        document_kind(&file_path),
        DocumentKind::Markdown | DocumentKind::Text
    ) {
        return Err(AppError::Message(
            "New editable files must use a supported Markdown or text extension.".to_owned(),
        ));
    }
    let sha256 = hash_bytes(b"");
    execute_new_file_mutation(root, &relative, &sha256, app_state, |operation_id| {
        atomic_write_if_version(&file_path, b"", None, true, operation_id)
    })?;
    read_document(root, &relative)
}

pub fn import_file_from_path(
    root: &str,
    path: &str,
    source_path: &Path,
    app_state: &PersistentState,
) -> AppResult<String> {
    if !source_path.is_file() {
        return Err(AppError::Message(
            "The imported file is no longer available.".to_owned(),
        ));
    }
    let (relative, file_path) = prepare_new_file_destination(root, path)?;
    let sha256 = hash_file(source_path)?;
    execute_new_file_mutation(root, &relative, &sha256, app_state, |operation_id| {
        atomic_copy_if_version(source_path, &file_path, None, true, false, operation_id)
    })?;
    Ok(relative)
}

fn prepare_new_file_destination(root: &str, path: &str) -> AppResult<(String, std::path::PathBuf)> {
    let root_path = canonical_root(root)?;
    let relative = normalize_content_relative(path)?;
    let file_path = resolve_for_write(&root_path, &relative)?;
    if file_path.exists() {
        return Err(AppError::Message(
            "A file already exists at that path.".to_owned(),
        ));
    }
    Ok((relative, file_path))
}

fn execute_new_file_mutation(
    root: &str,
    relative: &str,
    sha256: &str,
    app_state: &PersistentState,
    write: impl FnOnce(&str) -> AppResult<()>,
) -> AppResult<()> {
    execute_mutation(
        root,
        WorkspaceOperationKind::WriteFile {
            path: relative.to_owned(),
            version: sha256.to_owned(),
            previous_version: None,
            replace_existing: false,
        },
        vec![WorkspaceChangeIntent::upsert(relative, sha256)],
        app_state,
        (),
        |operation| write(&operation.id),
    )
}

pub fn read_text_at_root(root: &str, path: &str) -> AppResult<String> {
    Ok(read_document(root, path)?.content)
}

pub(super) fn modified_ms(metadata: &fs::Metadata) -> u64 {
    metadata
        .modified()
        .ok()
        .and_then(|value| value.duration_since(UNIX_EPOCH).ok())
        .map(|value| value.as_millis() as u64)
        .unwrap_or_default()
}

fn text_encoding(bytes: &[u8]) -> TextEncoding {
    if bytes.starts_with(&[0xef, 0xbb, 0xbf]) && std::str::from_utf8(&bytes[3..]).is_ok() {
        TextEncoding::Utf8Bom
    } else if std::str::from_utf8(bytes).is_ok() {
        TextEncoding::Utf8
    } else {
        TextEncoding::Unsupported
    }
}

fn strip_utf8_bom(bytes: &[u8]) -> &[u8] {
    bytes.strip_prefix(&[0xef, 0xbb, 0xbf]).unwrap_or(bytes)
}

fn encode_text(content: &str, encoding: TextEncoding) -> Vec<u8> {
    match encoding {
        TextEncoding::Utf8 => content.as_bytes().to_vec(),
        TextEncoding::Utf8Bom => {
            let mut bytes = Vec::with_capacity(content.len() + 3);
            bytes.extend_from_slice(&[0xef, 0xbb, 0xbf]);
            bytes.extend_from_slice(content.as_bytes());
            bytes
        }
        TextEncoding::Unsupported => Vec::new(),
    }
}

fn detect_line_ending(bytes: &[u8]) -> LineEnding {
    let mut lf = 0usize;
    let mut crlf = 0usize;
    let mut cr = 0usize;
    let mut index = 0usize;
    while index < bytes.len() {
        match bytes[index] {
            b'\r' if bytes.get(index + 1) == Some(&b'\n') => {
                crlf += 1;
                index += 2;
            }
            b'\r' => {
                cr += 1;
                index += 1;
            }
            b'\n' => {
                lf += 1;
                index += 1;
            }
            _ => index += 1,
        }
    }
    match (lf > 0, crlf > 0, cr > 0) {
        (false, false, false) => LineEnding::None,
        (true, false, false) => LineEnding::Lf,
        (false, true, false) => LineEnding::Crlf,
        (false, false, true) => LineEnding::Cr,
        _ => LineEnding::Mixed,
    }
}
