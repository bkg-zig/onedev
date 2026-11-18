//! Git GC (Garbage Collection) Service

use anyhow::Result;
use chrono::{DateTime, Utc};
use gix::Repository;
use gix_odb::memory::Proxy as OdbProxy;
use gix_odb::Handle;
use sqlx::{PgPool, Row};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::broadcast;
use tokio::task;
use tracing::{info, warn, error, debug};
use uuid::Uuid;

/// Git GC configuration
#[derive(Debug, Clone)]
pub struct GitGcConfig {
    pub repos_root: PathBuf,
    pub gc_interval_hours: u64,
    pub auto_gc_threshold: u64,
    pub aggressive: bool,
    pub prune_expire: String,
    pub max_repos_per_run: usize,
}

impl Default for GitGcConfig {
    fn default() -> Self {
        Self {
            repos_root: PathBuf::from("/var/lib/onedev/repos"),
            gc_interval_hours: 24,
            auto_gc_threshold: 1000,
            aggressive: false,
            prune_expire: "2.weeks.ago".to_string(),
            max_repos_per_run: 10,
        }
    }
}

/// Git GC service
pub struct GitGcService {
    db_pool: Arc<PgPool>,
    config: GitGcConfig,
    shutdown_rx: broadcast::Receiver<()>,
}

impl GitGcService {
    /// Create a new Git GC service
    pub fn new(
        db_pool: Arc<PgPool>,
        config: GitGcConfig,
        shutdown_rx: broadcast::Receiver<()>,
    ) -> Self {
        Self {
            db_pool,
            config,
            shutdown_rx,
        }
    }

    /// Run the GC service
    pub async fn run(mut self) {
        info!("Git GC service started");

        let mut interval = tokio::time::interval(
            tokio::time::Duration::from_secs(self.config.gc_interval_hours * 3600)
        );

        // Run once at startup
        if let Err(e) = self.run_gc().await {
            error!("Initial GC failed: {}", e);
        }

        loop {
            tokio::select! {
                _ = interval.tick() => {
                    if let Err(e) = self.run_gc().await {
                        error!("GC run failed: {}", e);
                    }
                }
                _ = self.shutdown_rx.recv() => {
                    info!("Git GC service received shutdown signal");
                    break;
                }
            }
        }

        info!("Git GC service stopped");
    }

    /// Run GC on repositories that need it
    async fn run_gc(&self) -> Result<()> {
        info!("Starting Git GC...");

        // Get repositories that need GC
        let repos = sqlx::query(
            r#"
            SELECT id, path FROM projects
            WHERE deleted_at IS NULL
            AND (
                gc_last_run IS NULL
                OR gc_last_run < NOW() - INTERVAL '1 hour' * $1
            )
            ORDER BY gc_last_run ASC NULLS FIRST
            LIMIT $2
            "#
        )
            .bind(self.config.gc_interval_hours as i64)
            .bind(self.config.max_repos_per_run as i64)
            .fetch_all(self.db_pool.as_ref()).await?;

        info!("Found {} repositories needing GC", repos.len());

        for repo in repos {
            let project_id: Uuid = repo.try_get("id")?;
            let path: String = repo.try_get("path")?;
            let repo_path = self.config.repos_root.join(&path);

            if !repo_path.exists() {
                warn!("Repository path does not exist: {}", repo_path.display());
                continue;
            }

            match self.run_gc_on_repo(&repo_path).await {
                Ok(_) => {
                    info!("GC completed for repository {}", project_id);
                    // Update last GC time
                    sqlx::query("UPDATE projects SET gc_last_run = $1 WHERE id = $2")
                        .bind(Utc::now())
                        .bind(project_id)
                        .execute(self.db_pool.as_ref()).await?;
                }
                Err(e) => {
                    error!("GC failed for repository {}: {}", project_id, e);
                }
            }
        }

        info!("Git GC completed");
        Ok(())
    }

    /// Run GC on a single repository
    async fn run_gc_on_repo(&self, repo_path: &PathBuf) -> Result<()> {
        let repo_path = repo_path.clone();

        // Run GC in blocking task
        task::spawn_blocking(move || -> Result<()> {
            let repo = Repository::discover(&repo_path)?;

            // Configure GC
            let mut gc = repo.gc();

            if self.config.aggressive {
                gc = gc.aggressive(true);
            }

            gc = gc.prune_expire(&self.config.prune_expire);

            // Run GC
            gc.exec()?;

            Ok(())
        }).await??;

        Ok(())
    }

    /// Run aggressive GC on a specific repository
    pub async fn run_aggressive_gc(&self, project_id: Uuid) -> Result<()> {
        let repo = sqlx::query("SELECT path FROM projects WHERE id = $1")
            .bind(project_id)
            .fetch_optional(self.db_pool.as_ref()).await?;

        let Some(repo) = repo else {
            return Err(anyhow::anyhow!("Project not found"));
        };

        let path: String = repo.try_get("path")?;
        let repo_path = self.config.repos_root.join(&path);

        if !repo_path.exists() {
            return Err(anyhow::anyhow!("Repository path does not exist"));
        }

        info!("Running aggressive GC on project {}", project_id);

        let repo_path = repo_path.clone();
        task::spawn_blocking(move || -> Result<()> {
            let repo = Repository::discover(&repo_path)?;
            repo.gc().aggressive(true).prune_expire("now").exec()?;
            Ok(())
        }).await??;

        // Update last GC time
        sqlx::query("UPDATE projects SET gc_last_run = $1 WHERE id = $2")
            .bind(Utc::now())
            .bind(project_id)
            .execute(self.db_pool.as_ref()).await?;

        info!("Aggressive GC completed for project {}", project_id);
        Ok(())
    }

    /// Get GC stats for a repository
    pub async fn get_gc_stats(&self, project_id: Uuid) -> Result<GitGcStats> {
        let repo = sqlx::query("SELECT path, gc_last_run FROM projects WHERE id = $1")
            .bind(project_id)
            .fetch_optional(self.db_pool.as_ref()).await?;

        let Some(repo) = repo else {
            return Err(anyhow::anyhow!("Project not found"));
        };

        let path: String = repo.try_get("path")?;
        let last_gc: Option<DateTime<Utc>> = repo.try_get("gc_last_run").ok();
        let repo_path = self.config.repos_root.join(&path);

        let stats = if repo_path.exists() {
            let repo_path = repo_path.clone();
            task::spawn_blocking(move || -> Result<GitGcStats> {
                let repo = Repository::discover(&repo_path)?;
                let object_db = repo.objects;

                let (loose_count, loose_size) = count_loose_objects(&object_db)?;
                let (pack_count, pack_size) = count_pack_objects(&object_db)?;

                Ok(GitGcStats {
                    loose_objects: loose_count,
                    loose_size_bytes: loose_size,
                    pack_files: pack_count,
                    pack_size_bytes: pack_size,
                    last_gc,
                })
            }).await??
        } else {
            GitGcStats::default()
        };

        Ok(stats)
    }
}

fn count_loose_objects(_object_db: &OdbProxy<Handle>) -> Result<(u64, u64)> {
    // Implementation would iterate loose objects
    Ok((0, 0))
}

fn count_pack_objects(_object_db: &OdbProxy<Handle>) -> Result<(u64, u64)> {
    // Implementation would iterate pack files
    Ok((0, 0))
}

/// Git GC statistics
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct GitGcStats {
    pub loose_objects: u64,
    pub loose_size_bytes: u64,
    pub pack_files: u64,
    pub pack_size_bytes: u64,
    pub last_gc: Option<DateTime<Utc>>,
}