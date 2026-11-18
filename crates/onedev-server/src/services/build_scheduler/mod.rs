//! Build scheduler.
//!
//! Builds are rows in `builds`: whoever decides that a build is due (a push,
//! a tag, a pull request, a schedule, a user) inserts a `pending` row through
//! [`trigger`], this service runs it.
//!
//! * **Claiming** is one `UPDATE … WHERE id = (SELECT … FOR UPDATE SKIP LOCKED)`,
//!   so several server instances can share a database without ever running a
//!   build twice. `NOTIFY onedev_build_queued` wakes the loop early, polling
//!   makes a lost notification harmless.
//! * **Running** happens in [`run`]: checkout, secrets, the
//!   [`onedev_build`] runner on a blocking thread, logs into `build_logs`.
//! * **Liveness**: a running build refreshes `builds.updated_at`; a running
//!   build whose heartbeat is older than `build.stale_after_secs` belongs to a
//!   dead server and is failed by whichever instance notices first.
//! * **Cancelling** is a status change (`cancelled`) the running build polls
//!   for; pending builds are simply never claimed.

pub mod hooks;
pub mod job_match;
pub mod logs;
pub mod run;
pub mod secrets;
pub mod trigger;

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use onedev_build::services::{CancelToken, ExecutorConfig, ExecutorMode};
use onedev_db::DbPool;
use sqlx::Row;
use tokio::sync::Notify;
use tokio::task::JoinSet;
use tracing::{error, info, warn};

use crate::config::{AppConfig, BuildExecutorKind};
use crate::lifecycle::ShutdownSignal;
use crate::services::git_cli::GitCli;
use crate::services::{Backoff, spawn_notify_listener};

/// Postgres channel notified when a build is queued.
pub const QUEUE_CHANNEL: &str = "onedev_build_queued";
/// How often the scheduler looks for builds of dead servers.
const REAP_INTERVAL: Duration = Duration::from_secs(30);

/// Executor settings derived from the server configuration.
pub fn executor_config(config: &AppConfig) -> anyhow::Result<ExecutorConfig> {
    let build = &config.build;
    let mode = match build.executor {
        BuildExecutorKind::Host => ExecutorMode::Host,
        BuildExecutorKind::Docker => ExecutorMode::Docker,
        BuildExecutorKind::Youki => {
            anyhow::bail!("build.executor = \"youki\" is not available in this build of the server")
        }
    };
    Ok(ExecutorConfig {
        mode,
        docker_binary: build.docker_binary.clone(),
        git_binary: config.git.git_binary.clone(),
        default_image: build.default_image.clone(),
        term_grace: Duration::from_secs(build.term_grace_secs.max(1)),
        container_memory: build.container_memory.clone(),
        container_cpus: build.container_cpus.clone(),
        container_pids_limit: build.container_pids_limit,
        max_artifact_bytes: build.max_artifact_mb.saturating_mul(1024 * 1024),
        ..ExecutorConfig::default()
    })
}

/// A build this instance has claimed and is about to run.
#[derive(Debug, Clone)]
pub struct ClaimedBuild {
    pub id: i64,
    pub project_id: i64,
    pub number: i32,
    pub ref_name: String,
    pub commit_sha: String,
    pub commit_message: Option<String>,
    pub author_id: i64,
    /// `build_trigger` as text (`push`, `pull_request`, ...).
    pub trigger: String,
    pub trigger_data: serde_json::Value,
    pub spec: serde_json::Value,
    pub parameters: serde_json::Value,
}

pub struct BuildScheduler {
    pub(crate) pool: DbPool,
    pub(crate) config: Arc<AppConfig>,
    pub(crate) git: GitCli,
    pub(crate) executor: ExecutorConfig,
    wake: Arc<Notify>,
    /// Cancel tokens of the builds running here.
    running: Mutex<HashMap<i64, CancelToken>>,
}

impl BuildScheduler {
    pub fn new(pool: DbPool, config: Arc<AppConfig>) -> anyhow::Result<Arc<Self>> {
        let executor = executor_config(&config)?;
        let git = GitCli::new(config.git.git_binary.clone());
        Ok(Arc::new(Self {
            pool,
            config,
            git,
            executor,
            wake: Arc::new(Notify::new()),
            running: Mutex::new(HashMap::new()),
        }))
    }

    /// Wake the scheduler (a build was queued).
    pub fn wake_handle(&self) -> Arc<Notify> {
        self.wake.clone()
    }

