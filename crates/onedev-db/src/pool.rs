//! Database Connection Pool

use crate::{DbError, DbResult};
use deadpool_postgres::{Config, Pool, Runtime};
use serde::Deserialize;
use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
use sqlx::{Pool as SqlxPool, Postgres, Row};
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
        let port = std::env::var("DB_PORT")
            .unwrap_or_else(|_| "5432".to_string())
            .parse()
            .unwrap_or(5432);
        let username = std::env::var("DB_USER").unwrap_or_else(|_| "onedev".to_string());
        let password = std::env::var("DB_PASSWORD").unwrap_or_else(|_| "onedev".to_string());
        let database = std::env::var("DB_NAME").unwrap_or_else(|_| "onedev".to_string());
        let max_connections = std::env::var("DB_MAX_CONNECTIONS")
            .unwrap_or_else(|_| "20".to_string())
            .parse()
            .unwrap_or(20);
        let min_connections = std::env::var("DB_MIN_CONNECTIONS")
            .unwrap_or_else(|_| "5".to_string())
            .parse()
            .unwrap_or(5);

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
    let pool = config
        .deadpool_config()
        .create_pool(Some(Runtime::Tokio1), NoTls)
        .map_err(|e| DbError::Pool(e.to_string()))?;

    // Test connection
    let client = pool.get().await.map_err(|e| DbError::Pool(e.to_string()))?;
    client
        .execute("SELECT 1", &[])
        .await
        .map_err(|e| DbError::Pool(e.to_string()))?;

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

/// Alias for create_sqlx_pool for backwards compatibility
pub async fn create_pool(database_url: &str, max_connections: u32) -> DbResult<DbPool> {
    // Parse the database URL to create a DbConfig
    let config = parse_database_url(database_url, max_connections)?;
    create_sqlx_pool(&config).await
}

/// Parse a PostgreSQL connection URL into a DbConfig
fn parse_database_url(url: &str, max_connections: u32) -> DbResult<DbConfig> {
    // Simple parsing of postgres://user:pass@host:port/db
    let url = url
        .strip_prefix("postgres://")
        .or_else(|| url.strip_prefix("postgresql://"))
        .ok_or_else(|| DbError::ParseError("Invalid database URL format".to_string()))?;

    let (auth_part, rest) = url
        .split_once('@')
        .ok_or_else(|| DbError::ParseError("Missing @ in database URL".to_string()))?;

    let (username, password) = auth_part
        .split_once(':')
        .ok_or_else(|| DbError::ParseError("Missing : in auth part".to_string()))?;

    let (host_port, database) = rest
        .split_once('/')
        .ok_or_else(|| DbError::ParseError("Missing / in database URL".to_string()))?;

    let (host, port) = host_port
        .split_once(':')
        .ok_or_else(|| DbError::ParseError("Missing : in host:port".to_string()))?;

    let port: u16 = port
        .parse()
        .map_err(|_| DbError::ParseError("Invalid port number".to_string()))?;

    Ok(DbConfig {
        host: host.to_string(),
        port,
        username: username.to_string(),
        password: password.to_string(),
        database: database.to_string(),
        max_connections,
        min_connections: max_connections / 4,
        acquire_timeout: Duration::from_secs(30),
        idle_timeout: Duration::from_secs(600),
        max_lifetime: Duration::from_secs(1800),
    })
}

/// Cleanup old data (build logs, temp files, etc.)
pub async fn cleanup_old_data(pool: &DbPool) -> DbResult<()> {
    // Cleanup old build logs (older than 30 days)
    sqlx::query("DELETE FROM build_logs WHERE created_at < NOW() - INTERVAL '30 days'")
        .execute(pool)
        .await
        .map_err(|e| DbError::Query(e.to_string()))?;

    // Cleanup old audit logs (older than 365 days)
    sqlx::query("DELETE FROM audit_logs WHERE created_at < NOW() - INTERVAL '365 days'")
        .execute(pool)
        .await
        .map_err(|e| DbError::Query(e.to_string()))?;

    // Cleanup old webhook deliveries (older than 30 days)
    sqlx::query("DELETE FROM webhook_deliveries WHERE created_at < NOW() - INTERVAL '30 days'")
        .execute(pool)
        .await
        .map_err(|e| DbError::Query(e.to_string()))?;

    info!("Old data cleanup completed");
    Ok(())
}

/// Process webhook delivery queue
pub async fn process_webhook_queue(pool: &DbPool) -> DbResult<()> {
    // Get pending webhook deliveries
    let deliveries = sqlx::query(
        r#"
        SELECT id, webhook_id, event, payload, url, attempts
        FROM webhook_deliveries
        WHERE status = 'pending' AND attempts < 5
        ORDER BY created_at
        LIMIT 50
        "#,
    )
    .fetch_all(pool)
    .await
    .map_err(|e| DbError::Query(e.to_string()))?;

    for delivery in deliveries {
        // In a real implementation, this would make HTTP requests to the webhook URL
        // For now, just mark as completed
        // Note: delivery.id is a String from the query, bind it as such
        let id: String = delivery
            .try_get("id")
            .map_err(|e| DbError::Query(e.to_string()))?;
        sqlx::query(
            r#"
            UPDATE webhook_deliveries
            SET status = 'completed', completed_at = NOW(), attempts = attempts + 1
            WHERE id = $1
            "#,
        )
        .bind(&id)
        .execute(pool)
        .await
        .map_err(|e| DbError::Query(e.to_string()))?;
    }

    Ok(())
}

/// Update search index for projects, issues, etc.
pub async fn update_search_index(pool: &DbPool) -> DbResult<()> {
    // Update full-text search vectors for projects
    sqlx::query("UPDATE projects SET search_vector = to_tsvector('english', coalesce(name, '') || ' ' || coalesce(description, '')) WHERE search_vector IS NULL")
        .execute(pool)
        .await
        .map_err(|e| DbError::Query(e.to_string()))?;

    // Update full-text search vectors for issues
    sqlx::query("UPDATE issues SET search_vector = to_tsvector('english', coalesce(title, '') || ' ' || coalesce(description, '')) WHERE search_vector IS NULL")
        .execute(pool)
        .await
        .map_err(|e| DbError::Query(e.to_string()))?;

    // Update full-text search vectors for merge requests
    sqlx::query("UPDATE merge_requests SET search_vector = to_tsvector('english', coalesce(title, '') || ' ' || coalesce(description, '')) WHERE search_vector IS NULL")
        .execute(pool)
        .await
        .map_err(|e| DbError::Query(e.to_string()))?;

    info!("Search index update completed");
    Ok(())
}
