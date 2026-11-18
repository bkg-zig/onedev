//! Build Scheduler Service

use anyhow::Result;
use chrono::{DateTime, Utc};
use flume;
use sqlx::Row;
use sqlx::PgPool;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Semaphore;
use tracing::{info, warn, error, debug};
use uuid::Uuid;

// Import types from onedev-build
use onedev_build::models::{BuildStatus, BuildSpec, BuildReport, JobResult, LogLine, TriggerEvent};
use onedev_build::services::{BuildRunner, HostExecutor};
use onedev_build::models::{StepResult, ExecuteCondition};

/// Build scheduler configuration
#[derive(Debug, Clone)]
pub struct BuildSchedulerConfig {
    pub max_concurrent_builds: usize,
    pub default_timeout_minutes: i64,
    pub queue_poll_interval_secs: u64,
    pub stuck_build_threshold_minutes: i64,
}

impl Default for BuildSchedulerConfig {
    fn default() -> Self {
        Self {
            max_concurrent_builds: 4,
            default_timeout_minutes: 60,
            queue_poll_interval_secs: 10,
            stuck_build_threshold_minutes: 10,
        }
    }
}

/// Build queue item
#[derive(Debug, Clone)]
pub struct QueuedBuild {
    pub build_id: Uuid,
    pub project_id: Uuid,
    pub pipeline: String,
    pub branch: String,
    pub commit_sha: String,
    pub variables: std::collections::HashMap<String, String>,
    pub triggered_by: Uuid,
    pub queued_at: DateTime<Utc>,
}

/// Build scheduler service
pub struct BuildScheduler {
    db_pool: Arc<PgPool>,
    config: BuildSchedulerConfig,
    runner: Arc<BuildRunner>,
    queue_sender: flume::Sender<QueuedBuild>,
    queue_receiver: flume::Receiver<QueuedBuild>,
    semaphore: Arc<Semaphore>,
    running_builds: Arc<tokio::sync::RwLock<Vec<RunningBuild>>>,
    shutdown_rx: tokio::sync::broadcast::Receiver<()>,
}

#[derive(Debug, Clone)]
struct RunningBuild {
    build_id: Uuid,
    started_at: DateTime<Utc>,
    project_id: Uuid,
}

impl BuildScheduler {
    /// Create a new build scheduler
    pub fn new(
        db_pool: Arc<PgPool>,
        config: BuildSchedulerConfig,
        shutdown_rx: tokio::sync::broadcast::Receiver<()>,
    ) -> Self {
        let runner = Arc::new(BuildRunner::new());
        let (queue_sender, queue_receiver) = flume::unbounded();

        let max_concurrent = config.max_concurrent_builds;

        Self {
            db_pool,
            config,
            runner,
            queue_sender,
            queue_receiver,
            semaphore: Arc::new(Semaphore::new(max_concurrent)),
            running_builds: Arc::new(tokio::sync::RwLock::new(Vec::new())),
            shutdown_rx,
        }
    }

    /// Get the queue sender for external use
    pub fn queue_sender(&self) -> flume::Sender<QueuedBuild> {
        self.queue_sender.clone()
    }

    /// Run the scheduler
    pub async fn run(mut self) {
        info!("Build scheduler started with max_concurrent_builds={}", self.config.max_concurrent_builds);

        let mut interval = tokio::time::interval(
            tokio::time::Duration::from_secs(self.config.queue_poll_interval_secs)
        );

        loop {
            tokio::select! {
                _ = interval.tick() => {
                    if let Err(e) = self.process_queue().await {
                        error!("Error processing build queue: {}", e);
                    }
                    if let Err(e) = self.check_stuck_builds().await {
                        error!("Error checking stuck builds: {}", e);
                    }
                }
                _ = self.shutdown_rx.recv() => {
                    info!("Build scheduler received shutdown signal");
                    self.shutdown().await;
                    break;
                }
            }
        }

        info!("Build scheduler stopped");
    }

