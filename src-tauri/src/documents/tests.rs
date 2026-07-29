use std::fs;

use base64::{engine::general_purpose::STANDARD, Engine};
use tempfile::TempDir;

use super::*;
use crate::{
    error::AppError,
    file_version::hash_bytes,
    paths::normalize_relative,
    state::PersistentState,
    types::{
        LineEnding, ManagedChangeKind, RepositoryConfig, SaveDocumentRequest,
        SaveRepositoryConfigRequest, TextEncoding,
    },
};

#[test]
fn normalizes_safe_relative_paths() {
    assert_eq!(
        normalize_relative(r"docs\hello.md").unwrap(),
        "docs/hello.md"
    );
    assert!(normalize_relative("../secret.md").is_err());
    assert!(normalize_relative("C:\\secret.md").is_err());
}

#[test]
fn content_hash_is_stable() {
    assert_eq!(
        hash_bytes("Marktree".as_bytes()),
        "6e750fe0fc2f976066101b4ed820a1d3f4ecf3dee792e435c8979dd63207864e"
    );
}

#[test]
fn complex_markdown_is_byte_stable_outside_the_edited_range() {
    let repository = TempDir::new().unwrap();
    let app_data = TempDir::new().unwrap();
    let state = PersistentState::load(app_data.path()).unwrap();
    let source = concat!(
        "---\r\ntitle: 中文\r\n---\r\n\r\n",
        "# Heading\r\n\r\n",
        "| A | B |\r\n|---|---|\r\n| 1 | 2 |\r\n\r\n",
        "- [x] task\r\n\r\n",
        "Footnote[^1] and $E=mc^2$.\r\n\r\n",
        "[^1]: kept\r\n\r\n",
        "```mermaid\r\ngraph LR\r\nA-->B\r\n```\r\n\r\n",
        ":::unknown value\r\nkeep exactly\r\n:::\r\n"
    );
    let file = repository.path().join("document.md");
    fs::write(&file, source.as_bytes()).unwrap();

    let opened = read_document(repository.path().to_str().unwrap(), "document.md").unwrap();
    assert_eq!(fs::read(&file).unwrap(), source.as_bytes());
    let edited = opened.content.replacen("Heading", "Edited heading", 1);
    save_document(
        SaveDocumentRequest {
            root: repository.path().to_string_lossy().into_owned(),
            path: "document.md".to_owned(),
            content: edited,
            expected_sha256: Some(opened.sha256),
            expected_missing: false,
            encoding: opened.encoding,
        },
        &state,
    )
    .unwrap();
    assert_eq!(
        fs::read(&file).unwrap(),
        source.replacen("Heading", "Edited heading", 1).as_bytes()
    );
}

#[test]
fn opening_a_document_persists_it_for_recent_file_consumers() {
    let repository = TempDir::new().unwrap();
    let app_data = TempDir::new().unwrap();
    let root = repository.path().to_string_lossy().into_owned();
    fs::write(repository.path().join("recent.md"), "# Recent\n").unwrap();
    let state = PersistentState::load(app_data.path()).unwrap();

    let opened = open_document(&root, "recent.md", &state).unwrap();
    assert_eq!(opened.path, "recent.md");
    let reloaded = PersistentState::load(app_data.path()).unwrap();
    assert_eq!(
        reloaded.snapshot().recent_files,
        vec![format!("{root}\nrecent.md")]
    );
}

#[test]
fn utf8_bom_and_line_endings_survive_the_real_open_save_chain() {
    let repository = TempDir::new().unwrap();
    let app_data = TempDir::new().unwrap();
    let state = PersistentState::load(app_data.path()).unwrap();
    let root = repository.path().to_string_lossy().into_owned();
    let file = repository.path().join("bom.md");
    let mut original = vec![0xef, 0xbb, 0xbf];
    original.extend_from_slice(b"# Title\r\n\r\nBody\r\n");
    fs::write(&file, &original).unwrap();

    let opened = open_document(&root, "bom.md", &state).unwrap();
    assert_eq!(opened.encoding, TextEncoding::Utf8Bom);
    assert_eq!(opened.line_ending, LineEnding::Crlf);
    assert!(!opened.content.starts_with('\u{feff}'));
    let saved = save_document(
        SaveDocumentRequest {
            root: root.clone(),
            path: "bom.md".to_owned(),
            content: opened.content.replace("Body", "Edited"),
            expected_sha256: Some(opened.sha256),
            expected_missing: false,
            encoding: opened.encoding,
        },
        &state,
    )
    .unwrap();

    assert_eq!(saved.encoding, TextEncoding::Utf8Bom);
    assert_eq!(saved.line_ending, LineEnding::Crlf);
    assert_eq!(
        fs::read(file).unwrap(),
        b"\xef\xbb\xbf# Title\r\n\r\nEdited\r\n"
    );
}

#[test]
fn mixed_and_unsupported_text_formats_are_reported_when_opened() {
    let repository = TempDir::new().unwrap();
    let root = repository.path().to_string_lossy().into_owned();
    fs::write(repository.path().join("mixed.md"), b"# Mixed\r\nLF\nCR\r").unwrap();
    fs::write(repository.path().join("legacy.md"), [0xff, 0xfe, 0x41]).unwrap();

    let documents = list_documents(&root, &[]).unwrap();
    assert_eq!(documents.len(), 2);
    let mixed = read_document(&root, "mixed.md").unwrap();
    assert_eq!(mixed.encoding, TextEncoding::Utf8);
    assert_eq!(mixed.line_ending, LineEnding::Mixed);
    assert!(read_document(&root, "legacy.md").is_err());
}

