use std::path::Path;

use tauri::AppHandle;

use crate::error::{AppError, AppResult};

#[derive(Debug, Clone)]
pub(crate) struct IncomingShare {
    pub text: Option<String>,
    pub subject: Option<String>,
    pub file_path: Option<String>,
    pub file_name: Option<String>,
    pub media_type: Option<String>,
}

#[cfg(target_os = "android")]
pub(crate) fn take_pending_share(app: &AppHandle) -> AppResult<Option<IncomingShare>> {
    use marktree_android_bridge::AndroidBridgeExt;

    app.android_bridge()
        .take_pending_share()
        .map(|share| {
            share.map(|share| IncomingShare {
                text: share.text,
                subject: share.subject,
                file_path: share.file_path,
                file_name: share.file_name,
                media_type: share.media_type,
            })
        })
        .map_err(AppError::Message)
}

#[cfg(not(target_os = "android"))]
pub(crate) fn take_pending_share(_app: &AppHandle) -> AppResult<Option<IncomingShare>> {
    Ok(None)
}

#[cfg(target_os = "android")]
pub(crate) fn share_file(
    app: &AppHandle,
    path: &Path,
    media_type: &str,
    title: &str,
) -> AppResult<()> {
    use marktree_android_bridge::AndroidBridgeExt;

    app.android_bridge()
        .share_file(&path.to_string_lossy(), media_type, title)
        .map_err(AppError::Message)
}

#[cfg(not(target_os = "android"))]
pub(crate) fn share_file(
    _app: &AppHandle,
    _path: &Path,
    _media_type: &str,
    _title: &str,
) -> AppResult<()> {
    Err(AppError::Message(
        "System sharing is available only on Android.".to_owned(),
    ))
}
