//! Running one claimed build: preparation, execution, bookkeeping.
//!
//! ```text
//! claim ─► load project/spec ─► secrets ─► job rows ─► runner (blocking thread)
//!                                              │            │ logs ─► build_logs
//!                                              │            │ hooks ─► build_jobs / build_steps
//!                                              ▼            ▼
//!                                       heartbeat + cancel watch    finish: status, artifacts, webhook
//! ```

use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use onedev_build::models::{BuildReport, BuildSpec, BuildStatus, TriggerEvent};
use onedev_build::services::{
    BuildRunner, CancelToken, CheckoutSource, LogSink, RunContext, SecretMasker, Variables, execution_order,
    safe_component,
};
use onedev_db::DbPool;
use rand::RngCore;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use sqlx::Row;
use tokio::runtime::Handle;
use tracing::{info, warn};

use super::hooks::SchedulerHooks;
use super::logs::{self, LogStats};
use super::secrets::{self, AuthorizationContext, BuildOrigin, ResolvedSettings};
use super::{BuildScheduler, ClaimedBuild};
use crate::lifecycle::ShutdownSignal;
use crate::services::project_paths::{project_full_path, repository_dir};
use crate::services::webhook_dispatcher::{WebhookEvent, enqueue_event};

/// How often a running build looks at its own row (cancellation).
const CANCEL_POLL: Duration = Duration::from_secs(2);
/// Capacity of the channel between the runner thread and the log writer.
const LOG_CHANNEL: usize = 4096;

/// Project data a build needs.
#[derive(Debug, Clone)]
pub(crate) struct ProjectInfo {
    pub name: String,
    pub full_path: String,
    pub timeout_minutes: i32,
}

/// What a finished run leaves behind.
struct Completed {
    report: BuildReport,
    artifact_root: PathBuf,
    log: LogStats,
}

// ---------------------------------------------------------------------------
// Pure helpers
// ---------------------------------------------------------------------------

/// Names of the jobs a trigger selected (`trigger_data.jobs`); `None` = all.
pub(crate) fn selected_jobs(trigger_data: &Value) -> Option<Vec<String>> {
    trigger_data.get("jobs")?.as_array().map(|jobs| {
        jobs.iter()
            .filter_map(|job| job.as_str().map(str::to_owned))
            .collect()
    })
}

/// Keep the selected jobs and everything they (transitively) depend on.
pub(crate) fn restrict_jobs(spec: &mut BuildSpec, selected: &[String]) {
    let mut keep: HashSet<String> = selected.iter().cloned().collect();
    loop {
        let before = keep.len();
        for job in &spec.jobs {
            if keep.contains(&job.name) {
                keep.extend(job.needs());
            }
        }
        if keep.len() == before {
            break;
        }
    }
    spec.jobs.retain(|job| keep.contains(&job.name));
}

/// `(branch, tag)` of a full ref (empty when the ref is neither).
pub(crate) fn ref_parts(ref_name: &str) -> (String, String) {
    if let Some(branch) = ref_name.strip_prefix("refs/heads/") {
        (branch.to_owned(), String::new())
    } else if let Some(tag) = ref_name.strip_prefix("refs/tags/") {
        (String::new(), tag.to_owned())
    } else {
        (String::new(), String::new())
    }
}

/// Parameter values of a build (`builds.parameters`): strings, numbers and
/// booleans as text, lists comma separated.
pub(crate) fn build_params(parameters: &Value) -> BTreeMap<String, String> {
    let mut params = BTreeMap::new();
    let Some(object) = parameters.as_object() else {
        return params;
    };
    for (name, value) in object {
        let text = match value {
            Value::String(text) => text.clone(),
            Value::Array(items) => items
                .iter()
                .map(|item| match item {
                    Value::String(text) => text.clone(),
                    other => other.to_string(),
                })
                .collect::<Vec<_>>()
                .join(","),
            Value::Null => continue,
            other => other.to_string(),
        };
        params.insert(name.clone(), text);
    }
    params
}

/// `(server_url, server_host)` the build refers to as `@server_url@` and
/// `@server_host@`.
pub(crate) fn server_urls(config: &crate::config::AppConfig) -> (String, String) {
    let url = config
        .server
        .public_url
        .as_deref()
        .map(|url| url.trim_end_matches('/').to_owned())
        .unwrap_or_else(|| format!("http://{}:{}", config.server.bind_address, config.server.port));
    let host = url
        .split_once("://")
        .map_or(url.as_str(), |(_, rest)| rest)
        .split('/')
        .next()
        .unwrap_or_default()
        .to_owned();
    (url, host)
}

/// The variables of a build (everything but the per job ones, which the
/// runner adds).
pub(crate) fn build_variables(
    build: &ClaimedBuild,
    project: &ProjectInfo,
    spec: &BuildSpec,
    resolved: &ResolvedSettings,
    server: &(String, String),
    job_token: &str,
) -> Variables {
    let (branch, tag) = ref_parts(&build.ref_name);
    let number = |pointer: &str| {
        build
            .trigger_data
            .pointer(pointer)
            .and_then(Value::as_i64)
            .map(|n| n.to_string())
            .unwrap_or_default()
    };
    let mut variables = Variables::new()
        .with_job("project_name", project.name.clone())
        .with_job("project_path", project.full_path.clone())
        .with_job("job_token", job_token)
        .with_job("ref", build.ref_name.clone())
        .with_job("branch", branch)
        .with_job("tag", tag)
        .with_job("commit_hash", build.commit_sha.clone())
        .with_job("build_number", build.number.to_string())
        .with_job("pull_request_number", number("/pull_request/number"))
        .with_job("issue_number", number("/issue_number"))
        .with_job("server_url", server.0.clone())
        .with_job("server_host", server.1.clone())
        .with_job("server", server.1.clone());

    variables.params = build_params(&build.parameters);
    // project settings first, properties of the build spec win
    variables.properties = resolved.properties.clone();
    for property in &spec.properties {
        if let (Some(name), Some(value)) = (
            property.get("name").and_then(Value::as_str),
            property.get("value").and_then(Value::as_str),
        ) {
            variables.properties.insert(name.to_owned(), value.to_owned());
        }
    }
    variables.secrets = resolved.secrets.clone();
    variables.denied_secrets = resolved.denied.clone();
    variables.extra_masked = resolved.denied_values.clone();
    variables
}

