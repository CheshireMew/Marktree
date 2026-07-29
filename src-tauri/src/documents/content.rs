use std::{fs, time::UNIX_EPOCH};

use crate::{
    content_policy::document_kind,
    error::{AppError, AppResult},
    file_version::{hash_bytes, verify_expected_version},
    paths::{
        atomic_create, atomic_write, canonical_root, normalize_relative, resolve_existing_file,
        resolve_for_write,
    },
    state::PersistentState,
    types::{
        DocumentContent, DocumentKind, LineEnding, ManagedChangeKind, SaveDocumentRequest,
        SaveDocumentResult, TextEncoding,
    },
};

pub fn read_document(root: &str, path: &str) -> AppResult<DocumentContent> {
    let root_path = canonical_root(root)?;
    let file_path = resolve_existing_file(&root_path, path)?;
    let kind = document_kind(&file_path);
    if !matches!(kind, DocumentKind::Markdown | DocumentKind::Text) {
        return Err(AppError::Message(
            "This file type cannot be opened as text.".to_owned(),
        ));
    }
    let bytes = fs::read(&file_path)?;
    let encoding = text_encoding(&bytes);
    let text_bytes = strip_utf8_bom(&bytes);
    let content = String::from_utf8(text_bytes.to_vec())
        .map_err(|_| AppError::Message("The file is not valid UTF-8 text.".to_owned()))?;
    let metadata = fs::metadata(&file_path)?;
    Ok(DocumentContent {
        path: normalize_relative(path)?,
        content,
        modified_ms: modified_ms(&metadata),
        sha256: hash_bytes(&bytes),
        read_only: kind != DocumentKind::Markdown,
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
    let root_path = canonical_root(&request.root)?;
    let relative = normalize_relative(&request.path)?;
    let file_path = resolve_for_write(&root_path, &relative)?;
    if document_kind(&file_path) != DocumentKind::Markdown {
        return Err(AppError::Message(
            "Only Markdown documents can be edited.".to_owned(),
        ));
    }

    verify_expected_version(
        &file_path,
        request.expected_sha256.as_deref(),
        request.expected_missing,
    )?;

    if let Some(parent) = file_path.parent() {
        fs::create_dir_all(parent)?;
    }
    if request.encoding == TextEncoding::Unsupported {
        return Err(AppError::Message(
            "Unsupported text encodings cannot be saved.".to_owned(),
        ));
    }
    let bytes = encode_text(&request.content, request.encoding);
    let sha256 = hash_bytes(&bytes);
    app_state.record_change(
        &request.root,
        &relative,
        &sha256,
        ManagedChangeKind::Document,
    )?;
    atomic_write(&file_path, &bytes)?;
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

pub fn create_document(
    root: &str,
    path: &str,
    app_state: &PersistentState,
) -> AppResult<DocumentContent> {
    let root_path = canonical_root(root)?;
    let relative = normalize_relative(path)?;
    let file_path = resolve_for_write(&root_path, &relative)?;
    if file_path.exists() {
        return Err(AppError::Message(
            "A file already exists at that path.".to_owned(),
        ));
    }
    if document_kind(&file_path) != DocumentKind::Markdown {
        return Err(AppError::Message(
            "New documents must use .md, .markdown, or .mdx.".to_owned(),
        ));
    }
    if let Some(parent) = file_path.parent() {
        fs::create_dir_all(parent)?;
    }
    let sha256 = hash_bytes(b"");
    app_state.record_change(root, &relative, &sha256, ManagedChangeKind::Document)?;
    atomic_create(&file_path, b"")?;
    read_document(root, &relative)
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
