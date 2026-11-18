//! Turning events into builds.
//!
//! A build is queued for an event (a push, a tag, a pull request, a schedule
//! or a manual run). The `.onedev-buildspec.yml` of the commit decides which
//! jobs run: those with a trigger that fires for the event (a manual run names
//! its jobs). The result is a `pending` row in `builds` that carries a
//! snapshot of the spec and the selected jobs in `trigger_data.jobs`; the
//! [scheduler](super) claims it from there.
//!
//! [`BuildTrigger`] is the entry point for code that knows what happened
//! (the API, the schedule service); [`subscribe`] connects it to the event
//! bus for everything the git and pull request layers publish.

use std::path::PathBuf;
use std::sync::Arc;

use async_trait::async_trait;
use onedev_build::models::{BuildSpec, PullRequestEvent, TriggerEvent};
use onedev_build::services::{jobs_for_event, parse_buildspec};
use onedev_core::events::{DomainEvent, EventBus, EventError, EventHandler};
use onedev_db::DbPool;
use serde_json::{Map, Value, json};
use sqlx::Row;
use tokio::sync::Notify;
use tracing::{debug, info, warn};

use super::QUEUE_CHANNEL;
use crate::services::git_cli::{GitCli, PathChange};
use crate::services::project_paths::{project_full_path, repository_dir};

/// Where a repository keeps its build spec.
pub const BUILD_SPEC_PATH: &str = ".onedev-buildspec.yml";
/// The user builds belong to when nobody started them (`OneDev` system user).
pub const SYSTEM_USER_ID: i64 = -1;
/// Larger build specs are rejected.
const MAX_SPEC_BYTES: usize = 1024 * 1024;
/// `builds.ref_name` is `VARCHAR(255)`.
const MAX_REF_LEN: usize = 255;
/// Longest commit message stored with a build.
const MAX_MESSAGE_LEN: usize = 64 * 1024;

/// `build_trigger` of a build.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TriggerKind {
    Push,
    Tag,
    PullRequest,
    Schedule,
    Manual,
    Api,
    Webhook,
}

impl TriggerKind {
    pub fn as_str(self) -> &'static str {
        match self {
            TriggerKind::Push => "push",
            TriggerKind::Tag => "tag",
            TriggerKind::PullRequest => "pull_request",
            TriggerKind::Schedule => "schedule",
            TriggerKind::Manual => "manual",
            TriggerKind::Api => "api",
            TriggerKind::Webhook => "webhook",
        }
    }
}

/// Everything needed to queue one build.
#[derive(Debug, Clone)]
pub struct TriggerRequest {
    pub project_id: i64,
    pub kind: TriggerKind,
    /// Full ref (`refs/heads/main`, `refs/tags/v1`, `refs/pull/3/head`).
    pub ref_name: String,
    pub commit_sha: String,
    pub author_id: i64,
    /// What happened; decides which jobs the triggers of the spec select.
    pub event: TriggerEvent,
    /// Jobs to run; `None` lets the triggers decide (a [`TriggerEvent::Manual`]
    /// event selects every job).
    pub jobs: Option<Vec<String>>,
    /// Values of the job parameters (`builds.parameters`, a JSON object).
    pub parameters: Value,
    /// Extra facts stored in `builds.trigger_data`.
    pub data: Map<String, Value>,
}

impl TriggerRequest {
    pub fn new(
        project_id: i64,
        kind: TriggerKind,
        ref_name: impl Into<String>,
        commit_sha: impl Into<String>,
        author_id: i64,
        event: TriggerEvent,
    ) -> Self {
        Self {
            project_id,
            kind,
            ref_name: ref_name.into(),
            commit_sha: commit_sha.into(),
            author_id,
            event,
            jobs: None,
            parameters: Value::Null,
            data: Map::new(),
        }
    }

    /// Run exactly these jobs (and what they depend on).
    pub fn with_jobs(mut self, jobs: Vec<String>) -> Self {
        self.jobs = Some(jobs);
        self
    }

    pub fn with_parameters(mut self, parameters: Value) -> Self {
        self.parameters = parameters;
        self
    }
}

/// Why nothing was queued.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SkipReason {
    /// The project does not run builds (disabled, archived, in maintenance).
    BuildsDisabled,
    /// The commit has no `.onedev-buildspec.yml`.
    NoBuildSpec,
    /// No job has a trigger that fires for the event.
    NoMatchingJobs,
    /// The event is not about a branch update (a deleted branch, another ref).
    NotApplicable,
}