#[test]
fn an_external_write_stops_the_save_before_user_content_is_overwritten() {
    let repository = TempDir::new().unwrap();
    let app_data = TempDir::new().unwrap();
    let state = PersistentState::load(app_data.path()).unwrap();
    let root = repository.path().to_string_lossy().into_owned();
    let file = repository.path().join("external.md");
    fs::write(&file, "# Original\n").unwrap();
    let opened = open_document(&root, "external.md", &state).unwrap();

    fs::write(&file, "# Changed elsewhere\n").unwrap();
    let error = save_document(
        SaveDocumentRequest {
            root,
            path: "external.md".to_owned(),
            content: "# Editor change\n".to_owned(),
            expected_sha256: Some(opened.sha256),
            expected_missing: false,
            encoding: opened.encoding,
        },
        &state,
    )
    .unwrap_err();

    assert!(matches!(error, AppError::ExternalChange));
    assert_eq!(fs::read_to_string(file).unwrap(), "# Changed elsewhere\n");
}

#[test]
fn a_confirmed_deletion_cannot_overwrite_a_file_that_reappeared() {
    let repository = TempDir::new().unwrap();
    let app_data = TempDir::new().unwrap();
    let state = PersistentState::load(app_data.path()).unwrap();
    let root = repository.path().to_string_lossy().into_owned();
    let file = repository.path().join("reappeared.md");
    fs::write(&file, "# Reappeared\n").unwrap();

    let error = save_document(
        SaveDocumentRequest {
            root,
            path: "reappeared.md".to_owned(),
            content: "# Editor\n".to_owned(),
            expected_sha256: None,
            expected_missing: true,
            encoding: TextEncoding::Utf8,
        },
        &state,
    )
    .unwrap_err();

    assert!(matches!(error, AppError::ExternalChange));
    assert_eq!(fs::read_to_string(file).unwrap(), "# Reappeared\n");
}

#[test]
fn image_preview_reads_the_exact_asset_bytes_from_the_repository() {
    let repository = TempDir::new().unwrap();
    let image = repository.path().join("assets").join("diagram.png");
    fs::create_dir_all(image.parent().unwrap()).unwrap();
    let bytes = b"\x89PNG\r\n\x1a\npreview";
    fs::write(&image, bytes).unwrap();

    let preview = read_asset(repository.path().to_str().unwrap(), "assets/diagram.png").unwrap();

    assert_eq!(preview.path, "assets/diagram.png");
    assert_eq!(preview.media_type, "image/png");
    assert_eq!(STANDARD.decode(preview.base64_data).unwrap(), bytes);
}

#[test]
fn content_addressed_asset_write_repairs_a_corrupt_existing_blob() {
    let repository = TempDir::new().unwrap();
    let app_data = TempDir::new().unwrap();
    let state = PersistentState::load(app_data.path()).unwrap();
    let root = repository.path().to_string_lossy().into_owned();
    let intended = b"\x89PNG\r\n\x1a\nintended";
    let sha256 = hash_bytes(intended);
    let relative = format!("assets/{}.png", &sha256[..24]);
    let destination = repository.path().join(&relative);
    fs::create_dir_all(destination.parent().unwrap()).unwrap();
    fs::write(&destination, b"corrupt").unwrap();

    let written = write_asset(
        &root,
        "notes/day.md",
        "diagram.png",
        &STANDARD.encode(intended),
        None,
        &state,
    )
    .unwrap();

    assert_eq!(written.path, relative);
    assert_eq!(fs::read(destination).unwrap(), intended);
    assert!(state
        .managed_changes(&root)
        .iter()
        .any(|change| change.path == written.path
            && change.sha256 == sha256
            && change.kind == ManagedChangeKind::Asset));
}

#[test]
fn external_repository_config_change_is_never_overwritten() {
    let repository = TempDir::new().unwrap();
    let app_data = TempDir::new().unwrap();
    let state = PersistentState::load(app_data.path()).unwrap();
    let root = repository.path().to_string_lossy().into_owned();
    let config_dir = repository.path().join(".marktree");
    fs::create_dir(&config_dir).unwrap();
    let config_path = config_dir.join("config.json");
    fs::write(&config_path, r#"{"assetsDir":"assets","ignoreRules":[]}"#).unwrap();
    let opened = read_repository_config(&root).unwrap();
    fs::write(
        &config_path,
        r#"{"assetsDir":"external-assets","ignoreRules":[]}"#,
    )
    .unwrap();

    let error = save_repository_config(
        SaveRepositoryConfigRequest {
            root,
            config: RepositoryConfig {
                assets_dir: "editor-assets".to_owned(),
                ignore_rules: Vec::new(),
            },
            expected_sha256: opened.sha256,
            expected_missing: opened.missing,
        },
        &state,
    )
    .unwrap_err();

    assert!(matches!(error, AppError::ExternalChange));
    assert_eq!(
        fs::read_to_string(config_path).unwrap(),
        r#"{"assetsDir":"external-assets","ignoreRules":[]}"#
    );
}

#[test]
fn repository_metadata_files_are_not_exposed_as_documents() {
    let repository = TempDir::new().unwrap();
    fs::write(repository.path().join(".git"), "gitdir: elsewhere").unwrap();
    fs::create_dir(repository.path().join(".marktree")).unwrap();
    fs::write(
        repository.path().join(".marktree").join("config.json"),
        r#"{"assetsDir":"assets","ignoreRules":[]}"#,
    )
    .unwrap();
    fs::write(repository.path().join("visible.md"), "# Visible\n").unwrap();

    let listed = list_documents(repository.path().to_str().unwrap(), &[]).unwrap();

    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].path, "visible.md");
}
