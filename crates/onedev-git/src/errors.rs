//! Error and result types for the Git layer.

use std::path::PathBuf;
use thiserror::Error;

/// Everything that can go wrong in the Git layer.
#[derive(Debug, Error)]
pub enum GitError {
    #[error("repository not found: {0}")]
    NotFound(String),

    #[error("repository already exists: {0}")]
    AlreadyExists(String),

    #[error("invalid project path: {0}")]
    InvalidProjectPath(String),

    #[error("invalid ref name: {0}")]
    InvalidRef(String),

    #[error("invalid revision: {0}")]
    InvalidRevision(String),

    #[error("object error: {0}")]
    Object(String),

    #[error("permission denied: {0}")]
    PermissionDenied(String),

    #[error("push rejected: {0}")]
    HookRejected(String),

    #[error("protocol error: {0}")]
    Protocol(String),

    #[error("LFS error: {0}")]
    Lfs(String),

    #[error("unsupported operation: {0}")]
    Unsupported(String),

    #[error("repository is locked by another operation: {0}")]
    Busy(String),

    #[error("`{command}` failed with {status}: {stderr}")]
    CommandFailed {
        command: String,
        status: String,
        stderr: String,
    },

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("{0}")]
    Other(#[from] anyhow::Error),
}

impl GitError {
    pub fn other(err: impl std::error::Error + Send + Sync + 'static) -> Self {
        GitError::Other(anyhow::Error::new(err))
    }

    pub fn msg(message: impl Into<String>) -> Self {
        GitError::Other(anyhow::anyhow!(message.into()))
    }

    /// Path that caused the error, when applicable.
    pub fn path(path: &PathBuf, message: impl Into<String>) -> Self {
        GitError::Other(anyhow::anyhow!("{}: {}", path.display(), message.into()))
    }
}

pub type GitResult<T> = std::result::Result<T, GitError>;
