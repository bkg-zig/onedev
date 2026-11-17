//! Core error types shared across the OneDev crates.

use thiserror::Error;

/// Errors raised by the domain layer.
#[derive(Debug, Error)]
pub enum CoreError {
    #[error("{entity} not found: {id}")]
    NotFound { entity: &'static str, id: String },

    #[error("validation failed: {0}")]
    Validation(String),

    #[error("conflict: {0}")]
    Conflict(String),

    #[error("permission denied: {0}")]
    PermissionDenied(String),

    #[error("invalid state: {0}")]
    InvalidState(String),

    #[error("serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("internal error: {0}")]
    Internal(String),
}

impl From<validator::ValidationErrors> for CoreError {
    fn from(errors: validator::ValidationErrors) -> Self {
        CoreError::Validation(errors.to_string())
    }
}

/// Result alias for domain operations.
pub type CoreResult<T> = std::result::Result<T, CoreError>;