    /// Process the build queue
    async fn process_queue(&self) -> Result<()> {
        // Try to acquire permit for new build
        let permit = match self.semaphore.clone().try_acquire_owned() {
            Ok(p) => p,
            Err(_) => {
                debug!("No available build slots, skipping queue processing");
                return Ok(());
            }
        };

        // Get next build from queue
        let queued_build = match self.queue_receiver.try_recv() {
            Ok(b) => b,
            Err(flume::TryRecvError::Empty) => {
                // No builds in queue, release permit
                drop(permit);
                return Ok(());
            }
            Err(flume::TryRecvError::Disconnected) => {
                error!("Build queue disconnected");
                drop(permit);
                return Ok(());
            }
        };

        info!("Starting build {} for project {}", queued_build.build_id, queued_build.project_id);

        // Update build status to running
        self.update_build_status(queued_build.build_id, BuildStatus::Running).await?;

        // Track running build
        let running = RunningBuild {
            build_id: queued_build.build_id,
            started_at: Utc::now(),
            project_id: queued_build.project_id,
        };
        self.running_builds.write().await.push(running.clone());

        // Spawn build execution
        let runner = self.runner.clone();
        let db_pool = self.db_pool.clone();
        let running_builds = self.running_builds.clone();
        let semaphore = self.semaphore.clone();
        let build_id = queued_build.build_id;
        let project_id = queued_build.project_id;

        tokio::spawn(async move {
            let _permit = permit; // Release when done
            let db_pool_for_build = db_pool.clone();
            let result = Self::execute_build(
                runner,
                db_pool_for_build,
                &queued_build,
            ).await;

            // Update final status
            let final_status = match result {
                Ok(_) => BuildStatus::Success,
                Err(e) => {
                    error!("Build {} failed: {}", build_id, e);
                    BuildStatus::Failed
                }
            };

            if let Err(e) = sqlx::query("UPDATE builds SET status = $1, finished_at = $2 WHERE id = $3")
                .bind(final_status.to_string())
                .bind(Utc::now())
                .bind(build_id)
                .execute(&*db_pool).await {
                error!("Failed to update build status: {}", e);
            }

            // Remove from running builds
            let mut running = running_builds.write().await;
            running.retain(|b| b.build_id != build_id);
        });

        Ok(())
    }

    /// Execute a build
    async fn execute_build(
        runner: Arc<BuildRunner>,
        db_pool: Arc<PgPool>,
        queued_build: &QueuedBuild,
    ) -> Result<BuildReport> {
        // Load build spec from database
        let spec = Self::load_build_spec(&db_pool, queued_build.project_id, &queued_build.pipeline).await?;

        // Create work directory
        let work_dir = PathBuf::from(format!("/var/lib/onedev/builds/{}", queued_build.build_id));
        tokio::fs::create_dir_all(&work_dir).await?;

        // Create artifact directory
        let artifact_dir = PathBuf::from(format!("/var/lib/onedev/artifacts/{}", queued_build.build_id));
        tokio::fs::create_dir_all(&artifact_dir).await?;

        // Create log channel
        let (log_sender, log_receiver) = flume::unbounded();

        // Start log writer task
        let db_pool_log = db_pool.clone();
        let build_id_log = queued_build.build_id;
        tokio::spawn(async move {
            while let Ok(log_line) = log_receiver.recv_async().await {
                if let Err(e) = Self::append_build_log(&db_pool_log, build_id_log, &log_line).await {
                    error!("Failed to write build log: {}", e);
                }
            }
        });

        // Prepare trigger event
        let trigger_event = onedev_build::models::TriggerEvent::Push {
            branch: queued_build.branch.clone(),
        };

        // Prepare variables
        let mut variables = queued_build.variables.clone();
        variables.insert("ONEDEV_BUILD_NUMBER".to_string(), queued_build.build_id.to_string());
        variables.insert("ONEDEV_PROJECT_ID".to_string(), queued_build.project_id.to_string());
        variables.insert("ONEDEV_COMMIT_SHA".to_string(), queued_build.commit_sha.clone());
        variables.insert("ONEDEV_BRANCH".to_string(), queued_build.branch.clone());

        // Run build in blocking task
        let runner_clone = runner.clone();
        let work_dir_clone = work_dir.clone();
        let artifact_dir_clone = artifact_dir.clone();
        let spec_clone = spec.clone();
        let trigger_event_clone = trigger_event.clone();
        let variables_clone = variables.clone();
        let log_sender_clone = log_sender.clone();

        let report = tokio::task::spawn_blocking(move || {
            runner_clone.run(
                &spec_clone,
                &work_dir_clone,
                &artifact_dir_clone,
                &trigger_event_clone,
                Some(log_sender_clone),
            )
        }).await??;

        // Save artifacts metadata
        Self::save_artifacts(&db_pool, queued_build.build_id, &report, &artifact_dir).await?;

        // Close log channel
        drop(log_sender);

        Ok(report)
    }

    /// Load build spec from database
    async fn load_build_spec(
        db_pool: &PgPool,
        project_id: Uuid,
        pipeline: &str,
    ) -> Result<BuildSpec> {
        let spec_row = sqlx::query("SELECT spec FROM build_specs WHERE project_id = $1 AND name = $2")
            .bind(project_id)
            .bind(pipeline)
            .fetch_optional(db_pool).await?;

        let spec = spec_row
            .map(|row| {
                let spec_str: String = row.try_get("spec").unwrap_or_else(|_| String::new());
                serde_yaml::from_str::<BuildSpec>(&spec_str)
            })
            .transpose()?
            .unwrap_or_else(|| BuildSpec::default());

        Ok(spec)
    }

