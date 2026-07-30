use tauri::State;

use crate::{
    auth,
    error::AppResult,
    git,
    state::PersistentState,
    types::{CredentialInput, GithubDeviceCode, GithubDeviceToken},
};

#[tauri::command(async)]
pub fn save_credential(input: CredentialInput) -> AppResult<()> {
    auth::save_credential(input)
}

#[tauri::command(async)]
pub fn set_workspace_git_credential(
    root: String,
    credential_id: String,
    state: State<'_, PersistentState>,
) -> AppResult<()> {
    auth::load_credential(&credential_id)?;
    state.set_credential_ref(&git::repository_lock_key(&root), &credential_id)
}

#[tauri::command(async)]
pub fn auth_configuration() -> auth::AuthConfiguration {
    auth::configuration()
}

#[tauri::command(async)]
pub async fn begin_github_device_flow() -> AppResult<GithubDeviceCode> {
    auth::begin_github_device_flow(&auth::configured_github_client_id()).await
}

#[tauri::command(async)]
pub async fn poll_github_device_flow(device_code: String) -> AppResult<GithubDeviceToken> {
    auth::poll_github_device_flow(&auth::configured_github_client_id(), &device_code).await
}
