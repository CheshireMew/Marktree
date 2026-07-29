use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("{0}")]
    Message(String),
    #[error("Git operation failed: {0}")]
    Git(#[from] git2::Error),
    #[error("File operation failed: {0}")]
    Io(#[from] std::io::Error),
    #[error("Invalid path: {0}")]
    InvalidPath(String),
    #[error("File not found: {path}")]
    FileNotFound { path: String },
    #[error("The file changed outside Marktree.")]
    ExternalChange,
    #[error("The saved Marktree content changed before synchronization: {path}")]
    ManagedContentChanged { path: String },
    #[error("A Git operation is already in progress for this repository: {root}")]
    GitOperationPending { root: String },
    #[error("Credential operation failed: {0}")]
    Credential(String),
    #[error("Network operation failed: {0}")]
    Network(String),
    #[error("File watcher failed: {0}")]
    Watch(String),
    #[error("Serialization failed: {0}")]
    Serialization(#[from] serde_json::Error),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ErrorCode {
    OperationFailed,
    GitFailed,
    FileFailed,
    InvalidPath,
    FileNotFound,
    ExternalChange,
    ManagedContentChanged,
    GitOperationPending,
    CredentialFailed,
    NetworkFailed,
    WatchFailed,
    SerializationFailed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ErrorPayload {
    pub code: ErrorCode,
    pub message: String,
}

impl AppError {
    pub fn code(&self) -> ErrorCode {
        match self {
            Self::Message(_) => ErrorCode::OperationFailed,
            Self::Git(_) => ErrorCode::GitFailed,
            Self::Io(_) => ErrorCode::FileFailed,
            Self::InvalidPath(_) => ErrorCode::InvalidPath,
            Self::FileNotFound { .. } => ErrorCode::FileNotFound,
            Self::ExternalChange => ErrorCode::ExternalChange,
            Self::ManagedContentChanged { .. } => ErrorCode::ManagedContentChanged,
            Self::GitOperationPending { .. } => ErrorCode::GitOperationPending,
            Self::Credential(_) => ErrorCode::CredentialFailed,
            Self::Network(_) => ErrorCode::NetworkFailed,
            Self::Watch(_) => ErrorCode::WatchFailed,
            Self::Serialization(_) => ErrorCode::SerializationFailed,
        }
    }

    pub fn payload(&self) -> ErrorPayload {
        ErrorPayload {
            code: self.code(),
            message: self.to_string(),
        }
    }
}

impl Serialize for AppError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        self.payload().serialize(serializer)
    }
}

pub type AppResult<T> = Result<T, AppError>;

impl From<tauri::Error> for AppError {
    fn from(value: tauri::Error) -> Self {
        Self::Message(value.to_string())
    }
}

impl From<reqwest::Error> for AppError {
    fn from(value: reqwest::Error) -> Self {
        Self::Network(value.to_string())
    }
}
