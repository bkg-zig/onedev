//! Database Migrations

use crate::{DbError, DbPool, DbResult};
use sqlx::migrate::Migrator;
use std::path::Path;

/// Migrations embedded into the binary at compile time.
///
/// The path is relative to this crate's manifest directory. File names must
/// follow the sqlx convention `<integer version>_<description>.sql`
/// (e.g. `0001_init.sql`), otherwise the macro fails at compile time.
pub static MIGRATOR: Migrator = sqlx::migrate!("./migrations");

/// Run the embedded migrations (independent of the working directory)
pub async fn run_migrations(pool: &DbPool) -> DbResult<()> {
    MIGRATOR.run(pool).await.map_err(DbError::Migration)?;

    Ok(())
}

/// Run migrations from embedded migrations (for production)
///
/// Alias of [`run_migrations`], kept for API compatibility.
pub async fn run_embedded_migrations(pool: &DbPool) -> DbResult<()> {
    run_migrations(pool).await
}

/// Run migrations read from a directory at runtime (e.g. during development)
pub async fn run_migrations_from_dir(pool: &DbPool, dir: impl AsRef<Path>) -> DbResult<()> {
    let migrator = Migrator::new(dir.as_ref())
        .await
        .map_err(DbError::Migration)?;

    migrator.run(pool).await.map_err(DbError::Migration)?;

    Ok(())
}

/// Get migration version
pub async fn get_migration_version(pool: &DbPool) -> DbResult<Option<i64>> {
    let version: Option<i64> =
        sqlx::query_scalar("SELECT version FROM _sqlx_migrations ORDER BY version DESC LIMIT 1")
            .fetch_optional(pool)
            .await
            .map_err(DbError::Connection)?;

    Ok(version)
}

/// Check if all embedded migrations have been applied
pub async fn check_migrations(pool: &DbPool) -> DbResult<bool> {
    let applied =
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM _sqlx_migrations WHERE success")
            .fetch_one(pool)
            .await
            .map_err(DbError::Connection)?;

    let expected = MIGRATOR
        .iter()
        .filter(|m| !m.migration_type.is_down_migration())
        .count() as i64;

    Ok(applied >= expected)
}