    pub(crate) fn register(&self, build_id: i64, token: CancelToken) {
        if let Ok(mut running) = self.running.lock() {
            running.insert(build_id, token);
        }
    }

    pub(crate) fn unregister(&self, build_id: i64) {
        if let Ok(mut running) = self.running.lock() {
            running.remove(&build_id);
        }
    }

    fn cancel_all(&self) {
        if let Ok(running) = self.running.lock() {
            for token in running.values() {
                token.cancel();
            }
        }
    }

    /// Claim the oldest pending build, `None` when the queue is empty.
    pub async fn claim_next(&self) -> Result<Option<ClaimedBuild>, sqlx::Error> {
        let row = sqlx::query(
            r#"
            UPDATE builds
            SET status = 'running', started_at = NOW(), updated_at = NOW(),
                finished_at = NULL, error_message = NULL
            WHERE id = (
                SELECT id FROM builds
                WHERE status = 'pending'
                ORDER BY created_at, id
                FOR UPDATE SKIP LOCKED
                LIMIT 1
            )
            RETURNING id, project_id, number, ref_name, commit_sha, commit_message, author_id,
                      trigger::text AS trigger, trigger_data, spec, parameters
            "#,
        )
        .fetch_optional(&self.pool)
        .await?;
        let Some(row) = row else { return Ok(None) };
        Ok(Some(ClaimedBuild {
            id: row.try_get("id")?,
            project_id: row.try_get("project_id")?,
            number: row.try_get("number")?,
            ref_name: row.try_get("ref_name")?,
            commit_sha: row.try_get("commit_sha")?,
            commit_message: row.try_get("commit_message")?,
            author_id: row.try_get("author_id")?,
            trigger: row.try_get("trigger")?,
            trigger_data: row
                .try_get::<Option<serde_json::Value>, _>("trigger_data")?
                .unwrap_or(serde_json::Value::Null),
            spec: row.try_get("spec")?,
            parameters: row
                .try_get::<Option<serde_json::Value>, _>("parameters")?
                .unwrap_or(serde_json::Value::Null),
        }))
    }

    /// Fail running builds whose heartbeat stopped (their server died).
    /// Returns the number of builds failed.
    pub async fn fail_stale_builds(&self) -> Result<u64, sqlx::Error> {
        let stale_after = self.config.build.stale_after_secs.max(30) as f64;
        let mut tx = self.pool.begin().await?;
        let ids: Vec<i64> = sqlx::query_scalar(
            r#"
            UPDATE builds
            SET status = 'failed', finished_at = NOW(),
                duration = GREATEST(EXTRACT(EPOCH FROM (NOW() - COALESCE(started_at, NOW())))::int, 0),
                error_message = 'build was interrupted: the server running it stopped responding'
            WHERE status = 'running' AND updated_at < NOW() - make_interval(secs => $1::double precision)
            RETURNING id
            "#,
        )
        .bind(stale_after)
        .fetch_all(&mut *tx)
        .await?;
        if !ids.is_empty() {
            sqlx::query(
                r#"
                UPDATE build_jobs
                SET status = CASE WHEN status = 'running' THEN 'failed'::build_status
                                  ELSE 'skipped'::build_status END,
                    finished_at = COALESCE(finished_at, NOW()),
                    error_message = COALESCE(error_message, 'build was interrupted')
                WHERE build_id = ANY($1) AND status IN ('pending', 'running')
                "#,
            )
            .bind(&ids)
            .execute(&mut *tx)
            .await?;
        }
        tx.commit().await?;
        Ok(ids.len() as u64)
    }

