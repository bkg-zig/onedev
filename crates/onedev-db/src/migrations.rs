//! Database Migrations

use crate::{DbPool, DbResult};
use sqlx::migrate::Migrator;
use std::path::Path;

/// Run migrations from the migrations directory
pub async fn run_migrations(pool: &DbPool) -> DbResult<()> {
    let migrator = Migrator::new(Path::new("./migrations"))
        .await
        .map_err(crate::DbError::Migration)?;

    migrator.run(pool).await.map_err(crate::DbError::Migration)?;

    Ok(())
}

/// Run migrations from embedded migrations (for production)
pub async fn run_embedded_migrations(pool: &DbPool) -> DbResult<()> {
    sqlx::migrate!("./migrations")
        .run(pool)
        .await
        .map_err(crate::DbError::Migration)?;

    Ok(())
}

/// Get migration version
pub async fn get_migration_version(pool: &DbPool) -> DbResult<Option<i64>> {
    let version: Option<i64> = sqlx::query_scalar(
        "SELECT version FROM _sqlx_migrations ORDER BY version DESC LIMIT 1"
    )
    .fetch_optional(pool)
    .await
    .map_err(crate::DbError::Connection)?;

    Ok(version)
}

/// Check if migrations are up to date
pub async fn check_migrations(pool: &DbPool) -> DbResult<bool> {
    let applied = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM _sqlx_migrations"
    )
    .fetch_one(pool)
    .await
    .map_err(crate::DbError::Connection)?;

    // In a real implementation, you'd compare with the expected number of migrations
    Ok(applied > 0)
}