/// Payload of `build.*` webhook events.
pub(crate) fn webhook_payload(
    event: &str,
    build: &ClaimedBuild,
    project_path: &str,
    status: &str,
    duration: Option<i64>,
    error: Option<&str>,
    public_url: Option<&str>,
) -> Value {
    json!({
        "event": event,
        "project": { "id": build.project_id, "path": project_path },
        "build": {
            "id": build.id,
            "number": build.number,
            "status": status,
            "ref": build.ref_name,
            "commit": build.commit_sha,
            "trigger": build.trigger,
            "duration": duration,
            "error": error,
            "url": public_url.map(|base| format!("{}/{}/~builds/{}", base.trim_end_matches('/'), project_path, build.number)),
        },
    })
}

// ---------------------------------------------------------------------------
// Database helpers
// ---------------------------------------------------------------------------

async fn load_project(pool: &DbPool, id: i64) -> Result<Option<ProjectInfo>, sqlx::Error> {
    let Some(full_path) = project_full_path(pool, id).await? else {
        return Ok(None);
    };
    let row = sqlx::query("SELECT name, build_timeout FROM projects WHERE id = $1 AND deleted_at IS NULL")
        .bind(id)
        .fetch_optional(pool)
        .await?;
    let Some(row) = row else { return Ok(None) };
    Ok(Some(ProjectInfo {
        name: row.try_get("name")?,
        full_path,
        timeout_minutes: row.try_get("build_timeout")?,
    }))
}

/// One `build_jobs` row per job of the build, in execution order.
async fn create_job_rows(
    pool: &DbPool,
    build_id: i64,
    spec: &BuildSpec,
) -> Result<HashMap<String, i64>, String> {
    let order = execution_order(spec).map_err(|e| e.to_string())?;
    let jobs: HashMap<&str, &onedev_build::models::JobSpec> =
        spec.jobs.iter().map(|job| (job.name.as_str(), job)).collect();
    let db = |e: sqlx::Error| format!("cannot create the job records: {e}");

    let mut tx = pool.begin().await.map_err(db)?;
    sqlx::query("DELETE FROM build_jobs WHERE build_id = $1")
        .bind(build_id)
        .execute(&mut *tx)
        .await
        .map_err(db)?;
    let mut rows: HashMap<String, i64> = HashMap::new();
    for (index, name) in order.iter().enumerate() {
        let job = jobs[name.as_str()];
        let spec_json = serde_json::to_value(job).map_err(|e| e.to_string())?;
        let id: i64 = sqlx::query_scalar(
            "INSERT INTO build_jobs (build_id, name, status, spec, order_num) \
             VALUES ($1, $2, 'pending', $3, $4) RETURNING id",
        )
        .bind(build_id)
        .bind(name)
        .bind(spec_json)
        .bind(index as i32)
        .fetch_one(&mut *tx)
        .await
        .map_err(db)?;
        rows.insert(name.clone(), id);
    }
    for job in &spec.jobs {
        let dependencies: Vec<i64> = job.needs().iter().filter_map(|need| rows.get(need).copied()).collect();
        if !dependencies.is_empty() {
            sqlx::query("UPDATE build_jobs SET dependencies = $2 WHERE id = $1")
                .bind(rows[&job.name])
                .bind(&dependencies)
                .execute(&mut *tx)
                .await
                .map_err(db)?;
        }
    }
    tx.commit().await.map_err(db)?;
    Ok(rows)
}

/// Append one line to the build log (used for failures before the runner ran).
async fn append_log(pool: &DbPool, build_id: i64, text: &str) {
    let result = sqlx::query(
        "INSERT INTO build_logs (build_id, seq, job_name, step_name, stream, line) \
         SELECT $1, COALESCE(MAX(seq), -1) + 1, '', '', 'stderr', $2 FROM build_logs WHERE build_id = $1",
    )
    .bind(build_id)
    .bind(text.replace('\0', "\u{fffd}"))
    .execute(pool)
    .await;
    if let Err(error) = result {
        warn!(%error, build_id, "cannot append to the build log");
    }
}

async fn current_status(pool: &DbPool, build_id: i64) -> Option<BuildStatus> {
    sqlx::query_scalar::<_, BuildStatus>("SELECT status FROM builds WHERE id = $1")
        .bind(build_id)
        .fetch_optional(pool)
        .await
        .ok()
        .flatten()
}

/// Heartbeat and cancellation watch of a running build.
async fn watch_build(
    pool: DbPool,
    build_id: i64,
    token: CancelToken,
    heartbeat: Duration,
    mut shutdown: ShutdownSignal,
) {
    let mut ticker = tokio::time::interval(CANCEL_POLL);
    ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    let mut last_beat = Instant::now();
    loop {
        tokio::select! {
            _ = ticker.tick() => {}
            _ = shutdown.wait() => {
                token.cancel();
                return;
            }
        }
        let beat = last_beat.elapsed() >= heartbeat;
        let status = if beat {
            last_beat = Instant::now();
            sqlx::query_scalar::<_, BuildStatus>("UPDATE builds SET updated_at = NOW() WHERE id = $1 RETURNING status")
                .bind(build_id)
                .fetch_optional(&pool)
                .await
        } else {
            sqlx::query_scalar::<_, BuildStatus>("SELECT status FROM builds WHERE id = $1")
                .bind(build_id)
                .fetch_optional(&pool)
                .await
        };
        match status {
            Ok(Some(BuildStatus::Running)) => {}
            // cancelled by a user, failed by another server, or deleted
            Ok(_) => {
                token.cancel();
                return;
            }
            Err(error) => warn!(%error, build_id, "cannot refresh the build heartbeat"),
        }
    }
}

