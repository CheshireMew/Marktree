use std::fs;
use std::fs::OpenOptions;

use tempfile::TempDir;

use super::*;
use crate::{
    error::AppError,
    file_version::hash_bytes,
    paths::normalize_relative,
    state::PersistentState,
    types::{
        DocumentSearchMatchType, LineEnding, SaveDocumentRequest, SaveWorkspaceConfigRequest,
        TextEncoding, WorkspaceConfig, WorkspaceEntryType,
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
fn performance_oversized_text_is_rejected_before_it_is_loaded() {
    let workspace = TempDir::new().unwrap();
    let file = workspace.path().join("oversized.md");
    OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&file)
        .unwrap()
        .set_len(MAX_EDITABLE_DOCUMENT_BYTES + 1)
        .unwrap();

    let error = read_document(workspace.path().to_str().unwrap(), "oversized.md").unwrap_err();
    assert!(error.to_string().contains("too large"));
}

#[test]
fn workspace_config_change_remains_observable_even_while_external_content_is_invalid() {
    let workspace = TempDir::new().unwrap();
    let config_dir = workspace.path().join(".marktree");
    fs::create_dir(&config_dir).unwrap();
    let config_path = config_dir.join("config.json");
    fs::write(&config_path, b"{ incomplete").unwrap();
    let canonical_config = fs::canonicalize(&config_path).unwrap();

    assert!(is_observable_path(workspace.path().to_str().unwrap(), &canonical_config).unwrap());
}

#[test]
fn performance_workspace_entry_patch_only_rebuilds_changed_subtrees() {
    let workspace = TempDir::new().unwrap();
    fs::create_dir(workspace.path().join("notes")).unwrap();
    fs::write(workspace.path().join("notes/day.md"), b"# Day\n").unwrap();
    fs::write(workspace.path().join("other.md"), b"# Other\n").unwrap();
    let root = workspace.path().to_string_lossy().into_owned();

    let patch = workspace_entries_patch(&root, &["notes".to_owned()], None).unwrap();

    assert!(!patch.full_reload_required);
    assert_eq!(patch.removed_paths, vec!["notes"]);
    assert!(patch.entries.iter().any(|entry| entry.path == "notes"));
    assert!(patch
        .entries
        .iter()
        .any(|entry| entry.path == "notes/day.md"));
    assert!(!patch.entries.iter().any(|entry| entry.path == "other.md"));
}

#[test]
fn performance_workspace_config_patch_requests_a_full_policy_reload() {
    let workspace = TempDir::new().unwrap();
    let root = workspace.path().to_string_lossy().into_owned();
    let patch = workspace_entries_patch(
        &root,
        &[crate::content_policy::VERSIONED_WORKSPACE_CONFIG.to_owned()],
        None,
    )
    .unwrap();

    assert!(patch.full_reload_required);
    assert!(patch.entries.is_empty());
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
fn unchanged_open_and_save_is_byte_for_byte_stable_on_disk() {
    let workspace = TempDir::new().unwrap();
    let app_data = TempDir::new().unwrap();
    let state = PersistentState::load(app_data.path()).unwrap();
    let root = workspace.path().to_string_lossy().into_owned();
    let file = workspace.path().join("unchanged.md");
    let original = b"\xef\xbb\xbf---\r\ntitle: \xe4\xb8\xad\xe6\x96\x87\r\n---\r\n\r\n# Title\r\n\r\n:::custom\r\nkeep exactly\r\n:::\r\n";
    fs::write(&file, original).unwrap();

    let opened = open_document(&root, "unchanged.md", &state).unwrap();
    save_document(
        SaveDocumentRequest {
            root: root.clone(),
            path: opened.path,
            content: opened.content,
            expected_sha256: Some(opened.sha256),
            expected_missing: false,
            encoding: opened.encoding,
        },
        &state,
    )
    .unwrap();

    assert_eq!(fs::read(file).unwrap(), original);
    assert!(state.workspace_changes(&root).is_empty());
}

#[cfg(target_os = "windows")]
#[test]
fn plain_workspace_real_create_save_move_and_trash_never_create_git_changes() {
    let workspace = TempDir::new().unwrap();
    let app_data = TempDir::new().unwrap();
    let state = PersistentState::load(app_data.path()).unwrap();
    let root = workspace.path().to_string_lossy().into_owned();

    let created = create_document(&root, "draft.md", &state).unwrap();
    save_document(
        SaveDocumentRequest {
            root: root.clone(),
            path: created.path,
            content: "# Plain workspace\r\n".to_owned(),
            expected_sha256: Some(created.sha256),
            expected_missing: false,
            encoding: created.encoding,
        },
        &state,
    )
    .unwrap();
    create_folder(&root, "archive", &state).unwrap();
    move_entry(&root, "draft.md", "archive/draft.md", &state).unwrap();

    assert_eq!(
        fs::read(workspace.path().join("archive/draft.md")).unwrap(),
        b"# Plain workspace\r\n"
    );
    assert!(state.workspace_changes(&root).is_empty());

    trash_entry(&root, "archive/draft.md", app_data.path(), &state).unwrap();

    assert!(!workspace.path().join("archive/draft.md").exists());
    assert!(state.workspace_changes(&root).is_empty());
}

#[test]
fn duplicate_entry_copies_real_files_and_records_only_new_git_paths() {
    let repository = TempDir::new().unwrap();
    git2::Repository::init(repository.path()).unwrap();
    fs::create_dir_all(repository.path().join("notes/empty")).unwrap();
    fs::write(repository.path().join("notes/day.md"), b"# Exact\r\n").unwrap();
    let app_data = TempDir::new().unwrap();
    let state = PersistentState::load(app_data.path()).unwrap();
    let root = repository.path().to_string_lossy().into_owned();

    let result = duplicate_entry(&root, "notes", "notes copy", &state).unwrap();

    assert_eq!(result.copied_files.len(), 1);
    assert_eq!(
        fs::read(repository.path().join("notes copy/day.md")).unwrap(),
        b"# Exact\r\n"
    );
    assert!(repository.path().join("notes copy/empty").is_dir());
    let changes = state.workspace_changes(&root);
    assert_eq!(changes.len(), 1);
    assert_eq!(changes[0].path, "notes copy/day.md");
    assert_eq!(
        changes[0].operation,
        crate::types::WorkspaceChangeOperation::Upsert
    );
}

#[test]
fn duplicate_entry_in_plain_workspace_never_creates_git_changes() {
    let workspace = TempDir::new().unwrap();
    fs::write(workspace.path().join("source.md"), b"plain").unwrap();
    let app_data = TempDir::new().unwrap();
    let state = PersistentState::load(app_data.path()).unwrap();
    let root = workspace.path().to_string_lossy().into_owned();

    duplicate_entry(&root, "source.md", "source copy.md", &state).unwrap();

    assert_eq!(
        fs::read(workspace.path().join("source copy.md")).unwrap(),
        b"plain"
    );
    assert!(state.workspace_changes(&root).is_empty());
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
fn search_returns_the_real_path_line_snippet_and_match_type() {
    let workspace = TempDir::new().unwrap();
    fs::create_dir(workspace.path().join("notes")).unwrap();
    fs::write(
        workspace.path().join("notes/roadmap.md"),
        "# Roadmap\n\nFirst milestone\nSecond milestone\nİstanbul target stays aligned\n",
    )
    .unwrap();
    fs::write(
        workspace.path().join("other.txt"),
        "Second milestone outside notes\n",
    )
    .unwrap();

    let path_response =
        search_documents(workspace.path().to_str().unwrap(), "roadmap", 20, || true).unwrap();
    let path_matches = path_response.results;
    assert_eq!(path_matches.len(), 2);
    assert_eq!(path_matches[0].path, "notes/roadmap.md");
    assert_eq!(path_matches[0].match_type, DocumentSearchMatchType::Path);
    assert_eq!(path_matches[0].line, None);
    assert_eq!(path_matches[1].match_type, DocumentSearchMatchType::Content);
    assert_eq!(path_matches[1].line, Some(1));
    assert_eq!(path_matches[1].snippet, "# Roadmap");

    let content_response = search_documents(
        workspace.path().to_str().unwrap(),
        "second milestone",
        20,
        || true,
    )
    .unwrap();
    let content_matches = content_response.results;
    assert_eq!(content_matches.len(), 2);
    assert_eq!(content_matches[0].path, "notes/roadmap.md");
    assert_eq!(content_matches[0].line, Some(4));
    assert_eq!(content_matches[0].column, Some(1));
    assert_eq!(content_matches[0].snippet, "Second milestone");
    assert_eq!(
        content_matches[0].match_type,
        DocumentSearchMatchType::Content
    );

    let filtered_response = search_documents_filtered(
        workspace.path().to_str().unwrap(),
        "second milestone",
        20,
        Some("notes"),
        &[crate::types::DocumentKind::Markdown],
        None,
        || true,
    )
    .unwrap();
    let filtered = filtered_response.results;
    assert_eq!(filtered.len(), 1);
    assert_eq!(filtered[0].path, "notes/roadmap.md");

    let unicode_response = search_documents(
        workspace.path().to_str().unwrap(),
        "target stays",
        20,
        || true,
    )
    .unwrap();
    let unicode_matches = unicode_response.results;
    assert_eq!(unicode_matches.len(), 1);
    assert_eq!(unicode_matches[0].line, Some(5));
    assert_eq!(unicode_matches[0].column, Some(10));
}

#[test]
fn search_skips_oversized_text_files_and_reports_the_resource_limit() {
    let workspace = TempDir::new().unwrap();
    fs::write(
        workspace.path().join("large.txt"),
        vec![b'x'; 8 * 1024 * 1024 + 1],
    )
    .unwrap();
    fs::write(workspace.path().join("small.md"), "find me\n").unwrap();

    let response =
        search_documents(workspace.path().to_str().unwrap(), "find me", 20, || true).unwrap();

    assert_eq!(response.results.len(), 1);
    assert_eq!(response.results[0].path, "small.md");
    assert_eq!(response.statistics.skipped_large_files, 1);
    assert!(response.statistics.truncated);
    assert!(response.statistics.scanned_bytes < 1024);
}

#[test]
fn search_skips_a_file_that_disappears_after_directory_enumeration() {
    let workspace = TempDir::new().unwrap();
    let disappearing = workspace.path().join("disappearing.md");
    fs::write(&disappearing, "find me\n").unwrap();
    let checks = std::cell::Cell::new(0usize);

    let response = search_documents(workspace.path().to_str().unwrap(), "find me", 20, || {
        let next = checks.get() + 1;
        checks.set(next);
        if next == 2 {
            fs::remove_file(&disappearing).unwrap();
        }
        true
    })
    .unwrap();

    assert!(response.results.is_empty());
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

    let entries = list_workspace_entries(&root, &[]).unwrap();
    assert_eq!(entries.len(), 2);
    assert!(entries
        .iter()
        .all(|entry| entry.entry_type == WorkspaceEntryType::File));
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
fn file_preview_reads_exact_image_pdf_audio_and_video_bytes_from_the_workspace() {
    let workspace = TempDir::new().unwrap();
    let image = workspace.path().join("assets").join("diagram.png");
    fs::create_dir_all(image.parent().unwrap()).unwrap();
    let bytes = b"\x89PNG\r\n\x1a\npreview";
    fs::write(&image, bytes).unwrap();

    let preview =
        read_workspace_preview(workspace.path().to_str().unwrap(), "assets/diagram.png").unwrap();

    assert_eq!(preview.path, "assets/diagram.png");
    assert_eq!(preview.kind, crate::types::DocumentKind::Image);
    assert_eq!(preview.media_type, "image/png");
    assert_eq!(fs::read(&preview.resource_path).unwrap(), bytes);

    let pdf_bytes = b"%PDF-1.7\npreview";
    fs::write(workspace.path().join("manual.pdf"), pdf_bytes).unwrap();
    let pdf = read_workspace_preview(workspace.path().to_str().unwrap(), "manual.pdf").unwrap();
    assert_eq!(pdf.kind, crate::types::DocumentKind::Pdf);
    assert_eq!(pdf.media_type, "application/pdf");
    assert_eq!(fs::read(&pdf.resource_path).unwrap(), pdf_bytes);

    let audio_bytes = b"ID3\x04\0\0preview";
    fs::write(workspace.path().join("sample.mp3"), audio_bytes).unwrap();
    let audio = read_workspace_preview(workspace.path().to_str().unwrap(), "sample.mp3").unwrap();
    assert_eq!(audio.kind, crate::types::DocumentKind::Audio);
    assert_eq!(audio.media_type, "audio/mpeg");
    assert_eq!(fs::read(&audio.resource_path).unwrap(), audio_bytes);

    let video_bytes = b"\0\0\0\x18ftypmp42preview";
    fs::write(workspace.path().join("sample.mp4"), video_bytes).unwrap();
    let video = read_workspace_preview(workspace.path().to_str().unwrap(), "sample.mp4").unwrap();
    assert_eq!(video.kind, crate::types::DocumentKind::Video);
    assert_eq!(video.media_type, "video/mp4");
    assert_eq!(fs::read(&video.resource_path).unwrap(), video_bytes);
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
    let source = app_data.path().join("intended.png");
    fs::write(&source, intended).unwrap();

    let written = write_asset(&root, "notes/day.md", "diagram.png", &source, None, &state).unwrap();

    assert_eq!(written.path, relative);
    assert_eq!(fs::read(destination).unwrap(), intended);
    assert!(state.workspace_changes(&root).is_empty());
}

#[test]
fn external_workspace_config_change_is_never_overwritten() {
    let repository = TempDir::new().unwrap();
    let app_data = TempDir::new().unwrap();
    let state = PersistentState::load(app_data.path()).unwrap();
    let root = repository.path().to_string_lossy().into_owned();
    let config_dir = repository.path().join(".marktree");
    fs::create_dir(&config_dir).unwrap();
    let config_path = config_dir.join("config.json");
    fs::write(&config_path, r#"{"assetsDir":"assets","ignoreRules":[]}"#).unwrap();
    let opened = read_workspace_config(&root).unwrap();
    fs::write(
        &config_path,
        r#"{"assetsDir":"external-assets","ignoreRules":[]}"#,
    )
    .unwrap();

    let error = save_workspace_config(
        SaveWorkspaceConfigRequest {
            root,
            config: WorkspaceConfig {
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
fn workspace_internal_files_are_not_exposed_as_entries() {
    let workspace = TempDir::new().unwrap();
    fs::write(workspace.path().join(".git"), "gitdir: elsewhere").unwrap();
    fs::create_dir(workspace.path().join(".marktree")).unwrap();
    fs::write(
        workspace.path().join(".marktree").join("config.json"),
        r#"{"assetsDir":"assets","ignoreRules":[]}"#,
    )
    .unwrap();
    fs::write(workspace.path().join("visible.md"), "# Visible\n").unwrap();

    let listed = list_workspace_entries(workspace.path().to_str().unwrap(), &[]).unwrap();

    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].path, "visible.md");
}
