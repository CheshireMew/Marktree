use std::path::Path;

use globset::GlobSet;

use crate::types::DocumentKind;

const MARKDOWN_EXTENSIONS: &[&str] = &["md", "markdown", "mdx"];
const TEXT_EXTENSIONS: &[&str] = &[
    "txt", "json", "jsonl", "yaml", "yml", "toml", "ini", "csv", "tsv", "html", "css", "js", "ts",
    "vue", "rs", "py", "java", "kt", "xml",
];
const IMAGE_EXTENSIONS: &[&str] = &["png", "jpg", "jpeg", "gif", "webp", "svg", "avif"];
const AUDIO_EXTENSIONS: &[&str] = &["mp3", "m4a", "aac", "wav", "ogg", "flac", "opus"];
const VIDEO_EXTENSIONS: &[&str] = &["mp4", "webm", "m4v", "mov", "ogv"];
const DEFAULT_EXCLUDED_DIRECTORIES: &[&str] =
    &["node_modules", "target", "dist", ".gradle", ".idea"];

pub const VERSIONED_WORKSPACE_CONFIG: &str = ".marktree/config.json";
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PortablePathViolation {
    GitMetadata,
    MarktreeInternal,
    TransactionArtifact,
}

pub fn document_kind(path: &Path) -> DocumentKind {
    match normalized_extension(path).as_deref() {
        Some(extension) if MARKDOWN_EXTENSIONS.contains(&extension) => DocumentKind::Markdown,
        Some(extension) if IMAGE_EXTENSIONS.contains(&extension) => DocumentKind::Image,
        Some("pdf") => DocumentKind::Pdf,
        Some(extension) if AUDIO_EXTENSIONS.contains(&extension) => DocumentKind::Audio,
        Some(extension) if VIDEO_EXTENSIONS.contains(&extension) => DocumentKind::Video,
        Some(extension) if TEXT_EXTENSIONS.contains(&extension) => DocumentKind::Text,
        _ => DocumentKind::Other,
    }
}

pub fn supported_image_extension(path: &Path) -> Option<String> {
    let extension = normalized_extension(path)?;
    IMAGE_EXTENSIONS
        .contains(&extension.as_str())
        .then_some(extension)
}

pub fn is_visible_workspace_path(relative: &str, is_directory: bool, ignore_set: &GlobSet) -> bool {
    let parts = relative.split('/').collect::<Vec<_>>();
    let directory_parts = if is_directory {
        parts.as_slice()
    } else {
        &parts[..parts.len().saturating_sub(1)]
    };
    !matches_path_or_ancestor(ignore_set, relative)
        && !parts.iter().copied().any(is_internal_transaction_file_name)
        && !parts
            .iter()
            .copied()
            .any(|part| part.eq_ignore_ascii_case(".git") || part.eq_ignore_ascii_case(".marktree"))
        && !directory_parts.iter().copied().any(|part| {
            DEFAULT_EXCLUDED_DIRECTORIES
                .iter()
                .any(|excluded| part.eq_ignore_ascii_case(excluded))
        })
}

/// Marktree publishes workspace writes through operation-owned sibling files.
/// They are implementation state, never workspace content, even if a process
/// stops before recovery can finish or remove them.
pub fn is_internal_transaction_file_name(file_name: &str) -> bool {
    if !file_name.starts_with('.') {
        return false;
    }
    if let Some(marker) = file_name.rfind(".marktree-") {
        let suffix = &file_name[marker + ".marktree-".len()..];
        return [".tmp", ".previous", ".rejected"]
            .iter()
            .any(|ending| suffix.strip_suffix(ending).is_some_and(is_operation_id));
    }

    let Some(stem) = file_name.strip_suffix(".tmp") else {
        return false;
    };
    let mut parts = stem.rsplitn(3, '.');
    let sequence = parts.next().unwrap_or_default();
    let process_id = parts.next().unwrap_or_default();
    let original_name = parts.next().unwrap_or_default();
    !original_name.is_empty()
        && !process_id.is_empty()
        && process_id.bytes().all(|byte| byte.is_ascii_digit())
        && !sequence.is_empty()
        && sequence.bytes().all(|byte| byte.is_ascii_digit())
}