/// Size and SHA-256 of a file.
fn hash_file(path: &Path) -> std::io::Result<(u64, String)> {
    use std::io::Read;
    let mut file = std::fs::File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0u8; 64 * 1024];
    let mut size = 0u64;
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
        size += read as u64;
    }
    Ok((size, hex::encode(hasher.finalize())))
}

// ---------------------------------------------------------------------------
// The build
// ---------------------------------------------------------------------------

impl BuildScheduler {
    fn work_dir(&self, build_id: i64) -> PathBuf {
        self.config.build_work_dir().join(build_id.to_string())
    }

    fn artifact_dir(&self, build: &ClaimedBuild) -> PathBuf {
        self.config
            .build_artifact_dir()
            .join(build.project_id.to_string())
            .join(build.number.to_string())
    }

    /// Run a claimed build to its end and record the outcome. Never fails:
    /// whatever goes wrong ends up as a failed build with an explanation.
    pub(super) async fn execute(self: Arc<Self>, build: ClaimedBuild, shutdown: ShutdownSignal) {
        let token = CancelToken::new();
        self.register(build.id, token.clone());
        let started = Instant::now();
        info!(build_id = build.id, number = build.number, project_id = build.project_id, "build started");

        let project_path = project_full_path(&self.pool, build.project_id)
            .await
            .ok()
            .flatten()
            .unwrap_or_default();
        self.notify("build.started", &build, &project_path, "running", None, None).await;

        let outcome = self.run_build(&build, &token, &shutdown).await;
        self.unregister(build.id);
        self.finish(&build, &project_path, outcome, &shutdown, started).await;
    }

    async fn notify(
        &self,
        event: &str,
        build: &ClaimedBuild,
        project_path: &str,
        status: &str,
        duration: Option<i64>,
        error: Option<&str>,
    ) {
        if !self.config.features.webhooks {
            return;
        }
        let payload = webhook_payload(
            event,
            build,
            project_path,
            status,
            duration,
            error,
            self.config.server.public_url.as_deref(),
        );
        let event = WebhookEvent {
            name: event.to_owned(),
            project_id: Some(build.project_id),
            ref_name: build.ref_name.starts_with("refs/").then(|| build.ref_name.clone()),
            payload,
        };
        if let Err(error) = enqueue_event(&self.pool, &event).await {
            warn!(%error, build_id = build.id, "cannot queue the build webhook event");
        }
    }

    async fn origin(&self, build: &ClaimedBuild, repo: &Path) -> BuildOrigin {
        if let Some(pull_request) = build.trigger_data.get("pull_request") {
            let same_project = pull_request
                .get("source_project_id")
                .and_then(Value::as_i64)
                .is_none_or(|id| id == build.project_id);
            let branch = |key: &str| pull_request.get(key).and_then(Value::as_str).map(str::to_owned);
            if same_project && let (Some(source_branch), Some(target_branch)) = (branch("source_branch"), branch("target_branch")) {
                return BuildOrigin::PullRequest {
                    source_branch,
                    target_branch,
                };
            }
        }
        let branches = match self.git.branches_containing(repo, &build.commit_sha).await {
            Ok(branches) => branches,
            Err(error) => {
                // fail closed: without knowing the branches no secret is authorized
                warn!(%error, build_id = build.id, "cannot determine the branches of the commit");
                Vec::new()
            }
        };
        BuildOrigin::Commit { branches }
    }

    async fn run_build(
        &self,
        build: &ClaimedBuild,
        token: &CancelToken,
        shutdown: &ShutdownSignal,
    ) -> Result<Completed, String> {
        let project = load_project(&self.pool, build.project_id)
            .await
            .map_err(|e| format!("cannot load the project: {e}"))?
            .ok_or_else(|| "the project does not exist (anymore)".to_owned())?;

        let mut spec: BuildSpec =
            serde_json::from_value(build.spec.clone()).map_err(|e| format!("invalid build spec: {e}"))?;
        if let Some(selected) = selected_jobs(&build.trigger_data) {
            restrict_jobs(&mut spec, &selected);
        }
        if spec.jobs.is_empty() {
            return Err("the build has no jobs to run".to_owned());
        }

        let repo_path = repository_dir(&self.config.git.repos_root, &project.full_path)
            .filter(|path| path.is_dir())
            .ok_or_else(|| format!("the repository of {} does not exist", project.full_path))?;

        // secrets and properties
        let hierarchy = secrets::load_hierarchy(&self.pool, build.project_id)
            .await
            .map_err(|e| format!("cannot load the project settings: {e}"))?;
        let context = AuthorizationContext {
            project_path: project.full_path.clone(),
            origin: self.origin(build, &repo_path).await,
        };
        let resolved = secrets::resolve(&hierarchy, &context);

        let mut token_bytes = [0u8; 16];
        rand::rng().fill_bytes(&mut token_bytes);
        let job_token = hex::encode(token_bytes);
        let variables = build_variables(
            build,
            &project,
            &spec,
            &resolved,
            &server_urls(&self.config),
            &job_token,
        );

        // directories
        let work_dir = self.work_dir(build.id);
        let artifact_root = self.artifact_dir(build);
        let _ = tokio::fs::remove_dir_all(&work_dir).await;
        let _ = tokio::fs::remove_dir_all(&artifact_root).await;
        tokio::fs::create_dir_all(&work_dir)
            .await
            .map_err(|e| format!("cannot create the work directory: {e}"))?;
        tokio::fs::create_dir_all(&artifact_root)
            .await
            .map_err(|e| format!("cannot create the artifact directory: {e}"))?;

        let lfs_objects_dir = match &self.config.git.lfs_storage_path {
            Some(root) => root.join(&project.full_path),
            None => repo_path.join("lfs"),
        }
        .join("objects");

        // bookkeeping rows, logs, hooks
        let job_rows = create_job_rows(&self.pool, build.id, &spec).await?;
        let (log_tx, log_rx) = flume::bounded(LOG_CHANNEL);
        let writer = logs::spawn_writer(self.pool.clone(), build.id, log_rx, self.config.build.max_log_lines);
        let hooks = Arc::new(SchedulerHooks::new(
            Handle::current(),
            self.pool.clone(),
            build.id,
            job_rows,
        ));

        let timeout_minutes = if project.timeout_minutes > 0 {
            i64::from(project.timeout_minutes)
        } else {
            self.config.build.default_timeout_minutes.max(1)
        };
        let timeout = Duration::from_secs(timeout_minutes.unsigned_abs() * 60);

        let mut context = RunContext::new(&work_dir, &artifact_root);
        context.log = LogSink::new(Some(log_tx), SecretMasker::new(variables.secret_values()));
        context.cancel = token.clone();
        context.deadline = Some(Instant::now() + timeout);
        context.checkout = Some(CheckoutSource {
            repo_path,
            commit: build.commit_sha.clone(),
            lfs_objects_dir: Some(lfs_objects_dir),
        });
        context.variables = variables;
        context.config = self.executor.clone();
        context.hooks = Some(hooks);

        let heartbeat = Duration::from_secs((self.config.build.stale_after_secs / 6).clamp(2, 30));
        let watcher = tokio::spawn(watch_build(
            self.pool.clone(),
            build.id,
            token.clone(),
            heartbeat,
            shutdown.child(),
        ));

        let run = tokio::task::spawn_blocking(move || {
            // `context` (and with it the log sender) is dropped when the run ends
            BuildRunner::new().run_with(&spec, &context, &TriggerEvent::Manual)
        })
        .await;
        watcher.abort();
        let log = writer.await.unwrap_or_default();

        match run {
            Ok(Ok(report)) => Ok(Completed {
                report,
                artifact_root,
                log,
            }),
            Ok(Err(error)) => Err(error.to_string()),
            Err(join) => Err(format!("the build runner crashed: {join}")),
        }
    }

