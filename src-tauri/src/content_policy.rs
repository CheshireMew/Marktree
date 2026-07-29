use std::path::Path;

use crate::types::DocumentKind;

const MARKDOWN_EXTENSIONS: &[&str] = &["md", "markdown", "mdx"];
const TEXT_EXTENSIONS: &[&str] = &[
    "txt", "json", "yaml", "yml", "toml", "ini", "csv", "tsv", "html", "css", "js", "ts", "vue",
    "rs", "py", "java", "kt", "xml",
];
const IMAGE_EXTENSIONS: &[&str] = &["png", "jpg", "jpeg", "gif", "webp", "svg", "avif"];

pub fn document_kind(path: &Path) -> DocumentKind {
    match normalized_extension(path).as_deref() {
        Some(extension) if MARKDOWN_EXTENSIONS.contains(&extension) => DocumentKind::Markdown,
        Some(extension) if IMAGE_EXTENSIONS.contains(&extension) => DocumentKind::Image,
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

pub fn is_marktree_managed_path(path: &str) -> bool {
    matches!(
        document_kind(Path::new(path)),
        DocumentKind::Markdown | DocumentKind::Image
    ) || path == ".marktree/config.json"
}

fn normalized_extension(path: &Path) -> Option<String> {
    path.extension()
        .map(|value| value.to_string_lossy().to_ascii_lowercase())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_policy_classifies_documents_and_sync_paths() {
        assert_eq!(
            document_kind(Path::new("notes/day.MD")),
            DocumentKind::Markdown
        );
        assert_eq!(
            document_kind(Path::new("assets/photo.WebP")),
            DocumentKind::Image
        );
        assert_eq!(document_kind(Path::new("data.json")), DocumentKind::Text);
        assert_eq!(document_kind(Path::new("archive.bin")), DocumentKind::Other);
        assert!(is_marktree_managed_path("notes/day.md"));
        assert!(is_marktree_managed_path("assets/photo.webp"));
        assert!(is_marktree_managed_path(".marktree/config.json"));
        assert!(!is_marktree_managed_path("src/main.rs"));
    }
}