fn is_operation_id(value: &str) -> bool {
    value.len() == 24 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

pub fn is_observable_workspace_path(
    relative: &str,
    is_directory: bool,
    ignore_set: &GlobSet,
) -> bool {
    relative == VERSIONED_WORKSPACE_CONFIG
        || is_visible_workspace_path(relative, is_directory, ignore_set)
}

pub fn portable_workspace_path_violation(
    relative: &str,
    is_directory: bool,
) -> Option<PortablePathViolation> {
    let parts = relative.split('/').collect::<Vec<_>>();
    if parts.iter().any(|part| part.eq_ignore_ascii_case(".git")) {
        return Some(PortablePathViolation::GitMetadata);
    }
    if parts.iter().copied().any(is_internal_transaction_file_name) {
        return Some(PortablePathViolation::TransactionArtifact);
    }
    if parts
        .iter()
        .any(|part| part.eq_ignore_ascii_case(".marktree"))
        && !((is_directory && relative.eq_ignore_ascii_case(".marktree"))
            || (!is_directory && relative.eq_ignore_ascii_case(VERSIONED_WORKSPACE_CONFIG)))
    {
        return Some(PortablePathViolation::MarktreeInternal);
    }
    None
}

pub fn is_portable_workspace_path(relative: &str, is_directory: bool) -> bool {
    portable_workspace_path_violation(relative, is_directory).is_none()
}

fn matches_path_or_ancestor(ignore_set: &GlobSet, relative: &str) -> bool {
    if ignore_set.is_match(relative) {
        return true;
    }
    let mut prefix = String::new();
    for part in relative
        .split('/')
        .take(relative.split('/').count().saturating_sub(1))
    {
        if !prefix.is_empty() {
            prefix.push('/');
        }
        prefix.push_str(part);
        if ignore_set.is_match(&prefix) {
            return true;
        }
    }
    false
}

fn normalized_extension(path: &Path) -> Option<String> {
    path.extension()
        .map(|value| value.to_string_lossy().to_ascii_lowercase())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_policy_classifies_workspace_files() {
        assert_eq!(
            document_kind(Path::new("notes/day.MD")),
            DocumentKind::Markdown
        );
        assert_eq!(
            document_kind(Path::new("assets/photo.WebP")),
            DocumentKind::Image
        );
        assert_eq!(document_kind(Path::new("data.json")), DocumentKind::Text);
        assert_eq!(document_kind(Path::new("manual.pdf")), DocumentKind::Pdf);
        assert_eq!(document_kind(Path::new("voice.opus")), DocumentKind::Audio);
        assert_eq!(document_kind(Path::new("demo.mp4")), DocumentKind::Video);
        assert_eq!(
            document_kind(Path::new("records.jsonl")),
            DocumentKind::Text
        );
        assert_eq!(document_kind(Path::new("archive.bin")), DocumentKind::Other);
    }

    #[test]
    fn one_policy_owns_visible_versioned_and_observable_boundaries() {
        let ignored = globset::GlobSetBuilder::new().build().unwrap();
        assert!(is_visible_workspace_path("notes/day.md", false, &ignored));
        assert!(!is_visible_workspace_path(
            "target/debug/build.log",
            false,
            &ignored
        ));
        assert!(!is_visible_workspace_path(
            "TARGET/debug/build.log",
            false,
            &ignored
        ));
        assert!(!is_visible_workspace_path(
            "Node_Modules/package/index.js",
            false,
            &ignored
        ));
        assert!(!is_visible_workspace_path("DIST/app.js", false, &ignored));
        assert!(is_visible_workspace_path("target", false, &ignored));
        assert!(!is_visible_workspace_path("target", true, &ignored));
        assert!(!is_visible_workspace_path(
            VERSIONED_WORKSPACE_CONFIG,
            false,
            &ignored
        ));
        assert!(is_observable_workspace_path(
            VERSIONED_WORKSPACE_CONFIG,
            false,
            &ignored
        ));
        assert!(!is_observable_workspace_path(
            ".marktree/recovery/pending.json",
            false,
            &ignored
        ));
        assert!(!is_visible_workspace_path(
            "nested/.GIT/config",
            false,
            &ignored
        ));
        assert!(!is_visible_workspace_path(
            ".note.md.marktree-0123456789abcdef01234567.tmp",
            false,
            &ignored
        ));
        assert!(!is_observable_workspace_path(
            "notes/.note.md.marktree-0123456789abcdef01234567.previous",
            false,
            &ignored
        ));
        assert!(!is_visible_workspace_path(
            "notes/.note.md.1234.99.tmp",
            false,
            &ignored
        ));
        assert!(is_visible_workspace_path(
            "notes/.ordinary.tmp",
            false,
            &ignored
        ));
    }

    #[test]
    fn transaction_file_detection_is_strict_about_the_reserved_shape() {
        assert!(is_internal_transaction_file_name(
            ".note.md.marktree-0123456789abcdef01234567.tmp"
        ));
        assert!(is_internal_transaction_file_name(
            ".note.md.marktree-0123456789abcdef01234567.rejected"
        ));
        assert!(is_internal_transaction_file_name(".note.md.123.4.tmp"));
        assert!(!is_internal_transaction_file_name(".note.md.tmp"));
        assert!(!is_internal_transaction_file_name(
            ".note.md.marktree-human-readable.tmp"
        ));
    }

    #[test]
    fn portable_policy_allows_content_and_versioned_config_only() {
        assert!(is_portable_workspace_path("notes/day.md", false));
        assert!(is_portable_workspace_path(".marktree", true));
        assert!(is_portable_workspace_path(
            VERSIONED_WORKSPACE_CONFIG,
            false
        ));
        assert_eq!(
            portable_workspace_path_violation("nested/.git/config", false),
            Some(PortablePathViolation::GitMetadata)
        );
        assert_eq!(
            portable_workspace_path_violation(".marktree/recovery/pending.json", false),
            Some(PortablePathViolation::MarktreeInternal)
        );
        assert_eq!(
            portable_workspace_path_violation(
                "notes/.day.md.marktree-0123456789abcdef01234567.previous",
                false,
            ),
            Some(PortablePathViolation::TransactionArtifact)
        );
    }
}
