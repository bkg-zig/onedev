//! Database errors

use thiserror::Error;

#[derive(Debug, Error)]
pub enum DbError {
    #[error("Database connection error: {0}")]
    Connection(#[from] sqlx::Error),

    #[error("Migration error: {0}")]
    Migration(#[from] sqlx::migrate::MigrateError),

    #[error("Entity not found: {entity_type} with id {id}")]
    NotFound { entity_type: String, id: String },

    #[error("Duplicate key violation: {0}")]
    DuplicateKey(String),

    #[error("Foreign key violation: {0}")]
    ForeignKeyViolation(String),

    #[error("Constraint violation: {0}")]
    ConstraintViolation(String),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("Pool error: {0}")]
    Pool(String),

    #[error("Transaction error: {0}")]
    Transaction(String),

    #[error("Invalid input: {0}")]
    InvalidInput(String),

    #[error("Configuration error: {0}")]
    Config(#[from] config::ConfigError),

    #[error("Parse error: {0}")]
    ParseError(String),

    #[error("Query error: {0}")]
    Query(String),
}

pub type DbResult<T> = Result<T, DbError>;

// No manual `impl From<DbError> for anyhow::Error`: `DbError` implements
// `std::error::Error + Send + Sync + 'static`, so anyhow's blanket
// `impl<E> From<E> for anyhow::Error` already covers it (a manual impl would
// conflict, E0119).
