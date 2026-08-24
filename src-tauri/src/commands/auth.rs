use tauri::{AppHandle, Manager};

use super::support::run_blocking;
use crate::{
    auth,
    error::AppResult,
    git,
    state::PersistentState,
    types::{CredentialInput, GithubDeviceCode, GithubDeviceToken},
};

#[tauri::command]
pub async fn save_credential(input: CredentialInput) -> AppResult<()> {
    run_blocking(move || auth::save_credential(input)).await
}

#[tauri::command]
pub async fn set_workspace_git_credential(
    root: String,
    credential_id: String,
    app: AppHandle,
) -> AppResult<()> {
    run_blocking(move || {
        auth::load_credential(&credential_id)?;
        app.state::<PersistentState>()
            .set_credential_ref(&git::repository_lock_key(&root), &credential_id)
    })
    .await
}

#[tauri::command]
pub async fn auth_configuration() -> auth::AuthConfiguration {
    auth::configuration()
}

#[tauri::command]
pub async fn begin_github_device_flow() -> AppResult<GithubDeviceCode> {
    auth::begin_github_device_flow(&auth::configured_github_client_id()).await
}

#[tauri::command]
pub async fn poll_github_device_flow(device_code: String) -> AppResult<GithubDeviceToken> {
    auth::poll_github_device_flow(&auth::configured_github_client_id(), &device_code).await
}
