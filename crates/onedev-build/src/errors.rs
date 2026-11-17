//! Error types for the build layer.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum BuildError {
    #[error("invalid build spec: {0}")]
    Spec(String),

    #[error("build spec not found: {0}")]
    NotFound(String),

    #[error("job failed: {0}")]
    JobFailed(String),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("YAML error: {0}")]
    Yaml(#[from] serde_yaml::Error),

    #[error("{0}")]
    Other(#[from] anyhow::Error),
}

pub type BuildResult<T> = std::result::Result<T, BuildError>;