    /// Record the outcome of a run.
    async fn finish(
        &self,
        build: &ClaimedBuild,
        project_path: &str,
        outcome: Result<Completed, String>,
        shutdown: &ShutdownSignal,
        started: Instant,
    ) {
        let db_status = current_status(&self.pool, build.id).await;
        let user_cancelled = db_status == Some(BuildStatus::Cancelled);

        let (status, error, completed) = match outcome {
            Ok(done) => {
                let (status, error) = match done.report.status {
                    BuildStatus::Cancelled if user_cancelled => {
                        (BuildStatus::Cancelled, Some("build was cancelled".to_owned()))
                    }
                    BuildStatus::Cancelled if shutdown.is_set() => (
                        BuildStatus::Failed,
                        Some("build was interrupted: the server is shutting down".to_owned()),
                    ),
                    BuildStatus::Cancelled => (BuildStatus::Cancelled, Some("build was cancelled".to_owned())),
                    other => (other, done.report.error.clone()),
                };
                (status, error, Some(done))
            }
            Err(message) => {
                warn!(build_id = build.id, %message, "build could not run");
                append_log(&self.pool, build.id, &message).await;
                (BuildStatus::Failed, Some(message), None)
            }
        };

        // artifacts
        let mut artifacts: Vec<Value> = Vec::new();
        if let Some(done) = &completed {
            artifacts = self.register_artifacts(build, &done.report, &done.artifact_root).await;
        }

        // job rows: whatever the hooks did not see (skipped, cancelled) and
        // anything still open when the build failed before running
        if let Some(done) = &completed {
            for job in &done.report.jobs {
                let reason = super::hooks::failure_reason(job);
                let result = sqlx::query(
                    "UPDATE build_jobs SET status = $3, finished_at = COALESCE(finished_at, NOW()), \
                     duration = COALESCE(duration, $4), error_message = COALESCE(error_message, $5) \
                     WHERE build_id = $1 AND name = $2 AND status IN ('pending', 'running')",
                )
                .bind(build.id)
                .bind(&job.name)
                .bind(job.status)
                .bind((job.duration_ms as f64 / 1000.0).round() as i32)
                .bind(reason)
                .execute(&self.pool)
                .await;
                if let Err(error) = result {
                    warn!(%error, build_id = build.id, "cannot finalize a job record");
                }
            }
        }
        let leftover = if status == BuildStatus::Cancelled {
            BuildStatus::Cancelled
        } else {
            BuildStatus::Skipped
        };
        if let Err(error) = sqlx::query(
            "UPDATE build_jobs SET status = CASE WHEN status = 'running' THEN $2 ELSE $3 END, \
             finished_at = COALESCE(finished_at, NOW()) \
             WHERE build_id = $1 AND status IN ('pending', 'running')",
        )
        .bind(build.id)
        .bind(if status == BuildStatus::Cancelled { BuildStatus::Cancelled } else { BuildStatus::Failed })
        .bind(leftover)
        .execute(&self.pool)
        .await
        {
            warn!(%error, build_id = build.id, "cannot close the open job records");
        }

        // the build itself (a user's `cancelled` is never overwritten)
        let duration_row = sqlx::query(
            "UPDATE builds SET \
                 status = CASE WHEN status = 'cancelled' THEN status ELSE $2 END, \
                 finished_at = NOW(), \
                 duration = GREATEST(EXTRACT(EPOCH FROM (NOW() - COALESCE(started_at, NOW())))::int, 0), \
                 error_message = $3, artifacts = $4 \
             WHERE id = $1 AND status IN ('running', 'cancelled') \
             RETURNING status::text AS status, duration",
        )
        .bind(build.id)
        .bind(status)
        .bind(error.as_deref())
        .bind(Value::Array(artifacts))
        .fetch_optional(&self.pool)
        .await;
        let (final_status, duration) = match duration_row {
            Ok(Some(row)) => (
                row.try_get::<String, _>("status").unwrap_or_else(|_| status_text(status).to_owned()),
                row.try_get::<Option<i32>, _>("duration").ok().flatten().map(i64::from),
            ),
            Ok(None) => {
                // another server already failed this build (stale heartbeat)
                warn!(build_id = build.id, "the build was finalized by someone else");
                (status_text(status).to_owned(), None)
            }
            Err(error) => {
                warn!(%error, build_id = build.id, "cannot store the build result");
                (status_text(status).to_owned(), None)
            }
        };

        let elapsed = started.elapsed();
        metrics::counter!("builds_total", "status" => final_status.clone()).increment(1);
        metrics::histogram!("build_duration_seconds", "status" => final_status.clone()).record(elapsed.as_secs_f64());
        if let Some(done) = &completed
            && (done.log.discarded > 0 || done.log.lost > 0)
        {
            warn!(
                build_id = build.id,
                discarded = done.log.discarded,
                lost = done.log.lost,
                "build log is incomplete"
            );
        }
        info!(build_id = build.id, status = %final_status, seconds = elapsed.as_secs(), "build finished");

        self.notify(
            "build.finished",
            build,
            project_path,
            &final_status,
            duration,
            error.as_deref(),
        )
        .await;

        let _ = tokio::fs::remove_dir_all(self.work_dir(build.id)).await;
    }