    /// Run until shutdown: claim builds up to the concurrency limit, run them,
    /// and stop (and wait for) the running builds when the server shuts down.
    pub async fn run(self: Arc<Self>, mut shutdown: ShutdownSignal) {
        let listener = spawn_notify_listener(
            self.pool.clone(),
            QUEUE_CHANNEL,
            self.wake.clone(),
            shutdown.child(),
        );
        let poll = Duration::from_secs(self.config.build.poll_interval_secs.max(1));
        let max_concurrent = self.config.build.max_concurrent_builds.max(1);
        let mut tasks: JoinSet<()> = JoinSet::new();
        let mut last_reap = Instant::now()
            .checked_sub(REAP_INTERVAL)
            .unwrap_or_else(Instant::now);
        let mut backoff = Backoff::new(Duration::from_secs(1), Duration::from_secs(30));
        info!(
            max_concurrent,
            executor = self.config.build.executor.as_str(),
            "build scheduler started"
        );

        while !shutdown.is_set() {
            while let Some(finished) = tasks.try_join_next() {
                log_join(finished);
            }

            if last_reap.elapsed() >= REAP_INTERVAL {
                last_reap = Instant::now();
                match self.fail_stale_builds().await {
                    Ok(0) => {}
                    Ok(count) => warn!(count, "failed builds whose server stopped responding"),
                    Err(error) => warn!(%error, "cannot look for stale builds"),
                }
            }

            let mut claim_failed = false;
            while tasks.len() < max_concurrent && !shutdown.is_set() {
                match self.claim_next().await {
                    Ok(Some(build)) => {
                        backoff.reset();
                        metrics::counter!("builds_started_total").increment(1);
                        let scheduler = self.clone();
                        let signal = shutdown.child();
                        tasks.spawn(async move { scheduler.execute(build, signal).await });
                        metrics::gauge!("builds_running").set(tasks.len() as f64);
                    }
                    Ok(None) => break,
                    Err(error) => {
                        warn!(%error, "cannot claim a build");
                        claim_failed = true;
                        break;
                    }
                }
            }

            let wait = if claim_failed { backoff.next_delay() } else { poll };
            tokio::select! {
                _ = self.wake.notified() => {}
                _ = tokio::time::sleep(wait) => {}
                Some(finished) = tasks.join_next(), if !tasks.is_empty() => {
                    log_join(finished);
                    metrics::gauge!("builds_running").set(tasks.len() as f64);
                }
                _ = shutdown.wait() => {}
            }
        }

        // Shutdown: stop the running builds and give them time to record
        // their outcome.
        if !tasks.is_empty() {
            info!(running = tasks.len(), "stopping running builds");
            self.cancel_all();
            let grace = Duration::from_secs(self.config.server.shutdown_grace_secs.max(5));
            let waited = tokio::time::timeout(grace, async {
                while let Some(finished) = tasks.join_next().await {
                    log_join(finished);
                }
            })
            .await;
            if waited.is_err() {
                warn!("running builds did not stop in time, abandoning them");
                tasks.abort_all();
            }
        }
        metrics::gauge!("builds_running").set(0.0);
        listener.abort();
        info!("build scheduler stopped");
    }
}