impl SkipReason {
    pub fn as_str(self) -> &'static str {
        match self {
            SkipReason::BuildsDisabled => "builds_disabled",
            SkipReason::NoBuildSpec => "no_build_spec",
            SkipReason::NoMatchingJobs => "no_matching_jobs",
            SkipReason::NotApplicable => "not_applicable",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Enqueued {
    Queued {
        build_id: i64,
        number: i32,
        jobs: Vec<String>,
    },
    Skipped(SkipReason),
}

#[derive(Debug, thiserror::Error)]
pub enum TriggerError {
    #[error("project {0} does not exist")]
    ProjectNotFound(i64),
    #[error("the build spec is invalid: {0}")]
    InvalidSpec(String),
    #[error("the build spec has no job named `{0}`")]
    UnknownJob(String),
    #[error("parameters must be a JSON object")]
    InvalidParameters,
    #[error("cannot read the repository: {0}")]
    Repository(String),
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
}

impl TriggerError {
    /// Short label for metrics.
    pub fn label(&self) -> &'static str {
        match self {
            TriggerError::ProjectNotFound(_) => "project_not_found",
            TriggerError::InvalidSpec(_) => "invalid_spec",
            TriggerError::UnknownJob(_) => "unknown_job",
            TriggerError::InvalidParameters => "invalid_parameters",
            TriggerError::Repository(_) => "repository",
            TriggerError::Database(_) => "database",
        }
    }
}

/// An all-zero object id (a ref that does not exist before/after an update).
pub fn is_zero_sha(sha: &str) -> bool {
    !sha.is_empty() && sha.bytes().all(|byte| byte == b'0')
}

/// The project a build is for.
struct Target {
    id: i64,
    path: String,
    repo: PathBuf,
}

pub struct BuildTrigger {
    pool: DbPool,
    git: GitCli,
    repos_root: PathBuf,
    wake: Arc<Notify>,
}

impl BuildTrigger {
    /// `wake` is the scheduler's wake handle ([`super::BuildScheduler::wake_handle`]).
    pub fn new(pool: DbPool, git: GitCli, repos_root: PathBuf, wake: Arc<Notify>) -> Self {
        Self {
            pool,
            git,
            repos_root,
            wake,
        }
    }

    // -- entry points ---------------------------------------------------------

    /// Queue a build described by `request`.
    pub async fn enqueue(&self, request: TriggerRequest) -> Result<Enqueued, TriggerError> {
        if !request.parameters.is_null() && !request.parameters.is_object() {
            return Err(TriggerError::InvalidParameters);
        }
        let target = match self.locate(request.project_id).await? {
            Ok(target) => target,
            Err(reason) => return Ok(Enqueued::Skipped(reason)),
        };
        let spec = match self.read_spec(&target, &request.commit_sha).await? {
            Ok(spec) => spec,
            Err(reason) => return Ok(Enqueued::Skipped(reason)),
        };
        self.queue(&target, &spec, request).await
    }

    /// A branch was created or updated by a push.
    pub async fn branch_updated(
        &self,
        project_id: i64,
        ref_name: &str,
        old_sha: &str,
        new_sha: &str,
        user_id: i64,
    ) -> Result<Enqueued, TriggerError> {
        let Some(branch) = ref_name.strip_prefix("refs/heads/") else {
            return Ok(Enqueued::Skipped(SkipReason::NotApplicable));
        };
        if is_zero_sha(new_sha) {
            return Ok(Enqueued::Skipped(SkipReason::NotApplicable)); // branch deleted
        }
        let target = match self.locate(project_id).await? {
            Ok(target) => target,
            Err(reason) => return Ok(Enqueued::Skipped(reason)),
        };
        let spec = match self.read_spec(&target, new_sha).await? {
            Ok(spec) => spec,
            Err(reason) => return Ok(Enqueued::Skipped(reason)),
        };
        let changed_paths = if spec.has_path_filters() {
            self.push_changed_paths(&target, old_sha, new_sha).await
        } else {
            None
        };
        let request = TriggerRequest::new(
            project_id,
            TriggerKind::Push,
            ref_name,
            new_sha,
            user_id,
            TriggerEvent::Push {
                branch: branch.to_owned(),
                changed_paths,
            },
        );
        self.queue(&target, &spec, request).await
    }

    /// A tag was created.
    pub async fn tag_created(
        &self,
        project_id: i64,
        tag: &str,
        target_sha: &str,
        user_id: i64,
    ) -> Result<Enqueued, TriggerError> {
        let target = match self.locate(project_id).await? {
            Ok(target) => target,
            Err(reason) => return Ok(Enqueued::Skipped(reason)),
        };
        // an annotated tag points at a tag object: build the commit it names
        let commit = self
            .git
            .resolve_commit(&target.repo, target_sha)
            .await
            .map_err(|e| TriggerError::Repository(format!("{e:#}")))?
            .ok_or_else(|| TriggerError::Repository(format!("tag {tag} does not point to a commit")))?;
        let spec = match self.read_spec(&target, &commit).await? {
            Ok(spec) => spec,
            Err(reason) => return Ok(Enqueued::Skipped(reason)),
        };
        let branches = if spec.has_tag_branch_filters() {
            match self.git.branches_containing(&target.repo, &commit).await {
                Ok(branches) => Some(branches),
                Err(error) => {
                    warn!(%error, project_id, tag, "cannot determine the branches containing the tagged commit");
                    None
                }
            }
        } else {
            None
        };
        let request = TriggerRequest::new(
            project_id,
            TriggerKind::Tag,
            format!("refs/tags/{tag}"),
            commit,
            user_id,
            TriggerEvent::Tag {
                tag: tag.to_owned(),
                branches,
            },
        );
        self.queue(&target, &spec, request).await
    }

