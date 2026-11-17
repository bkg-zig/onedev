//! Database Connection Pool

use crate::{DbError, DbResult};
use deadpool_postgres::{Config, Pool, Runtime};
use sqlx::postgres::PgPoolOptions;
use sqlx::{Pool as SqlxPool, Postgres};
use std::time::Duration;
use tokio_postgres::NoTls;
use tracing::{info, warn};

/// Database configuration
#[derive(Debug, Clone)]
pub struct DbConfig {
    pub host: String,
    pub port: u16,
    pub username: String,
    pub password: String,
    pub database: String,
    pub max_connections: u32,
    pub min_connections: u32,
    pub acquire_timeout: Duration,
    pub idle_timeout: Duration,
    pub max_lifetime: Duration,
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

    /// Load configuration from file (for production)
    pub fn from_file(path: &str) -> DbResult<Self> {
        let content = std::fs::read_to_string(path)?;
        let config: DbConfig = toml::from_str(&content)?;
        Ok(config)
    }

    /// Get connection string for sqlx
    pub fn sqlx_connection_string(&self) -> String {
        format!(
            "postgres://{}:{}@{}:{}/{}",
            self.username, self.password, self.host, self.port, self.database
        )
    }

    /// Get connection string for deadpool
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
        .connect(&config.sqlx_connection_string())
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

/// Run database migrations
pub async fn run_migrations(pool: &DbPool) -> DbResult<()> {
    info!("Running database migrations...");

    // Run SQLx migrations
    sqlx::migrate!("./migrations")
        .run(pool)
        .await
        .map_err(DbError::Migration)?;

    info!("Database migrations completed successfully");
    Ok(())
}

/// Check database health
pub async fn check_health(pool: &DbPool) -> DbResult<()> {
    sqlx::query("SELECT 1")
        .fetch_one(pool)
        .await
        .map_err(DbError::Connection)?;
    Ok(())
}

/// Get pool statistics
pub fn pool_stats(pool: &DbPool) -> sqlx::pool::PoolStats {
    pool.stats()
}

/// Close pool gracefully
pub async fn close_pool(pool: DbPool) {
    pool.close().await;
    info!("Database connection pool closed");
}