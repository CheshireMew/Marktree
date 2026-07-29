use std::sync::OnceLock;

use serde::{Deserialize, Serialize};

use crate::{
    error::{AppError, AppResult},
    types::{CredentialInput, CredentialRecord, GithubDeviceCode, GithubDeviceToken},
};

const CREDENTIAL_SERVICE: &str = "io.github.cheshiremew.marktree";
static KEYRING_READY: OnceLock<Result<(), String>> = OnceLock::new();

pub fn save_credential(input: CredentialInput) -> AppResult<()> {
    ensure_keyring()?;
    let entry = keyring_core::Entry::new(CREDENTIAL_SERVICE, &input.id)
        .map_err(|error| AppError::Credential(error.to_string()))?;
    let payload = serde_json::to_vec(&CredentialRecord {
        username: input.username,
        token: input.token,
    })?;
    entry
        .set_secret(&payload)
        .map_err(|error| AppError::Credential(error.to_string()))
}

pub fn load_credential(id: &str) -> AppResult<CredentialRecord> {
    ensure_keyring()?;
    let entry = keyring_core::Entry::new(CREDENTIAL_SERVICE, id)
        .map_err(|error| AppError::Credential(error.to_string()))?;
    let secret = entry
        .get_secret()
        .map_err(|error| AppError::Credential(error.to_string()))?;
    Ok(serde_json::from_slice(&secret)?)
}

pub async fn begin_github_device_flow(client_id: &str) -> AppResult<GithubDeviceCode> {
    if client_id.trim().is_empty() {
        return Err(AppError::Message(
            "GitHub OAuth client ID is not configured.".to_owned(),
        ));
    }
    let response = reqwest::Client::new()
        .post("https://github.com/login/device/code")
        .header("Accept", "application/json")
        .form(&[("client_id", client_id), ("scope", "repo read:user")])
        .send()
        .await?
        .error_for_status()?
        .json::<GithubDeviceCode>()
        .await?;
    Ok(response)
}

pub async fn poll_github_device_flow(
    client_id: &str,
    device_code: &str,
) -> AppResult<GithubDeviceToken> {
    #[derive(Debug, Deserialize)]
    struct TokenResponse {
        access_token: Option<String>,
        token_type: Option<String>,
        scope: Option<String>,
        error: Option<String>,
        error_description: Option<String>,
    }

    let response = reqwest::Client::new()
        .post("https://github.com/login/oauth/access_token")
        .header("Accept", "application/json")
        .form(&[
            ("client_id", client_id),
            ("device_code", device_code),
            ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
        ])
        .send()
        .await?
        .error_for_status()?
        .json::<TokenResponse>()
        .await?;
    let pending = matches!(
        response.error.as_deref(),
        Some("authorization_pending" | "slow_down")
    );
    Ok(GithubDeviceToken {
        access_token: response.access_token,
        token_type: response.token_type,
        scope: response.scope,
        pending,
        error: if pending {
            None
        } else {
            response.error_description.or(response.error)
        },
    })
}

pub fn configured_github_client_id() -> String {
    option_env!("MARKTREE_GITHUB_CLIENT_ID")
        .unwrap_or_default()
        .to_owned()
}

fn ensure_keyring() -> AppResult<()> {
    KEYRING_READY
        .get_or_init(configure_keyring)
        .clone()
        .map_err(AppError::Credential)
}

fn configure_keyring() -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        let store =
            windows_native_keyring_store::Store::new().map_err(|error| error.to_string())?;
        keyring_core::set_default_store(store);
        return Ok(());
    }

    #[cfg(target_os = "android")]
    {
        let store =
            android_native_keyring_store::Store::new().map_err(|error| error.to_string())?;
        keyring_core::set_default_store(store);
        return Ok(());
    }

    #[allow(unreachable_code)]
    Err("Marktree credential storage supports Windows and Android only.".to_owned())
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthConfiguration {
    pub github_client_id: String,
    pub github_enabled: bool,
}

pub fn configuration() -> AuthConfiguration {
    let github_client_id = configured_github_client_id();
    AuthConfiguration {
        github_enabled: !github_client_id.is_empty(),
        github_client_id,
    }
}