    /// A pull request was opened or changed (`Updated`), merged or discarded.
    /// Builds run in the target project, on the head of the pull request (the
    /// merge commit for merged ones).
    pub async fn pull_request_event(
        &self,
        pull_request_id: i64,
        kind: PullRequestEvent,
    ) -> Result<Enqueued, TriggerError> {
        let row = sqlx::query(
            "SELECT number, author_id, source_project_id, source_branch, target_project_id, \
                    target_branch, head_commit_sha, base_commit_sha, merge_base_sha, merge_commit_sha \
             FROM pull_requests WHERE id = $1",
        )
        .bind(pull_request_id)
        .fetch_optional(&self.pool)
        .await?;
        let Some(row) = row else {
            return Ok(Enqueued::Skipped(SkipReason::NotApplicable));
        };
        let number: i32 = row.try_get("number")?;
        let author_id: i64 = row.try_get("author_id")?;
        let source_project_id: i64 = row.try_get("source_project_id")?;
        let source_branch: String = row.try_get("source_branch")?;
        let target_project_id: i64 = row.try_get("target_project_id")?;
        let target_branch: String = row.try_get("target_branch")?;
        let head: String = row.try_get("head_commit_sha")?;
        let base: String = row.try_get("base_commit_sha")?;
        let merge_base: Option<String> = row.try_get("merge_base_sha")?;
        let merge_commit: Option<String> = row.try_get("merge_commit_sha")?;

        let commit = match kind {
            PullRequestEvent::Merged => merge_commit.unwrap_or_else(|| head.clone()),
            _ => head.clone(),
        };
        let target = match self.locate(target_project_id).await? {
            Ok(target) => target,
            Err(reason) => return Ok(Enqueued::Skipped(reason)),
        };
        let spec = match self.read_spec(&target, &commit).await? {
            Ok(spec) => spec,
            Err(reason) => return Ok(Enqueued::Skipped(reason)),
        };
        let changed_paths = if spec.has_path_filters() {
            let from = merge_base.as_deref().unwrap_or(&base);
            match self.git.changed_paths(&target.repo, from, &head).await {
                Ok(changes) => Some(changes.into_iter().map(path_of).collect()),
                Err(error) => {
                    warn!(%error, pull_request_id, "cannot determine the files of the pull request");
                    None
                }
            }
        } else {
            None
        };

        let mut request = TriggerRequest::new(
            target_project_id,
            TriggerKind::PullRequest,
            format!("refs/pull/{number}/head"),
            commit,
            author_id,
            TriggerEvent::PullRequest {
                kind,
                target_branch: target_branch.clone(),
                changed_paths,
            },
        );
        request.data.insert(
            "pull_request".to_owned(),
            json!({
                "id": pull_request_id,
                "number": number,
                "event": match kind {
                    PullRequestEvent::Updated => "updated",
                    PullRequestEvent::Merged => "merged",
                    PullRequestEvent::Discarded => "discarded",
                },
                "source_project_id": source_project_id,
                "source_branch": source_branch,
                "target_branch": target_branch,
            }),
        );
        self.queue(&target, &spec, request).await
    }

    // -- internals ------------------------------------------------------------

    /// The project and its repository; `Err(reason)` when it runs no builds.
    async fn locate(&self, project_id: i64) -> Result<Result<Target, SkipReason>, TriggerError> {
        let row = sqlx::query(
            "SELECT builds_enabled, status::text AS status FROM projects WHERE id = $1 AND deleted_at IS NULL",
        )
        .bind(project_id)
        .fetch_optional(&self.pool)
        .await?;
        let Some(row) = row else {
            return Err(TriggerError::ProjectNotFound(project_id));
        };
        let enabled: bool = row.try_get("builds_enabled")?;
        let status: String = row.try_get("status")?;
        if !enabled || status != "active" {
            return Ok(Err(SkipReason::BuildsDisabled));
        }
        let path = project_full_path(&self.pool, project_id)
            .await?
            .ok_or(TriggerError::ProjectNotFound(project_id))?;
        let repo = repository_dir(&self.repos_root, &path)
            .filter(|dir| dir.is_dir())
            .ok_or_else(|| TriggerError::Repository(format!("the repository of {path} does not exist")))?;
        Ok(Ok(Target {
            id: project_id,
            path,
            repo,
        }))
    }

    /// The build spec of `commit`; `Err(reason)` when it has none.
    async fn read_spec(&self, target: &Target, commit: &str) -> Result<Result<BuildSpec, SkipReason>, TriggerError> {
        let bytes = self
            .git
            .show_file(&target.repo, commit, BUILD_SPEC_PATH)
            .await
            .map_err(|e| TriggerError::Repository(format!("{e:#}")))?;
        let Some(bytes) = bytes else {
            return Ok(Err(SkipReason::NoBuildSpec));
        };
        if bytes.len() > MAX_SPEC_BYTES {
            return Err(TriggerError::InvalidSpec(format!(
                "{BUILD_SPEC_PATH} is larger than {} KiB",
                MAX_SPEC_BYTES / 1024
            )));
        }
        let yaml = String::from_utf8(bytes)
            .map_err(|_| TriggerError::InvalidSpec(format!("{BUILD_SPEC_PATH} is not valid UTF-8")))?;
        parse_buildspec(&yaml)
            .map(Ok)
            .map_err(|e| TriggerError::InvalidSpec(e.to_string()))
    }

