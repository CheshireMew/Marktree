use std::{
    fs,
    path::{Path, PathBuf},
};

use base64::{engine::general_purpose::STANDARD, Engine};

use super::config::read_workspace_config;
use crate::{
    content_policy::{document_kind, supported_image_extension},
    error::{AppError, AppResult},
    file_version::hash_bytes,
    git,
    paths::{
        atomic_write, canonical_root, normalize_relative, path_to_slashes, resolve_existing_file,
        resolve_for_write,
    },
    state::PersistentState,
    types::{AssetPreview, AssetWriteResult, DocumentKind, WorkspaceChangeOperation},
};

const MAX_ASSET_BYTES: u64 = 64 * 1024 * 1024;

pub fn read_asset(root: &str, path: &str) -> AppResult<AssetPreview> {
    let root_path = canonical_root(root)?;
    let file_path = resolve_existing_file(&root_path, path)?;
    if document_kind(&file_path) != DocumentKind::Image {
        return Err(AppError::Message(
            "Only supported image assets can be previewed.".to_owned(),
        ));
    }
    let extension = file_path
        .extension()
        .and_then(|value| value.to_str())
        .map(str::to_ascii_lowercase)
        .unwrap_or_default();
    let media_type = match extension.as_str() {
        "jpg" | "jpeg" => "image/jpeg",
        "svg" => "image/svg+xml",
        value => match value {
            "png" => "image/png",
            "gif" => "image/gif",
            "webp" => "image/webp",
            "avif" => "image/avif",
            _ => {
                return Err(AppError::Message(
                    "Only supported image assets can be previewed.".to_owned(),
                ))
            }
        },
    };
    if fs::metadata(&file_path)?.len() > MAX_ASSET_BYTES {
        return Err(AppError::Message(
            "The image is too large to preview in Marktree.".to_owned(),
        ));
    }
    Ok(AssetPreview {
        path: normalize_relative(path)?,
        media_type: media_type.to_owned(),
        base64_data: STANDARD.encode(fs::read(file_path)?),
    })
}

pub fn write_asset(
    root: &str,
    document_path: &str,
    file_name: &str,
    base64_data: &str,
    assets_dir: Option<&str>,
    app_state: &PersistentState,
) -> AppResult<AssetWriteResult> {
    let root_path = canonical_root(root)?;
    let document_relative = normalize_relative(document_path)?;
    let configured_assets_dir;
    let selected_assets_dir = if let Some(assets_dir) = assets_dir {
        assets_dir
    } else {
        configured_assets_dir = read_workspace_config(root)?.config.assets_dir;
        configured_assets_dir.as_str()
    };
    let asset_root_relative = normalize_relative(selected_assets_dir)?;
    if base64_data.len() as u64 > (MAX_ASSET_BYTES * 4 / 3) + 8 {
        return Err(AppError::Message(
            "The image is too large to store in Marktree.".to_owned(),
        ));
    }
    let bytes = STANDARD
        .decode(base64_data)
        .map_err(|error| AppError::Message(format!("Invalid asset data: {error}")))?;
    if bytes.len() as u64 > MAX_ASSET_BYTES {
        return Err(AppError::Message(
            "The image is too large to store in Marktree.".to_owned(),
        ));
    }
    let sha256 = hash_bytes(&bytes);
    let extension = supported_image_extension(Path::new(file_name))
        .ok_or_else(|| AppError::Message("Unsupported image type.".to_owned()))?;
    let stored_name = format!("{}.{}", &sha256[..24], extension);
    let relative_asset = format!(
        "{}/{}",
        asset_root_relative.trim_end_matches('/'),
        stored_name
    );
    let asset_path = resolve_for_write(&root_path, &relative_asset)?;
    if let Some(parent) = asset_path.parent() {
        fs::create_dir_all(parent)?;
    }
    if git::has_git_capability(root) {
        app_state.record_workspace_change(
            root,
            &relative_asset,
            WorkspaceChangeOperation::Upsert,
            Some(&sha256),
        )?;
    }
    if !asset_path.exists() || fs::read(&asset_path)? != bytes {
        atomic_write(&asset_path, &bytes)?;
    }

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
