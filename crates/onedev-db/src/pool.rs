//! Database Connection Pool

use crate::{DbError, DbResult};
use deadpool_postgres::{Config, Pool, Runtime};
use serde::Deserialize;
use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
use sqlx::{Pool as SqlxPool, Postgres};
use std::fmt;
use std::time::Duration;
use tokio_postgres::NoTls;
use tracing::info;

/// Database configuration
///
/// Can be built from the environment ([`DbConfig::from_env`]) or loaded from a
/// TOML file ([`DbConfig::from_file`]). Durations are given in whole seconds in
/// the file; missing keys fall back to [`DbConfig::default`].
#[derive(Clone, Deserialize)]
#[serde(default)]
pub struct DbConfig {
    pub host: String,
    pub port: u16,
    pub username: String,
    pub password: String,
    pub database: String,
    pub max_connections: u32,
    pub min_connections: u32,
    #[serde(deserialize_with = "deserialize_secs")]
    pub acquire_timeout: Duration,
    #[serde(deserialize_with = "deserialize_secs")]
    pub idle_timeout: Duration,
    #[serde(deserialize_with = "deserialize_secs")]
    pub max_lifetime: Duration,
}

/// Deserialize a `Duration` from a whole number of seconds
fn deserialize_secs<'de, D>(deserializer: D) -> Result<Duration, D::Error>
where
    D: serde::Deserializer<'de>,
{
    u64::deserialize(deserializer).map(Duration::from_secs)
}

// Manual `Debug` so that the password never ends up in logs.
impl fmt::Debug for DbConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DbConfig")
            .field("host", &self.host)
            .field("port", &self.port)
            .field("username", &self.username)
            .field("password", &"<redacted>")
            .field("database", &self.database)
            .field("max_connections", &self.max_connections)
            .field("min_connections", &self.min_connections)
            .field("acquire_timeout", &self.acquire_timeout)
            .field("idle_timeout", &self.idle_timeout)
            .field("max_lifetime", &self.max_lifetime)
            .finish()
    }
}

impl Default for DbConfig {
    fn default() -> Self {
        Self {
            host: "localhost".to_string(),
            port: 5432,
            username: "onedev".to_string(),
            password: "onedev".to_string(),
            database: "onedev".to_string(),
            max_connections: 20,
            min_connections: 5,
            acquire_timeout: Duration::from_secs(30),
            idle_timeout: Duration::from_secs(600),
            max_lifetime: Duration::from_secs(1800),
        }
    }
}

impl DbConfig {
    /// Load configuration from environment
    pub fn from_env() -> DbResult<Self> {
        let host = std::env::var("DB_HOST").unwrap_or_else(|_| "localhost".to_string());
        let port = std::env::var("DB_PORT").unwrap_or_else(|_| "5432".to_string()).parse().unwrap_or(5432);
        let username = std::env::var("DB_USER").unwrap_or_else(|_| "onedev".to_string());
        let password = std::env::var("DB_PASSWORD").unwrap_or_else(|_| "onedev".to_string());
        let database = std::env::var("DB_NAME").unwrap_or_else(|_| "onedev".to_string());
        let max_connections = std::env::var("DB_MAX_CONNECTIONS").unwrap_or_else(|_| "20".to_string()).parse().unwrap_or(20);
        let min_connections = std::env::var("DB_MIN_CONNECTIONS").unwrap_or_else(|_| "5".to_string()).parse().unwrap_or(5);

        Ok(Self {
            host,
            port,
            username,
            password,
            database,
            max_connections,
            min_connections,
            acquire_timeout: Duration::from_secs(30),
            idle_timeout: Duration::from_secs(600),
            max_lifetime: Duration::from_secs(1800),
        })
    }

    /// Load configuration from a TOML file (for production)
    pub fn from_file(path: &str) -> DbResult<Self> {
        let settings = config::Config::builder()
            .add_source(config::File::new(path, config::FileFormat::Toml))
            .build()?;
        Ok(settings.try_deserialize()?)
    }

    /// Get connection string for sqlx
    ///
    /// Note: credentials are not percent-encoded; prefer
    /// [`DbConfig::sqlx_connect_options`] when they may contain special characters.
    pub fn sqlx_connection_string(&self) -> String {
        format!(
            "postgres://{}:{}@{}:{}/{}",
            self.username, self.password, self.host, self.port, self.database
        )
    }

    /// Get typed connect options for sqlx (no URL encoding of credentials needed)
    pub fn sqlx_connect_options(&self) -> PgConnectOptions {
        PgConnectOptions::new()
            .host(&self.host)
            .port(self.port)
            .username(&self.username)
            .password(&self.password)
            .database(&self.database)
    }

    /// Get connection configuration for deadpool
    pub fn deadpool_config(&self) -> Config {
        let mut cfg = Config::new();
        cfg.host = Some(self.host.clone());
        cfg.port = Some(self.port);
        cfg.user = Some(self.username.clone());
        cfg.password = Some(self.password.clone());
        cfg.dbname = Some(self.database.clone());
        cfg
    }
}

/// Create SQLx connection pool
pub async fn create_sqlx_pool(config: &DbConfig) -> DbResult<SqlxPool<Postgres>> {
    let pool = PgPoolOptions::new()
        .max_connections(config.max_connections)
        .min_connections(config.min_connections)
        .acquire_timeout(config.acquire_timeout)
        .idle_timeout(config.idle_timeout)
        .max_lifetime(config.max_lifetime)
        .connect_with(config.sqlx_connect_options())
        .await
        .map_err(DbError::Connection)?;

    // Test connection
    sqlx::query("SELECT 1")
        .fetch_one(&pool)
        .await
        .map_err(DbError::Connection)?;

    info!("SQLx connection pool created successfully");
    Ok(pool)
}

/// Create deadpool connection pool
pub async fn create_deadpool_pool(config: &DbConfig) -> DbResult<Pool> {
    let pool = config.deadpool_config()
        .create_pool(Some(Runtime::Tokio1), NoTls)
        .map_err(|e| DbError::Pool(e.to_string()))?;

    // Test connection
    let client = pool.get().await.map_err(|e| DbError::Pool(e.to_string()))?;
    client.execute("SELECT 1", &[]).await.map_err(|e| DbError::Pool(e.to_string()))?;

    info!("Deadpool connection pool created successfully");
    Ok(pool)
}

/// Type alias for SQLx pool
pub type DbPool = SqlxPool<Postgres>;

// Migrations live in `crate::migrations` (`run_migrations`, `run_embedded_migrations`, ...).

/// Check database health
pub async fn check_health(pool: &DbPool) -> DbResult<()> {
    sqlx::query("SELECT 1")
        .fetch_one(pool)
        .await
        .map_err(DbError::Connection)?;
    Ok(())
}

/// Snapshot of the SQLx pool state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DbPoolStats {
    /// Open connections (idle + in use)
    pub size: u32,
    /// Idle connections
    pub idle: usize,
    /// Configured maximum number of connections
    pub max_connections: u32,
}

/// Get pool statistics
pub fn pool_stats(pool: &DbPool) -> DbPoolStats {
    DbPoolStats {
        size: pool.size(),
        idle: pool.num_idle(),
        max_connections: pool.options().get_max_connections(),
    }
}

/// Close pool gracefully
pub async fn close_pool(pool: DbPool) {
    pool.close().await;
    info!("Database connection pool closed");
}