    async fn push_changed_paths(&self, target: &Target, old_sha: &str, new_sha: &str) -> Option<Vec<String>> {
        let result = if is_zero_sha(old_sha) {
            // a new branch: what its head commit changes
            self.git.commit_changed_paths(&target.repo, new_sha).await
        } else {
            self.git
                .changed_paths(&target.repo, old_sha, new_sha)
                .await
                .map(|changes| changes.into_iter().map(path_of).collect())
        };
        match result {
            Ok(paths) => Some(paths),
            Err(error) => {
                warn!(%error, project_id = target.id, "cannot determine the files of the push");
                None
            }
        }
    }

    /// Select the jobs and insert the build.
    async fn queue(&self, target: &Target, spec: &BuildSpec, request: TriggerRequest) -> Result<Enqueued, TriggerError> {
        let jobs = select_jobs(target, spec, &request)?;
        if jobs.is_empty() {
            return Ok(Enqueued::Skipped(SkipReason::NoMatchingJobs));
        }
        if request.ref_name.len() > MAX_REF_LEN {
            return Err(TriggerError::Repository(format!(
                "ref name is longer than {MAX_REF_LEN} characters"
            )));
        }

        let message = match self.git.commit_message(&target.repo, &request.commit_sha).await {
            Ok(message) => message.map(|mut text| {
                if text.len() > MAX_MESSAGE_LEN {
                    let mut cut = MAX_MESSAGE_LEN;
                    while !text.is_char_boundary(cut) {
                        cut -= 1;
                    }
                    text.truncate(cut);
                }
                text
            }),
            Err(error) => {
                debug!(%error, commit = %request.commit_sha, "cannot read the commit message");
                None
            }
        };
        let spec_json = serde_json::to_value(spec).map_err(|e| TriggerError::InvalidSpec(e.to_string()))?;
        let TriggerRequest {
            kind,
            ref_name,
            commit_sha,
            author_id,
            parameters,
            mut data,
            ..
        } = request;
        data.insert("jobs".to_owned(), json!(jobs));
        let parameters = if parameters.is_null() { json!({}) } else { parameters };

        let mut tx = self.pool.begin().await?;
        let number: i32 = sqlx::query_scalar(
            "INSERT INTO project_number_counters (project_id, build_counter) VALUES ($1, 1) \
             ON CONFLICT (project_id) DO UPDATE \
             SET build_counter = project_number_counters.build_counter + 1 \
             RETURNING build_counter",
        )
        .bind(target.id)
        .fetch_one(&mut *tx)
        .await?;
        let build_id: i64 = sqlx::query_scalar(
            "INSERT INTO builds (project_id, number, ref_name, commit_sha, commit_message, author_id, \
                                 status, trigger, trigger_data, spec, parameters) \
             VALUES ($1, $2, $3, $4, $5, $6, 'pending', $7::build_trigger, $8, $9, $10) \
             RETURNING id",
        )
        .bind(target.id)
        .bind(number)
        .bind(&ref_name)
        .bind(&commit_sha)
        .bind(message)
        .bind(author_id)
        .bind(kind.as_str())
        .bind(Value::Object(data))
        .bind(spec_json)
        .bind(parameters)
        .fetch_one(&mut *tx)
        .await?;
        // delivered when the transaction commits
        sqlx::query("SELECT pg_notify($1, $2)")
            .bind(QUEUE_CHANNEL)
            .bind(build_id.to_string())
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;

        self.wake.notify_one();
        metrics::counter!("builds_queued_total", "trigger" => kind.as_str()).increment(1);
        Ok(Enqueued::Queued {
            build_id,
            number,
            jobs,
        })
    }
}

fn path_of(change: PathChange) -> String {
    match change {
        PathChange::Upsert(path) | PathChange::Deleted(path) => path,
    }
}

/// Jobs to run for the request: the named ones, or those whose triggers fire.
fn select_jobs(target: &Target, spec: &BuildSpec, request: &TriggerRequest) -> Result<Vec<String>, TriggerError> {
    match &request.jobs {
        Some(names) => {
            let mut selected: Vec<String> = Vec::new();
            for name in names {
                if !spec.jobs.iter().any(|job| &job.name == name) {
                    return Err(TriggerError::UnknownJob(name.clone()));
                }
                if !selected.contains(name) {
                    selected.push(name.clone());
                }
            }
            Ok(selected)
        }
        None => jobs_for_event(&spec.for_project(&target.path), &request.event)
            .map_err(|e| TriggerError::InvalidSpec(e.to_string())),
    }
}

// ---------------------------------------------------------------------------
// Event bus
// ---------------------------------------------------------------------------

/// Event types the build trigger reacts to.
const EVENT_TYPES: [&str; 6] = [
    "PushReceived",
    "TagCreated",
    "MergeRequestCreated",
    "MergeRequestUpdated",
    "MergeRequestMerged",
    "MergeRequestClosed",
];

/// Log what happened to an event; a trigger problem must never fail the code
/// that published the event (the push is already accepted).
fn report(what: &str, project_id: Option<i64>, result: Result<Enqueued, TriggerError>) {
    match result {
        Ok(Enqueued::Queued { build_id, number, jobs }) => {
            info!(build_id, number, ?jobs, what, project_id, "build queued");
        }
        Ok(Enqueued::Skipped(reason)) => {
            debug!(reason = reason.as_str(), what, project_id, "no build queued");
        }
        Err(error) => {
            metrics::counter!("build_trigger_errors_total", "reason" => error.label()).increment(1);
            warn!(%error, what, project_id, "cannot queue a build");
        }
    }
}

struct EventTrigger {
    trigger: Arc<BuildTrigger>,
}

#[async_trait]
impl EventHandler for EventTrigger {
    async fn handle(&self, event: &DomainEvent) -> Result<(), EventError> {
        match event {
            DomainEvent::PushReceived(push) => {
                let result = self
                    .trigger
                    .branch_updated(
                        push.project_id,
                        &push.ref_name,
                        &push.before_sha,
                        &push.after_sha,
                        push.pusher_id,
                    )
                    .await;
                report("push", Some(push.project_id), result);
            }
            DomainEvent::TagCreated(tag) => {
                let result = self
                    .trigger
                    .tag_created(tag.project_id, &tag.tag_name, &tag.target_sha, tag.created_by_id)
                    .await;
                report("tag", Some(tag.project_id), result);
            }
            DomainEvent::MergeRequestCreated(pull_request) => {
                let result = self
                    .trigger
                    .pull_request_event(pull_request.id, PullRequestEvent::Updated)
                    .await;
                report("pull request opened", Some(pull_request.project_id), result);
            }
            // only a new head commit starts builds; edits of the title etc. do not
            DomainEvent::MergeRequestUpdated(pull_request) if pull_request.changes.contains_key("head_commit_sha") => {
                let result = self
                    .trigger
                    .pull_request_event(pull_request.id, PullRequestEvent::Updated)
                    .await;
                report("pull request updated", None, result);
            }
            DomainEvent::MergeRequestMerged(pull_request) => {
                let result = self
                    .trigger
                    .pull_request_event(pull_request.id, PullRequestEvent::Merged)
                    .await;
                report("pull request merged", None, result);
            }
            DomainEvent::MergeRequestClosed(pull_request) => {
                let result = self
                    .trigger
                    .pull_request_event(pull_request.id, PullRequestEvent::Discarded)
                    .await;
                report("pull request discarded", None, result);
            }
            _ => {}
        }
        Ok(())
    }