    /// Append build log
    async fn append_build_log(
        db_pool: &PgPool,
        build_id: Uuid,
        log_line: &onedev_build::models::LogLine,
    ) -> Result<()> {
        let log_entry = format!("[{}] [{}] [{}]: {}\n", log_line.job, log_line.step, if log_line.stderr { "stderr" } else { "stdout" }, log_line.line);
        sqlx::query("UPDATE builds SET log = COALESCE(log, '') || $1 WHERE id = $2")
            .bind(log_entry)
            .bind(build_id)
            .execute(db_pool).await?;
        Ok(())
    }

    /// Save artifacts metadata
    async fn save_artifacts(
        db_pool: &PgPool,
        build_id: Uuid,
        report: &BuildReport,
        artifact_dir: &PathBuf,
    ) -> Result<()> {
        for job_result in &report.jobs {
            for artifact in &job_result.artifacts {
                let path = artifact_dir.join(&job_result.name).join(artifact);
                if path.exists() {
                    let metadata = tokio::fs::metadata(&path).await?;
                    sqlx::query(
                        "INSERT INTO build_artifacts (id, build_id, job_name, name, path, size, created_at) VALUES ($1, $2, $3, $4, $5, $6, $7)"
                    )
                        .bind(Uuid::new_v4())
                        .bind(build_id)
                        .bind(&job_result.name)
                        .bind(artifact)
                        .bind(path.to_string_lossy().to_string())
                        .bind(metadata.len() as i64)
                        .bind(Utc::now())
                        .execute(db_pool).await?;
                }
            }
        }
        Ok(())
    }

    /// Check for stuck builds
    async fn check_stuck_builds(&self) -> Result<()> {
        let threshold = Utc::now() - chrono::Duration::minutes(self.config.stuck_build_threshold_minutes);

        let stuck_builds = sqlx::query(
            r#"
            SELECT id, project_id, started_at
            FROM builds
            WHERE status = 'Running'
            AND started_at < $1
            "#
        )
            .bind(threshold)
            .fetch_all(self.db_pool.as_ref()).await?;

        for build in stuck_builds {
            let build_id: Uuid = build.get("id");
            let project_id: Uuid = build.get("project_id");
            warn!("Build {} appears stuck, marking as failed", build_id);
            sqlx::query("UPDATE builds SET status = $1, finished_at = $2, error = $3 WHERE id = $4")
                .bind(BuildStatus::Failed.to_string())
                .bind(Utc::now())
                .bind("Build timed out / stuck")
                .bind(build_id)
                .execute(self.db_pool.as_ref()).await?;

            // Remove from running builds
            let mut running = self.running_builds.write().await;
            running.retain(|b| b.build_id != build_id);
        }

        Ok(())
    }

    /// Update build status
    async fn update_build_status(&self, build_id: Uuid, status: BuildStatus) -> Result<()> {
        sqlx::query("UPDATE builds SET status = $1, started_at = CASE WHEN started_at IS NULL THEN $2 ELSE started_at END WHERE id = $3")
            .bind(status)
            .bind(Utc::now())
            .bind(build_id)
            .execute(&*self.db_pool).await?;
        Ok(())
    }

    /// Graceful shutdown
    async fn shutdown(&self) {
        info!("Waiting for {} running builds to complete...", self.running_builds.read().await.len());

        // Wait for running builds with timeout
        let timeout = tokio::time::Duration::from_secs(300); // 5 minutes
        let start = tokio::time::Instant::now();

        while !self.running_builds.read().await.is_empty() {
            if start.elapsed() > timeout {
                warn!("Shutdown timeout reached, forcing termination of remaining builds");
                break;
            }
            tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
        }

        info!("All builds completed or terminated");
    }

    /// Get currently running builds count
    pub async fn running_count(&self) -> usize {
        self.running_builds.read().await.len()
    }

    /// Get queue depth
    pub async fn queue_depth(&self) -> usize {
        // Note: flume doesn't have a len() method for unbounded channels
        // This is an approximation
        0
    }
}

/// Build queue for external use
pub struct BuildQueue {
    sender: flume::Sender<QueuedBuild>,
    receiver: flume::Receiver<QueuedBuild>,
}

impl BuildQueue {
    pub fn new(_db_pool: Arc<PgPool>) -> Self {
        // In a real implementation, this would be shared with the scheduler
        let (sender, receiver) = flume::unbounded();
        Self { sender, receiver }
    }

    /// Add a build to the queue
    pub async fn push(&self, build: QueuedBuild) -> Result<()> {
        self.sender.send_async(build).await?;
        Ok(())
    }

    /// Pop a build from the queue (used by scheduler)
    pub fn pop(&self) -> Result<Option<QueuedBuild>> {
        match self.receiver.try_recv() {
            Ok(build) => Ok(Some(build)),
            Err(flume::TryRecvError::Empty) => Ok(None),
            Err(flume::TryRecvError::Disconnected) => Ok(None),
        }
    }

    /// Get queue length (approximate)
    pub async fn len(&self) -> usize {
        0 // flume unbounded doesn't support len()
    }
}