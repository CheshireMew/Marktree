use std::{
    fs,
    path::{Path, PathBuf},
};

use super::config::read_workspace_config;
use crate::{
    content_policy::{document_kind, supported_image_extension},
    error::{AppError, AppResult},
    file_version::hash_file,
    paths::{
        atomic_copy_if_version, canonical_root, normalize_content_relative, path_to_slashes,
        resolve_existing_file, resolve_for_write,
    },
    state::PersistentState,
    types::{AssetWriteResult, DocumentKind, WorkspaceFilePreview},
    workspace_operation::{execute_mutation, WorkspaceChangeIntent, WorkspaceOperationKind},
};

pub(crate) const MAX_ASSET_BYTES: u64 = 64 * 1024 * 1024;
const MAX_IMAGE_PREVIEW_BYTES: u64 = 64 * 1024 * 1024;
const MAX_PDF_PREVIEW_BYTES: u64 = 512 * 1024 * 1024;
const MAX_STREAM_PREVIEW_BYTES: u64 = 8 * 1024 * 1024 * 1024;

pub fn read_workspace_preview(root: &str, path: &str) -> AppResult<WorkspaceFilePreview> {
    let root_path = canonical_root(root)?;
    let relative = normalize_content_relative(path)?;
    let file_path = resolve_existing_file(&root_path, &relative)?;
    let kind = document_kind(&file_path);
    let extension = file_path
        .extension()
        .and_then(|value| value.to_str())
        .map(str::to_ascii_lowercase)
        .unwrap_or_default();
    let media_type = match extension.as_str() {
        "jpg" | "jpeg" => "image/jpeg",
        "svg" => "image/svg+xml",
        "pdf" => "application/pdf",
        "mp3" => "audio/mpeg",
        "m4a" => "audio/mp4",
        "aac" => "audio/aac",
        "wav" => "audio/wav",
        "ogg" | "opus" => "audio/ogg",
        "flac" => "audio/flac",
        "mp4" | "m4v" => "video/mp4",
        "webm" => "video/webm",
        "mov" => "video/quicktime",
        "ogv" => "video/ogg",
        value => match value {
            "png" => "image/png",
            "gif" => "image/gif",
            "webp" => "image/webp",
            "avif" => "image/avif",
            _ => {
                return Err(AppError::Message(
                    "This file type cannot be previewed in Marktree.".to_owned(),
                ))
            }
        },
    };
    if !matches!(
        kind,
        DocumentKind::Image | DocumentKind::Pdf | DocumentKind::Audio | DocumentKind::Video
    ) {
        return Err(AppError::Message(
            "This file type cannot be previewed in Marktree.".to_owned(),
        ));
    }
    let size = fs::metadata(&file_path)?.len();
    let max_bytes = match kind {
        DocumentKind::Image => MAX_IMAGE_PREVIEW_BYTES,
        DocumentKind::Pdf => MAX_PDF_PREVIEW_BYTES,
        DocumentKind::Audio | DocumentKind::Video => MAX_STREAM_PREVIEW_BYTES,
        _ => unreachable!("preview kind was validated above"),
    };
    if size > max_bytes {
        return Err(AppError::Message(
            "The file is too large to preview in Marktree; open it with the system instead."
                .to_owned(),
        ));
    }
    Ok(WorkspaceFilePreview {
        path: relative,
        kind,
        media_type: media_type.to_owned(),
        resource_path: file_path.to_string_lossy().into_owned(),
    })
}

pub fn write_asset(
    root: &str,
    document_path: &str,
    file_name: &str,
    source_path: &Path,
    assets_dir: Option<&str>,
    app_state: &PersistentState,
) -> AppResult<AssetWriteResult> {
    let root_path = canonical_root(root)?;
    let document_relative = normalize_content_relative(document_path)?;
    let configured_assets_dir;
    let selected_assets_dir = if let Some(assets_dir) = assets_dir {
        assets_dir
    } else {
        configured_assets_dir = read_workspace_config(root)?.config.assets_dir;
        configured_assets_dir.as_str()
    };
    let asset_root_relative = normalize_content_relative(selected_assets_dir)?;
    if !source_path.is_file() || fs::metadata(source_path)?.len() > MAX_ASSET_BYTES {
        return Err(AppError::Message(
            "The image is too large to store in Marktree.".to_owned(),
        ));
    }
    let sha256 = hash_file(source_path)?;
    let extension = supported_image_extension(Path::new(file_name))
        .ok_or_else(|| AppError::Message("Unsupported image type.".to_owned()))?;
    let stored_name = format!("{}.{}", &sha256[..24], extension);
    let relative_asset = format!(
        "{}/{}",
        asset_root_relative.trim_end_matches('/'),
        stored_name
    );
    let asset_path = resolve_for_write(&root_path, &relative_asset)?;
    execute_mutation(
        root,
        WorkspaceOperationKind::WriteFile {
            path: relative_asset.clone(),
            version: sha256.clone(),
            previous_version: None,
            replace_existing: true,
        },
        vec![WorkspaceChangeIntent::upsert(&relative_asset, &sha256)],
        app_state,
        (),
        |operation| {
            if !asset_path.exists() || hash_file(&asset_path)? != sha256 {
                atomic_copy_if_version(
                    source_path,
                    &asset_path,
                    None,
                    !asset_path.exists(),
                    true,
                    &operation.id,
                )?;
            }
            Ok(())
        },
    )?;

    let document_directory = Path::new(&document_relative)
        .parent()
        .unwrap_or_else(|| Path::new(""));
    let markdown_path = pathdiff::diff_paths(Path::new(&relative_asset), document_directory)
        .unwrap_or_else(|| PathBuf::from(&relative_asset));
    let markdown_path = path_to_slashes(&markdown_path);
    Ok(AssetWriteResult {
        path: relative_asset,
        markdown_path,
        sha256,
    })
}