    fn event_types(&self) -> Vec<&'static str> {
        EVENT_TYPES.to_vec()
    }

    fn handler_id(&self) -> &str {
        "build-trigger"
    }
}

/// Queue builds for the events published on `bus`.
pub async fn subscribe(bus: &dyn EventBus, trigger: Arc<BuildTrigger>) -> Result<(), EventError> {
    for event_type in EVENT_TYPES {
        bus.subscribe(
            event_type,
            Box::new(EventTrigger {
                trigger: trigger.clone(),
            }),
        )
        .await?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testsupport::{TestDb, make_repo, push_commit, seed_project, seed_user};
    use onedev_core::events::{InMemoryEventBus, PushReceived, TagCreated};
    use std::process::Command;

    const STEP: &str = "steps:\n      - type: CommandStep\n        interpreter:\n          type: PosixInterpreter\n          commands: \"true\"";

    fn spec_yaml() -> String {
        format!(
            r#"
version: 59
jobs:
  - name: build
    {STEP}
    triggers:
      - type: BranchUpdateTrigger
        branches: main release/**
  - name: test
    jobDependencies: [ {{ jobName: build }} ]
    {STEP}
  - name: deploy
    jobDependencies: [ {{ jobName: test }} ]
    {STEP}
  - name: docs
    {STEP}
    triggers:
      - type: BranchUpdateTrigger
        paths: docs/**
      - type: TagCreateTrigger
        tags: v*
        branches: main
  - name: on-pr
    {STEP}
    triggers:
      - type: PullRequestUpdateTrigger
        branches: main
"#
        )
    }

    struct Fixture {
        db: TestDb,
        _dir: tempfile::TempDir,
        trigger: Arc<BuildTrigger>,
        wake: Arc<Notify>,
        user: i64,
        project: i64,
        repo: PathBuf,
        commit: String,
    }

    const ZERO: &str = "0000000000000000000000000000000000000000";

    async fn fixture(files: &[(&str, &str)]) -> Option<Fixture> {
        let db = TestDb::new().await?;
        let dir = tempfile::tempdir().unwrap();
        let repos_root = dir.path().join("repos");
        let (repo, commit) = make_repo(&repos_root, "app", files);
        let user = seed_user(&db.pool, "pusher").await;
        let project = seed_project(&db.pool, user, "app", None).await;
        let wake = Arc::new(Notify::new());
        let trigger = Arc::new(BuildTrigger::new(db.pool.clone(), GitCli::new("git"), repos_root, wake.clone()));
        Some(Fixture {
            db,
            _dir: dir,
            trigger,
            wake,
            user,
            project,
            repo,
            commit,
        })
    }

    async fn build_row(pool: &DbPool, id: i64) -> (String, String, String, String, i64, Value, Value) {
        sqlx::query_as(
            "SELECT status::text, trigger::text, ref_name, commit_sha, author_id, trigger_data, spec \
             FROM builds WHERE id = $1",
        )
        .bind(id)
        .fetch_one(pool)
        .await
        .unwrap()
    }

    fn queued(result: Result<Enqueued, TriggerError>) -> (i64, i32, Vec<String>) {
        match result.expect("no error") {
            Enqueued::Queued { build_id, number, jobs } => (build_id, number, jobs),
            other => panic!("expected a queued build, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn a_push_queues_the_triggered_jobs_only() {
        let Some(f) = fixture(&[(BUILD_SPEC_PATH, &spec_yaml()), ("src/a.rs", "x")]).await else { return };

        let result = f
            .trigger
            .branch_updated(f.project, "refs/heads/main", ZERO, &f.commit, f.user)
            .await;
        let (id, number, jobs) = queued(result);
        // `test` and `deploy` have no trigger, `docs` filters on paths: only `build`
        assert_eq!((number, jobs.as_slice()), (1, ["build".to_owned()].as_slice()));

        let (status, trigger, ref_name, sha, author, data, spec) = build_row(&f.db.pool, id).await;
        assert_eq!(status, "pending");
        assert_eq!(trigger, "push");
        assert_eq!(ref_name, "refs/heads/main");
        assert_eq!(sha, f.commit);
        assert_eq!(author, f.user);
        assert_eq!(data["jobs"], json!(["build"]));
        assert_eq!(spec["jobs"].as_array().unwrap().len(), 5, "the whole spec is snapshotted");
        let message: Option<String> = sqlx::query_scalar("SELECT commit_message FROM builds WHERE id = $1")
            .bind(id)
            .fetch_one(&f.db.pool)
            .await
            .unwrap();
        assert!(message.is_some_and(|m| !m.is_empty()));
        // the scheduler is woken without waiting for its poll interval
        tokio::time::timeout(std::time::Duration::from_secs(1), f.wake.notified())
            .await
            .expect("scheduler was woken");

        // numbers count up per project
        let (_, second, _) = queued(
            f.trigger
                .branch_updated(f.project, "refs/heads/release/1.0", ZERO, &f.commit, f.user)
                .await,
        );
        assert_eq!(second, 2);

        // nothing to do: other branch, deleted branch, other refs
        for (name, new) in [
            ("refs/heads/feature/x", f.commit.as_str()),
            ("refs/heads/main", ZERO),
            ("refs/notes/commits", f.commit.as_str()),
        ] {
            let result = f.trigger.branch_updated(f.project, name, ZERO, new, f.user).await.unwrap();
            assert!(matches!(result, Enqueued::Skipped(_)), "{name}: {result:?}");
        }
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM builds").fetch_one(&f.db.pool).await.unwrap();
        assert_eq!(count, 2);
    }

    #[tokio::test]
    async fn path_filters_look_at_the_files_a_push_changed() {
        let Some(f) = fixture(&[(BUILD_SPEC_PATH, &spec_yaml()), ("src/a.rs", "x")]).await else { return };

        // a push that only touches sources: `docs` does not fire (and `build`
        // is limited to main/release, so on another branch nothing does)
        let src_only = push_commit(&f.repo, &[("src/b.rs", Some("y"))], "sources");
        let result = f
            .trigger
            .branch_updated(f.project, "refs/heads/topic", &f.commit, &src_only, f.user)
            .await
            .unwrap();
        assert_eq!(result, Enqueued::Skipped(SkipReason::NoMatchingJobs));

        // a push that touches docs
        let docs = push_commit(&f.repo, &[("docs/guide.md", Some("# guide"))], "docs");
        let (_, _, jobs) = queued(
            f.trigger
                .branch_updated(f.project, "refs/heads/topic", &src_only, &docs, f.user)
                .await,
        );
        assert_eq!(jobs, ["docs"]);

        // deleting a docs file counts as a change as well
        let removed = push_commit(&f.repo, &[("docs/guide.md", None)], "remove docs");
        let (_, _, jobs) = queued(
            f.trigger
                .branch_updated(f.project, "refs/heads/topic", &docs, &removed, f.user)
                .await,
        );
        assert_eq!(jobs, ["docs"]);

        // a brand new branch: the head commit decides
        let (_, _, jobs) = queued(
            f.trigger
                .branch_updated(f.project, "refs/heads/new-topic", ZERO, &docs, f.user)
                .await,
        );
        assert_eq!(jobs, ["docs"]);
    }

    #[tokio::test]
    async fn tags_need_the_tagged_commit_on_a_matching_branch() {
        let Some(f) = fixture(&[(BUILD_SPEC_PATH, &spec_yaml()), ("src/a.rs", "x")]).await else { return };

        let (id, _, jobs) = queued(f.trigger.tag_created(f.project, "v1.0", &f.commit, f.user).await);
        assert_eq!(jobs, ["docs"]);
        let (_, trigger, ref_name, sha, _, _, _) = build_row(&f.db.pool, id).await;
        assert_eq!((trigger.as_str(), ref_name.as_str(), sha.as_str()), ("tag", "refs/tags/v1.0", f.commit.as_str()));

        // the tag name has to match, too
        let not_a_release = f.trigger.tag_created(f.project, "nightly", &f.commit, f.user).await.unwrap();
        assert_eq!(not_a_release, Enqueued::Skipped(SkipReason::NoMatchingJobs));

        // a commit that is on no branch matching `main`
        let git = |args: &[&str]| {
            let output = Command::new("git")
                .arg("--git-dir")
                .arg(&f.repo)
                .args(args)
                .env("GIT_AUTHOR_NAME", "T")
                .env("GIT_AUTHOR_EMAIL", "t@example.com")
                .env("GIT_COMMITTER_NAME", "T")
                .env("GIT_COMMITTER_EMAIL", "t@example.com")
                .output()
                .unwrap();
            assert!(output.status.success(), "{args:?}: {}", String::from_utf8_lossy(&output.stderr));
            String::from_utf8_lossy(&output.stdout).trim().to_owned()
        };
        let tree = git(&["rev-parse", "main^{tree}"]);
        let side = git(&["commit-tree", &tree, "-m", "side"]);
        git(&["update-ref", "refs/heads/side", &side]);
        let off_main = f.trigger.tag_created(f.project, "v2.0", &side, f.user).await.unwrap();
        assert_eq!(off_main, Enqueued::Skipped(SkipReason::NoMatchingJobs));

        // annotated tags are peeled to their commit
        git(&["tag", "-a", "-m", "release", "v3.0", "main"]);
        let tag_object = git(&["rev-parse", "refs/tags/v3.0"]);
        assert_ne!(tag_object, f.commit);
        let (id, _, _) = queued(f.trigger.tag_created(f.project, "v3.0", &tag_object, f.user).await);
        let (_, _, _, sha, _, _, _) = build_row(&f.db.pool, id).await;
        assert_eq!(sha, f.commit);
    }

    #[tokio::test]
    async fn manual_requests_name_their_jobs_and_are_validated() {
        let Some(f) = fixture(&[(BUILD_SPEC_PATH, &spec_yaml())]).await else { return };
        let request = |jobs: Option<Vec<&str>>| {
            let mut request = TriggerRequest::new(
                f.project,
                TriggerKind::Manual,
                "refs/heads/main",
                f.commit.clone(),
                f.user,
                TriggerEvent::Manual,
            );
            request.jobs = jobs.map(|names| names.into_iter().map(str::to_owned).collect());
            request
        };

        // an explicit job (without any trigger) runs, dependencies come with it at run time
        let (id, _, jobs) = queued(
            f.trigger
                .enqueue(request(Some(vec!["deploy", "deploy"])).with_parameters(json!({"env": "prod"})))
                .await,
        );
        assert_eq!(jobs, ["deploy"]);
        let parameters: Value = sqlx::query_scalar("SELECT parameters FROM builds WHERE id = $1")
            .bind(id)
            .fetch_one(&f.db.pool)
            .await
            .unwrap();
        assert_eq!(parameters, json!({"env": "prod"}));

        // no explicit jobs: a manual run starts everything
        let (_, _, all) = queued(f.trigger.enqueue(request(None)).await);
        assert_eq!(all.len(), 5);

        assert!(matches!(
            f.trigger.enqueue(request(Some(vec!["nope"]))).await,
            Err(TriggerError::UnknownJob(name)) if name == "nope"
        ));
        assert!(matches!(
            f.trigger.enqueue(request(None).with_parameters(json!(["not", "an", "object"]))).await,
            Err(TriggerError::InvalidParameters)
        ));
        assert_eq!(
            f.trigger.enqueue(request(Some(vec![]))).await.unwrap(),
            Enqueued::Skipped(SkipReason::NoMatchingJobs)
        );
        let missing = TriggerRequest::new(
            999_999,
            TriggerKind::Manual,
            "refs/heads/main",
            f.commit.clone(),
            f.user,
            TriggerEvent::Manual,
        );
        assert!(matches!(
            f.trigger.enqueue(missing).await,
            Err(TriggerError::ProjectNotFound(999_999))
        ));
    }

    #[tokio::test]
    async fn broken_missing_and_disabled_build_specs_queue_nothing() {
        let Some(f) = fixture(&[("README.md", "no spec here")]).await else { return };
        let push = |sha: String| {
            let trigger = f.trigger.clone();
            let (project, user) = (f.project, f.user);
            async move { trigger.branch_updated(project, "refs/heads/main", ZERO, &sha, user).await }
        };
        assert_eq!(push(f.commit.clone()).await.unwrap(), Enqueued::Skipped(SkipReason::NoBuildSpec));

        let broken = push_commit(&f.repo, &[(BUILD_SPEC_PATH, Some("jobs: [this is: not valid"))], "broken");
        assert!(matches!(push(broken).await, Err(TriggerError::InvalidSpec(_))));
        let no_jobs = push_commit(&f.repo, &[(BUILD_SPEC_PATH, Some("version: 59\njobs: []\n"))], "empty");
        assert!(matches!(push(no_jobs).await, Err(TriggerError::InvalidSpec(_))));

        let good = push_commit(&f.repo, &[(BUILD_SPEC_PATH, Some(&spec_yaml()))], "good");
        queued(push(good.clone()).await);

        sqlx::query("UPDATE projects SET builds_enabled = FALSE WHERE id = $1")
            .bind(f.project)
            .execute(&f.db.pool)
            .await
            .unwrap();
        assert_eq!(push(good.clone()).await.unwrap(), Enqueued::Skipped(SkipReason::BuildsDisabled));
        sqlx::query("UPDATE projects SET builds_enabled = TRUE, status = 'archived' WHERE id = $1")
            .bind(f.project)
            .execute(&f.db.pool)
            .await
            .unwrap();
        assert_eq!(push(good).await.unwrap(), Enqueued::Skipped(SkipReason::BuildsDisabled));
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM builds").fetch_one(&f.db.pool).await.unwrap();
        assert_eq!(count, 1);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn concurrent_pushes_get_distinct_build_numbers() {
        let Some(f) = fixture(&[(BUILD_SPEC_PATH, &spec_yaml())]).await else { return };
        let mut tasks = Vec::new();
        for _ in 0..8 {
            let trigger = f.trigger.clone();
            let (project, user, commit) = (f.project, f.user, f.commit.clone());
            tasks.push(tokio::spawn(async move {
                trigger
                    .branch_updated(project, "refs/heads/main", ZERO, &commit, user)
                    .await
            }));
        }
        let mut numbers = Vec::new();
        for task in tasks {
            numbers.push(queued(task.await.unwrap()).1);
        }
        numbers.sort_unstable();
        assert_eq!(numbers, (1..=8).collect::<Vec<i32>>());
    }

    #[tokio::test]
    async fn pull_requests_build_in_the_target_project_with_their_origin_recorded() {
        let Some(f) = fixture(&[(BUILD_SPEC_PATH, &spec_yaml()), ("src/a.rs", "x")]).await else { return };
        let head = push_commit(&f.repo, &[("src/b.rs", Some("y"))], "feature work");
        let pull_request: i64 = sqlx::query_scalar(
            "INSERT INTO pull_requests (project_id, number, title, author_id, source_project_id, source_branch, \
                                        target_project_id, target_branch, head_commit_sha, base_commit_sha) \
             VALUES ($1, 1, 'Feature', $2, $1, 'feature', $1, 'main', $3, $4) RETURNING id",
        )
        .bind(f.project)
        .bind(f.user)
        .bind(&head)
        .bind(&f.commit)
        .fetch_one(&f.db.pool)
        .await
        .unwrap();

        let (id, _, jobs) = queued(f.trigger.pull_request_event(pull_request, PullRequestEvent::Updated).await);
        assert_eq!(jobs, ["on-pr"]);
        let (_, trigger, ref_name, sha, _, data, _) = build_row(&f.db.pool, id).await;
        assert_eq!((trigger.as_str(), ref_name.as_str(), sha.as_str()), ("pull_request", "refs/pull/1/head", head.as_str()));
        assert_eq!(data["pull_request"]["source_branch"], "feature");
        assert_eq!(data["pull_request"]["target_branch"], "main");
        assert_eq!(data["pull_request"]["event"], "updated");
        assert_eq!(data["pull_request"]["source_project_id"], f.project);

        // no trigger reacts to merging or discarding in this spec
        for kind in [PullRequestEvent::Merged, PullRequestEvent::Discarded] {
            assert_eq!(
                f.trigger.pull_request_event(pull_request, kind).await.unwrap(),
                Enqueued::Skipped(SkipReason::NoMatchingJobs)
            );
        }
        assert_eq!(
            f.trigger.pull_request_event(424_242, PullRequestEvent::Updated).await.unwrap(),
            Enqueued::Skipped(SkipReason::NotApplicable)
        );
    }

    #[tokio::test]
    async fn the_event_bus_starts_builds_and_never_fails_the_publisher() {
        let Some(f) = fixture(&[(BUILD_SPEC_PATH, &spec_yaml())]).await else { return };
        let bus = InMemoryEventBus::new();
        subscribe(&bus, f.trigger.clone()).await.unwrap();

        let now = chrono::Utc::now();
        bus.publish(DomainEvent::PushReceived(PushReceived {
            project_id: f.project,
            ref_name: "refs/heads/main".to_owned(),
            before_sha: ZERO.to_owned(),
            after_sha: f.commit.clone(),
            commits: Vec::new(),
            pusher_id: f.user,
            created_at: now,
        }))
        .await
        .expect("publishing works");
        bus.publish(DomainEvent::TagCreated(TagCreated {
            project_id: f.project,
            tag_name: "v1.0".to_owned(),
            target_sha: f.commit.clone(),
            created_by_id: f.user,
            created_at: now,
        }))
        .await
        .expect("publishing works");
        // a push to a project that does not exist is logged, not propagated
        bus.publish(DomainEvent::PushReceived(PushReceived {
            project_id: 987_654,
            ref_name: "refs/heads/main".to_owned(),
            before_sha: ZERO.to_owned(),
            after_sha: f.commit.clone(),
            commits: Vec::new(),
            pusher_id: f.user,
            created_at: now,
        }))
        .await
        .expect("a failing trigger does not fail the publisher");

        let rows: Vec<(String, String)> =
            sqlx::query_as("SELECT trigger::text, ref_name FROM builds ORDER BY number")
                .fetch_all(&f.db.pool)
                .await
                .unwrap();
        assert_eq!(
            rows,
            vec![
                ("push".to_owned(), "refs/heads/main".to_owned()),
                ("tag".to_owned(), "refs/tags/v1.0".to_owned()),
            ]
        );
    }

    #[test]
    fn zero_ids_and_kinds() {
        assert!(is_zero_sha(ZERO));
        assert!(is_zero_sha(&"0".repeat(64)));
        assert!(!is_zero_sha(""));
        assert!(!is_zero_sha("abc0000000000000000000000000000000000000"));
        assert_eq!(TriggerKind::PullRequest.as_str(), "pull_request");
        assert_eq!(SkipReason::NoBuildSpec.as_str(), "no_build_spec");
        assert_eq!(TriggerError::InvalidParameters.label(), "invalid_parameters");
    }
}