    /// Store the files jobs published and describe them for `builds.artifacts`.
    async fn register_artifacts(
        &self,
        build: &ClaimedBuild,
        report: &BuildReport,
        artifact_root: &Path,
    ) -> Vec<Value> {
        let mut registered = Vec::new();
        for job in &report.jobs {
            for name in &job.artifacts {
                let path = artifact_root.join(safe_component(&job.name)).join(name);
                let hashed = {
                    let path = path.clone();
                    tokio::task::spawn_blocking(move || hash_file(&path)).await
                };
                let (size, sha256) = match hashed {
                    Ok(Ok(result)) => result,
                    Ok(Err(error)) => {
                        warn!(%error, build_id = build.id, artifact = %name, "cannot read a published artifact");
                        continue;
                    }
                    Err(_) => continue,
                };
                let storage_path = format!(
                    "{}/{}/{}/{}",
                    build.project_id,
                    build.number,
                    safe_component(&job.name),
                    name
                );
                let result = sqlx::query(
                    "INSERT INTO build_artifacts (build_id, job_name, name, storage_path, size, sha256) \
                     VALUES ($1, $2, $3, $4, $5, $6) \
                     ON CONFLICT (build_id, job_name, name) \
                     DO UPDATE SET storage_path = EXCLUDED.storage_path, size = EXCLUDED.size, sha256 = EXCLUDED.sha256",
                )
                .bind(build.id)
                .bind(&job.name)
                .bind(name)
                .bind(&storage_path)
                .bind(size as i64)
                .bind(&sha256)
                .execute(&self.pool)
                .await;
                match result {
                    Ok(_) => registered.push(json!({
                        "job": job.name,
                        "name": name,
                        "size": size,
                        "sha256": sha256,
                    })),
                    Err(error) => warn!(%error, build_id = build.id, artifact = %name, "cannot record an artifact"),
                }
            }
        }
        registered
    }
}

