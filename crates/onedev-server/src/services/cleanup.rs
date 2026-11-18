//! Cleanup Service

use anyhow::Result;
use chrono::{DateTime, Utc};
use sqlx::PgPool;
use std::sync::Arc;
use tokio::sync::broadcast;
use tracing::{info, warn, error, debug};
use uuid::Uuid;

/// Cleanup service configuration
#[derive(Debug, Clone)]
pub struct CleanupConfig {
    pub build_log_retention_days: i64,
    pub artifact_retention_days: i64,
    pub temp_file_retention_hours: i64,
    pub audit_log_retention_days: i64,
    pub webhook_log_retention_days: i64,
    pub search_index_cleanup_days: i64,
    pub run_interval_hours: u64,
}

impl Default for CleanupConfig {
    fn default() -> Self {
        Self {
            build_log_retention_days: 30,
            artifact_retention_days: 90,
            temp_file_retention_hours: 24,
            audit_log_retention_days: 365,
            webhook_log_retention_days: 30,
            search_index_cleanup_days: 7,
            run_interval_hours: 1,
        }
    }
}

/// Cleanup service
pub struct CleanupService {
    db_pool: Arc<PgPool>,
    config: CleanupConfig,
    shutdown_rx: broadcast::Receiver<()>,
}

impl CleanupService {
    /// Create a new cleanup service
    pub fn new(
        db_pool: Arc<PgPool>,
        config: CleanupConfig,
        shutdown_rx: broadcast::Receiver<()>,
    ) -> Self {
        Self {
            db_pool,
            config,
            shutdown_rx,
        }
    }

    /// Run the cleanup service
    pub async fn run(mut self) {
        info!("Cleanup service started");

        let mut interval = tokio::time::interval(
            tokio::time::Duration::from_secs(self.config.run_interval_hours * 3600)
        );

        // Run once at startup
        if let Err(e) = self.run_cleanup().await {
            error!("Initial cleanup failed: {}", e);
        }

        loop {
            tokio::select! {
                _ = interval.tick() => {
                    if let Err(e) = self.run_cleanup().await {
                        error!("Cleanup run failed: {}", e);
                    }
                }
                _ = self.shutdown_rx.recv() => {
                    info!("Cleanup service received shutdown signal");
                    break;
                }
            }
        }

        info!("Cleanup service stopped");
    }

    /// Run all cleanup tasks
    async fn run_cleanup(&self) -> Result<()> {
        info!("Starting cleanup tasks...");

        // Clean old build logs
        self.cleanup_build_logs().await?;

        // Clean old artifacts
        self.cleanup_artifacts().await?;

        // Clean temp files
        self.cleanup_temp_files().await?;

        // Clean audit logs
        self.cleanup_audit_logs().await?;

        // Clean webhook logs
        self.cleanup_webhook_logs().await?;

        // Clean search index
        self.cleanup_search_index().await?;

        info!("Cleanup tasks completed");
        Ok(())
    }

    /// Clean old build logs
    async fn cleanup_build_logs(&self) -> Result<()> {
        let cutoff = Utc::now() - chrono::Duration::days(self.config.build_log_retention_days);

        let result = sqlx::query(
            r#"
            UPDATE builds
            SET log = NULL
            WHERE status IN ('Success', 'Failed', 'Cancelled')
            AND finished_at < $1
            AND log IS NOT NULL
            "#
        )
            .bind(cutoff)
            .execute(&self.db_pool).await?;

        if result.rows_affected() > 0 {
            info!("Cleaned build logs for {} builds older than {} days",
                result.rows_affected(), self.config.build_log_retention_days);
        }

        Ok(())
    }

    /// Clean old artifacts
    async fn cleanup_artifacts(&self) -> Result<()> {
        let cutoff = Utc::now() - chrono::Duration::days(self.config.artifact_retention_days);

        // Get artifacts to delete
        let artifacts = sqlx::query(
            r#"
            SELECT id, path FROM build_artifacts
            WHERE created_at < $1
            "#
        )
            .bind(cutoff)
            .fetch_all(&self.db_pool).await?;

        for artifact in &artifacts {
            let path: String = artifact.try_get("path")?;
            // Try to delete file
            if let Err(e) = tokio::fs::remove_file(&path).await {
                debug!("Failed to delete artifact file {}: {}", path, e);
            }
        }

        // Delete records
        let result = sqlx::query("DELETE FROM build_artifacts WHERE created_at < $1")
            .bind(cutoff)
            .execute(&self.db_pool).await?;

        if result.rows_affected() > 0 {
            info!("Deleted {} artifact records older than {} days",
                result.rows_affected(), self.config.artifact_retention_days);
        }

        Ok(())
    }

    /// Clean temp files
    async fn cleanup_temp_files(&self) -> Result<()> {
        let cutoff = Utc::now() - chrono::Duration::hours(self.config.temp_file_retention_hours);

        let result = sqlx::query("DELETE FROM temp_files WHERE created_at < $1")
            .bind(cutoff)
            .execute(&self.db_pool).await?;

        if result.rows_affected() > 0 {
            info!("Deleted {} temp file records older than {} hours",
                result.rows_affected(), self.config.temp_file_retention_hours);
        }

        Ok(())
    }

    /// Clean old audit logs
    async fn cleanup_audit_logs(&self) -> Result<()> {
        let cutoff = Utc::now() - chrono::Duration::days(self.config.audit_log_retention_days);

        let result = sqlx::query("DELETE FROM audit_logs WHERE created_at < $1")
            .bind(cutoff)
            .execute(&self.db_pool).await?;

        if result.rows_affected() > 0 {
            info!("Deleted {} audit log records older than {} days",
                result.rows_affected(), self.config.audit_log_retention_days);
        }

        Ok(())
    }

    /// Clean old webhook logs
    async fn cleanup_webhook_logs(&self) -> Result<()> {
        let cutoff = Utc::now() - chrono::Duration::days(self.config.webhook_log_retention_days);

        let result = sqlx::query("DELETE FROM webhook_deliveries WHERE created_at < $1")
            .bind(cutoff)
            .execute(&self.db_pool).await?;

        if result.rows_affected() > 0 {
            info!("Deleted {} webhook delivery records older than {} days",
                result.rows_affected(), self.config.webhook_log_retention_days);
        }

        Ok(())
    }

    /// Clean search index
    async fn cleanup_search_index(&self) -> Result<()> {
        let cutoff = Utc::now() - chrono::Duration::days(self.config.search_index_cleanup_days);

        // Remove index entries for deleted entities
        let result = sqlx::query(
            r#"
            DELETE FROM search_index
            WHERE entity_type IN ('issue', 'project', 'wiki', 'commit')
            AND entity_id NOT IN (
                SELECT id FROM issues WHERE deleted_at IS NULL
                UNION SELECT id FROM projects WHERE deleted_at IS NULL
                UNION SELECT id FROM wiki_pages WHERE deleted_at IS NULL
                UNION SELECT id FROM commits
            )
            AND updated_at < $1
            "#
        )
            .bind(cutoff)
            .execute(&self.db_pool).await?;

        if result.rows_affected() > 0 {
            info!("Cleaned {} stale search index entries", result.rows_affected());
        }

        Ok(())
    }

    /// Manual cleanup trigger
    pub async fn trigger_cleanup(&self) -> Result<()> {
        self.run_cleanup().await
    }
}