use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AppErrorCode {
    RuntimeUnavailable,
    RuntimeUnhealthy,
    BackendUnreachable,
    CredentialMismatch,
    LibraryRootOffline,
    LibraryRootUnreadable,
    MediaUnsupported,
    MediaMissing,
    StorageFull,
    NotFound,
    InvalidRequest,
    Internal,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppError {
    pub code: AppErrorCode,
    pub message: String,
    pub retryable: bool,
}

impl AppError {
    pub fn new(code: AppErrorCode, message: impl Into<String>, retryable: bool) -> Self {
        Self {
            code,
            message: message.into(),
            retryable,
        }
    }
}

impl std::fmt::Display for AppErrorCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            AppErrorCode::RuntimeUnavailable => "runtime_unavailable",
            AppErrorCode::RuntimeUnhealthy => "runtime_unhealthy",
            AppErrorCode::BackendUnreachable => "backend_unreachable",
            AppErrorCode::CredentialMismatch => "credential_mismatch",
            AppErrorCode::LibraryRootOffline => "library_root_offline",
            AppErrorCode::LibraryRootUnreadable => "library_root_unreadable",
            AppErrorCode::MediaUnsupported => "media_unsupported",
            AppErrorCode::MediaMissing => "media_missing",
            AppErrorCode::StorageFull => "storage_full",
            AppErrorCode::NotFound => "not_found",
            AppErrorCode::InvalidRequest => "invalid_request",
            AppErrorCode::Internal => "internal",
        };
        write!(f, "{}", s)
    }
}

impl std::fmt::Display for AppError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}

impl std::error::Error for AppError {}