fn status_text(status: BuildStatus) -> &'static str {
    match status {
        BuildStatus::Pending => "pending",
        BuildStatus::Running => "running",
        BuildStatus::Success => "success",
        BuildStatus::Failed => "failed",
        BuildStatus::Skipped => "skipped",
        BuildStatus::Cancelled => "cancelled",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::AppConfig;
    use crate::lifecycle::Lifecycle;
    use crate::testsupport::{TestDb, make_repo, seed_project, seed_user};
    use onedev_build::services::parse_buildspec;

    fn job(name: &str, needs: &[&str]) -> onedev_build::models::JobSpec {
        serde_json::from_value(json!({
            "name": name,
            "steps": [],
            "jobDependencies": needs.iter().map(|n| json!({"jobName": n})).collect::<Vec<_>>(),
        }))
        .unwrap()
    }

    #[test]
    fn selected_jobs_pull_in_their_dependencies_only() {
        let mut spec = BuildSpec {
            jobs: vec![job("a", &[]), job("b", &["a"]), job("c", &["b"]), job("d", &[])],
            ..BuildSpec::default()
        };
        restrict_jobs(&mut spec, &["c".to_owned()]);
        let names: Vec<&str> = spec.jobs.iter().map(|j| j.name.as_str()).collect();
        assert_eq!(names, ["a", "b", "c"]);

        assert_eq!(selected_jobs(&json!({"jobs": ["x", "y"]})), Some(vec!["x".to_owned(), "y".to_owned()]));
        assert_eq!(selected_jobs(&json!({})), None);
        assert_eq!(selected_jobs(&Value::Null), None);
    }

    #[test]
    fn refs_params_and_server_urls() {
        assert_eq!(ref_parts("refs/heads/feature/x"), ("feature/x".to_owned(), String::new()));
        assert_eq!(ref_parts("refs/tags/v1"), (String::new(), "v1".to_owned()));
        assert_eq!(ref_parts("refs/pull/3/head"), (String::new(), String::new()));

        let params = build_params(&json!({"env": "prod", "n": 3, "flag": true, "list": ["a", "b"], "none": null}));
        assert_eq!(params["env"], "prod");
        assert_eq!(params["n"], "3");
        assert_eq!(params["flag"], "true");
        assert_eq!(params["list"], "a,b");
        assert!(!params.contains_key("none"));
        assert!(build_params(&json!([1])).is_empty());

        let mut config = AppConfig::default();
        assert_eq!(server_urls(&config), ("http://127.0.0.1:18080".to_owned(), "127.0.0.1:18080".to_owned()));
        config.server.public_url = Some("https://ci.example.com/".to_owned());
        assert_eq!(server_urls(&config), ("https://ci.example.com".to_owned(), "ci.example.com".to_owned()));
    }

    fn claimed() -> ClaimedBuild {
        ClaimedBuild {
            id: 9,
            project_id: 4,
            number: 12,
            ref_name: "refs/heads/main".to_owned(),
            commit_sha: "abc123".to_owned(),
            commit_message: None,
            author_id: 1,
            trigger: "push".to_owned(),
            trigger_data: json!({"pull_request": {"number": 5}, "issue_number": 77}),
            spec: Value::Null,
            parameters: json!({"env": "prod"}),
        }
    }

    #[test]
    fn variables_expose_build_facts_and_secret_state() {
        let project = ProjectInfo {
            name: "App".to_owned(),
            full_path: "group/app".to_owned(),
            timeout_minutes: 60,
        };
        let spec = BuildSpec {
            properties: vec![json!({"name": "jdk", "value": "21"}), json!({"bad": 1})],
            ..BuildSpec::default()
        };
        let mut resolved = ResolvedSettings::default();
        resolved.secrets.insert("token".to_owned(), "token-value-1".to_owned());
        resolved.denied.insert("prod-key".to_owned());
        resolved.denied_values.push("prod-key-value".to_owned());
        resolved.properties.insert("jdk".to_owned(), "11".to_owned());
        resolved.properties.insert("node".to_owned(), "20".to_owned());

        let variables = build_variables(
            &claimed(),
            &project,
            &spec,
            &resolved,
            &("https://ci.example.com".to_owned(), "ci.example.com".to_owned()),
            "job-token",
        );
        let text = |t: &str| variables.interpolate(t).unwrap();
        assert_eq!(text("@project_path@ @project_name@ #@build_number@"), "group/app App #12");
        assert_eq!(text("@branch@|@tag@|@commit_hash@|@ref@"), "main||abc123|refs/heads/main");
        assert_eq!(text("@pull_request_number@ @issue_number@"), "5 77");
        assert_eq!(text("@server_url@ @server_host@"), "https://ci.example.com ci.example.com");
        assert_eq!(text("@param:env@ @property:jdk@ @property:node@"), "prod 21 20");
        assert_eq!(text("@secret:token@"), "token-value-1");
        assert!(variables
            .interpolate("@secret:prod-key@")
            .unwrap_err()
            .to_string()
            .contains("No authorized job secret found (project: group/app, job secret: prod-key)"));
        let masked = variables.secret_values();
        assert!(masked.contains(&"token-value-1".to_owned()) && masked.contains(&"prod-key-value".to_owned()));
    }

    #[test]
    fn webhook_payload_describes_the_build() {
        let payload = webhook_payload("build.finished", &claimed(), "group/app", "failed", Some(7), Some("boom"), Some("https://ci/"));
        assert_eq!(payload["event"], "build.finished");
        assert_eq!(payload["project"]["path"], "group/app");
        assert_eq!(payload["build"]["number"], 12);
        assert_eq!(payload["build"]["status"], "failed");
        assert_eq!(payload["build"]["duration"], 7);
        assert_eq!(payload["build"]["url"], "https://ci/group/app/~builds/12");
        assert!(webhook_payload("build.started", &claimed(), "g", "running", None, None, None)["build"]["url"].is_null());
    }

    // -- end to end ---------------------------------------------------------

    struct Env {
        db: TestDb,
        _dir: tempfile::TempDir,
        scheduler: Arc<BuildScheduler>,
        lifecycle: Arc<Lifecycle>,
        runner: Option<tokio::task::JoinHandle<()>>,
        user: i64,
        project: i64,
        repo: PathBuf,
        commit: String,
        artifacts_root: PathBuf,
    }

    impl Env {
        async fn new(files: &[(&str, &str)]) -> Option<Env> {
            let db = TestDb::new().await?;
            let dir = tempfile::tempdir().unwrap();
            let mut config = AppConfig::default();
            config.storage.local_path = dir.path().join("storage");
            config.git.repos_root = dir.path().join("repos");
            config.build.poll_interval_secs = 1;
            config.build.max_concurrent_builds = 2;
            let (repo, commit) = make_repo(&config.git.repos_root, "app", files);
            let user = seed_user(&db.pool, "builder").await;
            let project = seed_project(&db.pool, user, "app", None).await;
            let artifacts_root = config.build_artifact_dir();
            let scheduler = BuildScheduler::new(db.pool.clone(), Arc::new(config)).unwrap();
            let lifecycle = Arc::new(Lifecycle::new());
            let runner = tokio::spawn(scheduler.clone().run(lifecycle.signal()));
            Some(Env {
                db,
                _dir: dir,
                scheduler,
                lifecycle,
                runner: Some(runner),
                user,
                project,
                repo,
                commit,
                artifacts_root,
            })
        }

        async fn queue(&self, number: i32, yaml: &str, trigger_data: Value) -> i64 {
            self.queue_with_parameters(number, yaml, trigger_data, json!({})).await
        }

        async fn queue_with_parameters(&self, number: i32, yaml: &str, trigger_data: Value, parameters: Value) -> i64 {
            let spec = parse_buildspec(yaml).expect("valid spec");
            sqlx::query_scalar::<_, i64>(
                "INSERT INTO builds (project_id, number, ref_name, commit_sha, author_id, spec, trigger_data, parameters) \
                 VALUES ($1, $2, 'refs/heads/main', $3, $4, $5, $6, $7) RETURNING id",
            )
            .bind(self.project)
            .bind(number)
            .bind(&self.commit)
            .bind(self.user)
            .bind(serde_json::to_value(spec).unwrap())
            .bind(trigger_data)
            .bind(parameters)
            .fetch_one(&self.db.pool)
            .await
            .unwrap()
        }

        async fn wait_for_finish(&self, build: i64, within: Duration) -> (String, Option<String>) {
            let deadline = Instant::now() + within;
            loop {
                let (status, finished, error): (String, Option<chrono::DateTime<chrono::Utc>>, Option<String>) =
                    sqlx::query_as("SELECT status::text, finished_at, error_message FROM builds WHERE id = $1")
                        .bind(build)
                        .fetch_one(&self.db.pool)
                        .await
                        .unwrap();
                if finished.is_some() {
                    return (status, error);
                }
                assert!(Instant::now() < deadline, "build {build} did not finish in time (status {status})");
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
        }

        async fn log_text(&self, build: i64) -> String {
            sqlx::query_scalar::<_, String>("SELECT line FROM build_logs WHERE build_id = $1 ORDER BY seq")
                .bind(build)
                .fetch_all(&self.db.pool)
                .await
                .unwrap()
                .join("\n")
        }

        async fn stop(&mut self) {
            self.lifecycle.begin_shutdown();
            if let Some(runner) = self.runner.take() {
                tokio::time::timeout(Duration::from_secs(20), runner)
                    .await
                    .expect("scheduler stops")
                    .unwrap();
            }
        }
    }

    const PIPELINE: &str = r#"
version: 59
jobs:
  - name: build
    steps:
      - type: CheckoutStep
        name: checkout
      - type: CommandStep
        name: compile
        interpreter:
          type: PosixInterpreter
          commands: |
            echo "building #@build_number@ on @branch@ (@commit_hash@)"
            echo "token is @secret:token@"
            echo "param is @param:env@"
            mkdir -p dist && cp hello.txt dist/hello.txt
      - type: SetBuildVersionStep
        name: version
        buildVersion: "1.0.@build_number@"
      - type: PublishArtifactStep
        name: publish
        sourcePath: dist
        artifacts: "**"
  - name: test
    jobDependencies:
      - jobName: build
        artifacts: "**"
        destinationPath: from-build
    steps:
      - type: CheckoutStep
        name: checkout
      - type: CommandStep
        name: verify
        interpreter:
          type: PosixInterpreter
          commands: |
            cat from-build/hello.txt
            test "@build_version@" = "1.0.@build_number@"
"#;

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn a_queued_build_runs_end_to_end() {
        let Some(mut env) = Env::new(&[("hello.txt", "hello from git\n")]).await else { return };
        // a secret only the main branch may use, and one nobody may use
        sqlx::query("UPDATE projects SET build_setting = $2 WHERE id = $1")
            .bind(env.project)
            .bind(json!({"jobSecrets": [
                {"name": "token", "value": "tok-secret-value", "authorization": "on branch \"main\""},
                {"name": "other", "value": "other-secret-value", "authorization": "on branch \"release/**\""},
            ]}))
            .execute(&env.db.pool)
            .await
            .unwrap();

        let build = env
            .queue_with_parameters(1, PIPELINE, json!({}), json!({"env": "prod"}))
            .await;
        let (status, error) = env.wait_for_finish(build, Duration::from_secs(60)).await;
        let log = env.log_text(build).await;
        assert_eq!(status, "success", "error: {error:?}\n{log}");
        assert_eq!(error, None);

        // log content: variables interpolated, authorized secret masked
        assert!(log.contains("building #1 on main (") && log.contains(&env.commit), "{log}");
        assert!(log.contains("token is *******"), "{log}");
        assert!(!log.contains("tok-secret-value"), "secret leaked: {log}");
        assert!(log.contains("param is prod"), "{log}");
        assert!(log.contains("hello from git"), "dependency artifacts reached the second job: {log}");

        // job and step records
        let jobs: Vec<(String, String, i32)> = sqlx::query_as(
            "SELECT name, status::text, order_num FROM build_jobs WHERE build_id = $1 ORDER BY order_num",
        )
        .bind(build)
        .fetch_all(&env.db.pool)
        .await
        .unwrap();
        assert_eq!(
            jobs,
            vec![
                ("build".to_owned(), "success".to_owned(), 0),
                ("test".to_owned(), "success".to_owned(), 1)
            ]
        );
        let steps: Vec<(String, String)> = sqlx::query_as(
            "SELECT s.name, s.status::text FROM build_steps s JOIN build_jobs j ON j.id = s.job_id \
             WHERE j.build_id = $1 AND j.name = 'build' ORDER BY s.order_num",
        )
        .bind(build)
        .fetch_all(&env.db.pool)
        .await
        .unwrap();
        assert_eq!(
            steps.iter().map(|(n, s)| format!("{n}:{s}")).collect::<Vec<_>>(),
            ["checkout:success", "compile:success", "version:success", "publish:success"]
        );
        let dependencies: Vec<i64> = sqlx::query_scalar("SELECT dependencies FROM build_jobs WHERE build_id = $1 AND name = 'test'")
            .bind(build)
            .fetch_one(&env.db.pool)
            .await
            .map(|d: Vec<i64>| d)
            .unwrap();
        assert_eq!(dependencies.len(), 1);

        // metadata, artifacts
        let (version, duration, artifacts): (Option<String>, Option<i32>, Option<Value>) =
            sqlx::query_as("SELECT version, duration, artifacts FROM builds WHERE id = $1")
                .bind(build)
                .fetch_one(&env.db.pool)
                .await
                .unwrap();
        assert_eq!(version.as_deref(), Some("1.0.1"));
        assert!(duration.is_some());
        let artifacts = artifacts.unwrap();
        assert_eq!(artifacts[0]["job"], "build");
        assert_eq!(artifacts[0]["name"], "hello.txt");
        assert_eq!(artifacts[0]["size"], 15);
        let (path, size, sha): (String, i64, String) = sqlx::query_as(
            "SELECT storage_path, size, sha256 FROM build_artifacts WHERE build_id = $1",
        )
        .bind(build)
        .fetch_one(&env.db.pool)
        .await
        .unwrap();
        assert_eq!(size, 15);
        assert_eq!(sha.len(), 64);
        let stored = env.artifacts_root.join(&path);
        assert_eq!(std::fs::read_to_string(&stored).unwrap(), "hello from git\n", "{}", stored.display());
        assert!(!env.scheduler.config.build_work_dir().join(build.to_string()).exists(), "work dir is cleaned up");
        env.stop().await;
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn unauthorized_secrets_and_failing_steps_fail_the_build_visibly() {
        let Some(mut env) = Env::new(&[("a.txt", "x")]).await else { return };
        sqlx::query("UPDATE projects SET build_setting = $2 WHERE id = $1")
            .bind(env.project)
            .bind(json!({"jobSecrets": [
                {"name": "prod", "value": "prod-secret-value", "authorization": "on branch \"release/**\""},
            ]}))
            .execute(&env.db.pool)
            .await
            .unwrap();

        let denied = env
            .queue(
                1,
                r#"
jobs:
  - name: deploy
    steps:
      - type: CommandStep
        name: use secret
        interpreter: { type: PosixInterpreter, commands: "echo @secret:prod@" }
"#,
                json!({}),
            )
            .await;
        let (status, _) = env.wait_for_finish(denied, Duration::from_secs(60)).await;
        let log = env.log_text(denied).await;
        assert_eq!(status, "failed", "{log}");
        assert!(
            log.contains("No authorized job secret found (project: app, job secret: prod)"),
            "{log}"
        );
        assert!(!log.contains("prod-secret-value"));

        let failing = env
            .queue(
                2,
                r#"
jobs:
  - name: broken
    maxAttempts: 1
    steps:
      - type: CommandStep
        name: boom
        interpreter: { type: PosixInterpreter, commands: "echo before; exit 3" }
  - name: after
    jobDependencies: [ { jobName: broken } ]
    steps:
      - type: CommandStep
        name: never
        interpreter: { type: PosixInterpreter, commands: "echo should not run" }
"#,
                json!({}),
            )
            .await;
        let (status, _) = env.wait_for_finish(failing, Duration::from_secs(60)).await;
        let log = env.log_text(failing).await;
        assert_eq!(status, "failed", "{log}");
        assert!(log.contains("before") && !log.contains("should not run"), "{log}");
        let jobs: Vec<(String, String, Option<String>)> = sqlx::query_as(
            "SELECT name, status::text, error_message FROM build_jobs WHERE build_id = $1 ORDER BY order_num",
        )
        .bind(failing)
        .fetch_all(&env.db.pool)
        .await
        .unwrap();
        assert_eq!(jobs[0].0, "broken");
        assert_eq!(jobs[0].1, "failed");
        assert!(jobs[0].2.as_deref().unwrap_or_default().contains("exit code 3"), "{jobs:?}");
        assert_eq!((jobs[1].0.as_str(), jobs[1].1.as_str()), ("after", "skipped"));
        env.stop().await;
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn a_build_that_cannot_run_explains_why_in_its_log() {
        let Some(mut env) = Env::new(&[("a.txt", "x")]).await else { return };
        let build = env.queue(1, PIPELINE, json!({"jobs": ["nonexistent"]})).await;
        let (status, error) = env.wait_for_finish(build, Duration::from_secs(30)).await;
        assert_eq!(status, "failed");
        assert_eq!(error.as_deref(), Some("the build has no jobs to run"));
        assert!(env.log_text(build).await.contains("the build has no jobs to run"));
        env.stop().await;
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn cancelling_a_running_build_stops_it_and_keeps_the_cancelled_status() {
        let Some(mut env) = Env::new(&[("a.txt", "x")]).await else { return };
        let build = env
            .queue(
                1,
                r#"
jobs:
  - name: slow
    steps:
      - type: CommandStep
        name: wait
        interpreter: { type: PosixInterpreter, commands: "echo started; sleep 60; echo finished" }
"#,
                json!({}),
            )
            .await;
        // wait until the step is really running
        let started = Instant::now();
        while !env.log_text(build).await.contains("started") {
            assert!(started.elapsed() < Duration::from_secs(30), "build never started");
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        sqlx::query("UPDATE builds SET status = 'cancelled' WHERE id = $1 AND status = 'running'")
            .bind(build)
            .execute(&env.db.pool)
            .await
            .unwrap();
        let (status, _) = env.wait_for_finish(build, Duration::from_secs(30)).await;
        assert_eq!(status, "cancelled");
        assert!(started.elapsed() < Duration::from_secs(40), "the sleeping process was killed");
        // the command is echoed (`$ ...; echo finished`), but its last echo must not have run
        assert!(
            !env.log_text(build).await.lines().any(|line| line == "finished"),
            "the step kept running after the cancellation"
        );
        env.stop().await;
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn shutting_down_interrupts_running_builds_with_an_explanation() {
        let Some(mut env) = Env::new(&[("a.txt", "x")]).await else { return };
        let build = env
            .queue(
                1,
                r#"
jobs:
  - name: slow
    steps:
      - type: CommandStep
        name: wait
        interpreter: { type: PosixInterpreter, commands: "echo started; sleep 60" }
"#,
                json!({}),
            )
            .await;
        let started = Instant::now();
        while !env.log_text(build).await.contains("started") {
            assert!(started.elapsed() < Duration::from_secs(30), "build never started");
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        let pool = env.db.pool.clone();
        env.stop().await;
        let (status, error): (String, Option<String>) =
            sqlx::query_as("SELECT status::text, error_message FROM builds WHERE id = $1")
                .bind(build)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(status, "failed");
        assert!(error.unwrap().contains("shutting down"));
    }

    #[test]
    fn unused_import_guard() {
        // `Path`/`PathBuf` are used by the end-to-end fixtures above
        let _ = (&Path::new("."), env_repo_hint());
    }

    fn env_repo_hint() -> Option<PathBuf> {
        None
    }
}
