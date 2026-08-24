use std::{
    collections::HashMap,
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

use base64::{engine::general_purpose::STANDARD, Engine};
use parking_lot::Mutex;

use crate::{
    content_policy::supported_image_extension,
    error::{AppError, AppResult},
    file_version::hash_bytes,
    paths::{canonical_root, normalize_content_relative},
    types::{AssetUploadChunkRequest, AssetUploadTicket, BeginAssetUploadRequest},
};

const MAX_CHUNK_BYTES: usize = 512 * 1024;
static UPLOAD_SEQUENCE: AtomicU64 = AtomicU64::new(1);

#[derive(Clone)]
pub(crate) struct AssetUpload {
    pub(crate) root: String,
    pub(crate) document_path: String,
    pub(crate) document_sha256: String,
    pub(crate) file_name: String,
    pub(crate) assets_dir: Option<String>,
    pub(crate) source_path: PathBuf,
    total_bytes: u64,
    written_bytes: u64,
}

pub(crate) struct AssetUploadRuntime {
    directory: PathBuf,
    uploads: Mutex<HashMap<String, AssetUpload>>,
}

impl AssetUploadRuntime {
    pub(crate) fn new(app_cache_dir: &Path) -> AppResult<Self> {
        let directory = app_cache_dir.join("asset-uploads");
        if directory.exists() {
            fs::remove_dir_all(&directory)?;
        }
        fs::create_dir_all(&directory)?;
        Ok(Self {
            directory,
            uploads: Mutex::new(HashMap::new()),
        })
    }

    pub(crate) fn begin(
        &self,
        request: BeginAssetUploadRequest,
        document_sha256: String,
    ) -> AppResult<AssetUploadTicket> {
        canonical_root(&request.root)?;
        normalize_content_relative(&request.document_path)?;
        if supported_image_extension(Path::new(&request.file_name)).is_none() {
            return Err(AppError::Message("Unsupported image type.".to_owned()));
        }
        if request.total_bytes > crate::documents::MAX_ASSET_BYTES {
            return Err(AppError::Message(
                "The image is too large to store in Marktree.".to_owned(),
            ));
        }
        let seed = format!(
            "{}\n{}\n{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos(),
            UPLOAD_SEQUENCE.fetch_add(1, Ordering::Relaxed)
        );
        let id = hash_bytes(seed.as_bytes())[..24].to_owned();
        let source_path = self.directory.join(format!("{id}.upload"));
        OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&source_path)?;
        self.uploads.lock().insert(
            id.clone(),
            AssetUpload {
                root: request.root,
                document_path: request.document_path,
                document_sha256,
                file_name: request.file_name,
                assets_dir: request.assets_dir,
                source_path,
                total_bytes: request.total_bytes,
                written_bytes: 0,
            },
        );
        Ok(AssetUploadTicket {
            id,
            max_chunk_bytes: MAX_CHUNK_BYTES,
        })
    }

    pub(crate) fn append(&self, request: AssetUploadChunkRequest) -> AppResult<()> {
        if request.base64_data.len() > (MAX_CHUNK_BYTES * 4 / 3) + 8 {
            return Err(AppError::Message(
                "The asset upload chunk is too large.".to_owned(),
            ));
        }
        let bytes = STANDARD
            .decode(request.base64_data)
            .map_err(|error| AppError::Message(format!("Invalid asset data: {error}")))?;
        if bytes.len() > MAX_CHUNK_BYTES {
            return Err(AppError::Message(
                "The asset upload chunk is too large.".to_owned(),
            ));
        }
        let mut uploads = self.uploads.lock();
        let upload = uploads.get_mut(&request.upload_id).ok_or_else(|| {
            AppError::Message("The asset upload is no longer available.".to_owned())
        })?;
        if request.offset != upload.written_bytes
            || upload.written_bytes.saturating_add(bytes.len() as u64) > upload.total_bytes
        {
            return Err(AppError::Message(
                "The asset upload arrived out of order or exceeded its declared size.".to_owned(),
            ));
        }
        let mut file = OpenOptions::new().append(true).open(&upload.source_path)?;
        file.write_all(&bytes)?;
        upload.written_bytes += bytes.len() as u64;
        Ok(())
    }

    pub(crate) fn completed(&self, id: &str) -> AppResult<AssetUpload> {
        let uploads = self.uploads.lock();
        let upload = uploads.get(id).ok_or_else(|| {
            AppError::Message("The asset upload is no longer available.".to_owned())
        })?;
        if upload.written_bytes != upload.total_bytes {
            return Err(AppError::Message(
                "The asset upload is incomplete.".to_owned(),
            ));
        }
        OpenOptions::new()
            .write(true)
            .open(&upload.source_path)?
            .sync_all()?;
        Ok(upload.clone())
    }

    pub(crate) fn finish(&self, id: &str) {
        if let Some(upload) = self.uploads.lock().remove(id) {
            let _ = fs::remove_file(upload.source_path);
        }
    }

    pub(crate) fn abort(&self, id: &str) {
        self.finish(id);
    }
}

#[cfg(test)]
mod tests {
    use tempfile::TempDir;

    use super::*;

    #[test]
    fn chunked_upload_preserves_bytes_and_rejects_out_of_order_data() {
        let cache = TempDir::new().unwrap();
        let workspace = TempDir::new().unwrap();
        let runtime = AssetUploadRuntime::new(cache.path()).unwrap();
        let ticket = runtime
            .begin(
                BeginAssetUploadRequest {
                    root: workspace.path().to_string_lossy().into_owned(),
                    document_path: "note.md".to_owned(),
                    file_name: "image.png".to_owned(),
                    assets_dir: None,
                    total_bytes: 6,
                },
                "document-version".to_owned(),
            )
            .unwrap();
        assert_eq!(ticket.max_chunk_bytes, MAX_CHUNK_BYTES);

        runtime
            .append(AssetUploadChunkRequest {
                upload_id: ticket.id.clone(),
                offset: 0,
                base64_data: STANDARD.encode(b"abc"),
            })
            .unwrap();
        assert!(runtime
            .append(AssetUploadChunkRequest {
                upload_id: ticket.id.clone(),
                offset: 1,
                base64_data: STANDARD.encode(b"bad"),
            })
            .is_err());
        runtime
            .append(AssetUploadChunkRequest {
                upload_id: ticket.id.clone(),
                offset: 3,
                base64_data: STANDARD.encode(b"def"),
            })
            .unwrap();

        let upload = runtime.completed(&ticket.id).unwrap();
        assert_eq!(fs::read(&upload.source_path).unwrap(), b"abcdef");
        assert_eq!(upload.document_sha256, "document-version");
        runtime.finish(&ticket.id);
        assert!(!upload.source_path.exists());
    }
}