fn log_join(result: Result<(), tokio::task::JoinError>) {
    if let Err(error) = result {
        if error.is_panic() {
            error!("a build task panicked");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testsupport::{TestDb, seed_project, seed_user};

    async fn insert_build(pool: &DbPool, project: i64, user: i64, number: i32, status: &str) -> i64 {
        sqlx::query_scalar::<_, i64>(
            "INSERT INTO builds (project_id, number, ref_name, commit_sha, author_id, spec, status) \
             VALUES ($1, $2, 'main', 'abc', $3, '{\"jobs\": []}'::jsonb, $4::build_status) RETURNING id",
        )
        .bind(project)
        .bind(number)
        .bind(user)
        .bind(status)
        .fetch_one(pool)
        .await
        .unwrap()
    }

    fn scheduler(db: &TestDb) -> Arc<BuildScheduler> {
        BuildScheduler::new(db.pool.clone(), Arc::new(AppConfig::default())).unwrap()
    }

    #[tokio::test]
    async fn claiming_is_fifo_exclusive_and_marks_the_build_running() {
        let Some(db) = TestDb::new().await else { return };
        let user = seed_user(&db.pool, "sched").await;
        let project = seed_project(&db.pool, user, "claim-project", None).await;
        let first = insert_build(&db.pool, project, user, 1, "pending").await;
        let second = insert_build(&db.pool, project, user, 2, "pending").await;
        let _done = insert_build(&db.pool, project, user, 3, "success").await;
        let scheduler = scheduler(&db);

        let a = scheduler.claim_next().await.unwrap().expect("first claim");
        let b = scheduler.claim_next().await.unwrap().expect("second claim");
        assert_eq!((a.id, b.id), (first, second));
        assert_eq!(a.number, 1);
        assert!(a.trigger_data.is_null());
        assert!(scheduler.claim_next().await.unwrap().is_none(), "queue is empty now");

        let status: String = sqlx::query_scalar("SELECT status::text FROM builds WHERE id = $1")
            .bind(first)
            .fetch_one(&db.pool)
            .await
            .unwrap();
        assert_eq!(status, "running");
    }

    #[tokio::test]
    async fn concurrent_claims_never_hand_out_a_build_twice() {
        let Some(db) = TestDb::new().await else { return };
        let user = seed_user(&db.pool, "sched2").await;
        let project = seed_project(&db.pool, user, "race-project", None).await;
        for number in 1..=12 {
            insert_build(&db.pool, project, user, number, "pending").await;
        }
        let scheduler = scheduler(&db);
        let mut handles = Vec::new();
        for _ in 0..6 {
            let scheduler = scheduler.clone();
            handles.push(tokio::spawn(async move {
                let mut claimed = Vec::new();
                while let Some(build) = scheduler.claim_next().await.unwrap() {
                    claimed.push(build.id);
                }
                claimed
            }));
        }
        let mut all: Vec<i64> = Vec::new();
        for handle in handles {
            all.extend(handle.await.unwrap());
        }
        all.sort_unstable();
        let unique: std::collections::BTreeSet<i64> = all.iter().copied().collect();
        assert_eq!(all.len(), 12, "every build claimed: {all:?}");
        assert_eq!(unique.len(), 12, "no build claimed twice: {all:?}");
    }

    #[tokio::test]
    async fn only_builds_with_a_stale_heartbeat_are_failed() {
        let Some(db) = TestDb::new().await else { return };
        let user = seed_user(&db.pool, "sched3").await;
        let project = seed_project(&db.pool, user, "stale-project", None).await;
        let stale = insert_build(&db.pool, project, user, 1, "running").await;
        let fresh = insert_build(&db.pool, project, user, 2, "running").await;
        let pending = insert_build(&db.pool, project, user, 3, "pending").await;
        let job = sqlx::query_scalar::<_, i64>(
            "INSERT INTO build_jobs (build_id, name, spec, status) VALUES ($1, 'j', '{}'::jsonb, 'running') RETURNING id",
        )
        .bind(stale)
        .fetch_one(&db.pool)
        .await
        .unwrap();
        // the heartbeat trigger rewrites updated_at on every update, so age the row explicitly
        sqlx::query("ALTER TABLE builds DISABLE TRIGGER update_builds_updated_at")
            .execute(&db.pool)
            .await
            .unwrap();
        sqlx::query("UPDATE builds SET updated_at = NOW() - INTERVAL '1 hour', started_at = NOW() - INTERVAL '1 hour' WHERE id = $1")
            .bind(stale)
            .execute(&db.pool)
            .await
            .unwrap();
        sqlx::query("ALTER TABLE builds ENABLE TRIGGER update_builds_updated_at")
            .execute(&db.pool)
            .await
            .unwrap();

        let scheduler = scheduler(&db);
        assert_eq!(scheduler.fail_stale_builds().await.unwrap(), 1);
        assert_eq!(scheduler.fail_stale_builds().await.unwrap(), 0);

        let status = |id: i64| {
            let pool = db.pool.clone();
            async move {
                sqlx::query_scalar::<_, String>("SELECT status::text FROM builds WHERE id = $1")
                    .bind(id)
                    .fetch_one(&pool)
                    .await
                    .unwrap()
            }
        };
        assert_eq!(status(stale).await, "failed");
        assert_eq!(status(fresh).await, "running");
        assert_eq!(status(pending).await, "pending");
        let (job_status, error): (String, Option<String>) =
            sqlx::query_as("SELECT status::text, error_message FROM build_jobs WHERE id = $1")
                .bind(job)
                .fetch_one(&db.pool)
                .await
                .unwrap();
        assert_eq!(job_status, "failed");
        assert!(error.unwrap().contains("interrupted"));
    }

    #[test]
    fn the_executor_follows_the_configuration() {
        let mut config = AppConfig::default();
        assert_eq!(executor_config(&config).unwrap().mode, ExecutorMode::Host);
        config.build.executor = BuildExecutorKind::Docker;
        config.build.container_memory = Some("2g".to_owned());
        config.build.max_artifact_mb = 3;
        let executor = executor_config(&config).unwrap();
        assert_eq!(executor.mode, ExecutorMode::Docker);
        assert_eq!(executor.container_memory.as_deref(), Some("2g"));
        assert_eq!(executor.max_artifact_bytes, 3 * 1024 * 1024);
    }
}
