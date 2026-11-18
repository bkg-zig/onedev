//! Bridge between the (blocking) build runner and the database.
//!
//! The runner calls [`StepHooks`] from its own thread; every callback runs a
//! short database statement on the Tokio runtime and never fails the build:
//! progress bookkeeping is best effort, the final result is reconciled from
//! the build report afterwards.

use std::collections::HashMap;
use std::sync::Mutex;

use onedev_build::models::{BuildStatus, JobResult, StepResult};
use onedev_build::services::{ServerStep, StepHooks};
use onedev_db::DbPool;
use tokio::runtime::Handle;
use tracing::warn;

/// Longest build version / description accepted from a step.
const MAX_VERSION_LEN: usize = 255;
const MAX_DESCRIPTION_LEN: usize = 4000;

pub struct SchedulerHooks {
    runtime: Handle,
    pool: DbPool,
    build_id: i64,
    /// `build_jobs.id` by job name (rows are created when the build starts).
    job_rows: HashMap<String, i64>,
    /// Next `order_num` of a job's steps.
    step_counters: Mutex<HashMap<String, i32>>,
}

impl SchedulerHooks {
    pub fn new(runtime: Handle, pool: DbPool, build_id: i64, job_rows: HashMap<String, i64>) -> Self {
        Self {
            runtime,
            pool,
            build_id,
            job_rows,
            step_counters: Mutex::new(HashMap::new()),
        }
    }

    fn run<F>(&self, what: &str, future: F)
    where
        F: std::future::Future<Output = Result<(), sqlx::Error>>,
    {
        if let Err(error) = self.runtime.block_on(future) {
            warn!(%error, build_id = self.build_id, "build bookkeeping failed: {what}");
        }
    }

    fn set_metadata(&self, request: &ServerStep<'_>, field: &str, column: &'static str, limit: usize) -> Result<(), String> {
        let raw = request
            .step
            .extra
            .get(field)
            .and_then(|value| value.as_str())
            .ok_or_else(|| format!("step has no `{field}`"))?;
        let value = request
            .variables
            .interpolate(raw)
            .map_err(|error| error.to_string())?;
        let value = value.trim().to_owned();
        if value.is_empty() {
            return Err(format!("`{field}` is empty after interpolation"));
        }
        if value.chars().count() > limit {
            return Err(format!("`{field}` is longer than {limit} characters"));
        }
        // `column` is one of two literals chosen by the callers below
        let statement = format!("UPDATE builds SET {column} = $2 WHERE id = $1");
        self.runtime
            .block_on(
                sqlx::query(&statement)
                    .bind(self.build_id)
                    .bind(&value)
                    .execute(&self.pool),
            )
            .map_err(|error| format!("cannot store the {column}: {error}"))?;
        request.log.line(
            &request.job.name,
            request.step.display_name(),
            false,
            format!("build {column} set to `{value}`"),
        );
        if column == "version" {
            // later `@build_version@` references (in any job) see the new value
            request.variables.set_build_version(value);
        }
        Ok(())
    }
}

/// One line describing why a job failed.
pub fn failure_reason(result: &JobResult) -> Option<String> {
    if result.status != BuildStatus::Failed {
        return None;
    }
    let failed = result
        .steps
        .iter()
        .find(|step| step.status == BuildStatus::Failed);
    Some(match failed {
        Some(step) => match step.exit_code {
            Some(code) => format!("step `{}` failed with exit code {code}", step.name),
            None => format!("step `{}` failed", step.name),
        },
        None => "job failed".to_owned(),
    })
}

impl StepHooks for SchedulerHooks {
    fn job_started(&self, job: &str, attempt: u32) {
        let Some(&row) = self.job_rows.get(job) else { return };
        if let Ok(mut counters) = self.step_counters.lock() {
            counters.insert(job.to_owned(), 0);
        }
        let pool = self.pool.clone();
        self.run("job started", async move {
            if attempt > 1 {
                // the steps shown are those of the latest attempt
                sqlx::query("DELETE FROM build_steps WHERE job_id = $1")
                    .bind(row)
                    .execute(&pool)
                    .await?;
            }
            sqlx::query(
                "UPDATE build_jobs SET status = 'running', started_at = COALESCE(started_at, NOW()), \
                 finished_at = NULL, error_message = NULL WHERE id = $1",
            )
            .bind(row)
            .execute(&pool)
            .await?;
            Ok(())
        });
    }

    fn step_finished(&self, job: &str, step: &StepResult) {
        let Some(&row) = self.job_rows.get(job) else { return };
        let order = {
            let Ok(mut counters) = self.step_counters.lock() else { return };
            let counter = counters.entry(job.to_owned()).or_insert(0);
            let order = *counter;
            *counter += 1;
            order
        };
        let pool = self.pool.clone();
        let name = step.name.clone();
        let status = step.status;
        let seconds = step.duration_ms as f64 / 1000.0;
        let error = match (step.status, step.exit_code) {
            (BuildStatus::Failed, Some(code)) => Some(format!("exit code {code}")),
            _ => None,
        };
        self.run("step finished", async move {
            sqlx::query(
                "INSERT INTO build_steps (job_id, name, status, started_at, finished_at, duration, error_message, order_num) \
                 VALUES ($1, $2, $3, NOW() - make_interval(secs => $4::double precision), NOW(), $5, $6, $7)",
            )
            .bind(row)
            .bind(name)
            .bind(status)
            .bind(seconds)
            .bind(seconds.round() as i32)
            .bind(error)
            .bind(order)
            .execute(&pool)
            .await?;
            Ok(())
        });
    }

    fn job_finished(&self, result: &JobResult) {
        let Some(&row) = self.job_rows.get(&result.name) else { return };
        let pool = self.pool.clone();
        let status = result.status;
        let seconds = (result.duration_ms as f64 / 1000.0).round() as i32;
        let reason = failure_reason(result);
        self.run("job finished", async move {
            sqlx::query(
                "UPDATE build_jobs SET status = $2, finished_at = NOW(), duration = $3, error_message = $4, \
                 started_at = COALESCE(started_at, NOW()) WHERE id = $1",
            )
            .bind(row)
            .bind(status)
            .bind(seconds)
            .bind(reason)
            .execute(&pool)
            .await?;
            Ok(())
        });
    }

    fn server_step(&self, request: &ServerStep<'_>) -> Option<Result<(), String>> {
        match request.step.step_type.as_str() {
            "SetBuildVersionStep" => Some(self.set_metadata(request, "buildVersion", "version", MAX_VERSION_LEN)),
            "SetBuildDescriptionStep" => {
                Some(self.set_metadata(request, "buildDescription", "description", MAX_DESCRIPTION_LEN))
            }
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testsupport::{TestDb, seed_project, seed_user};
    use onedev_build::models::Step;
    use onedev_build::services::{LogSink, Variables};
    use std::sync::Arc;

    async fn seed(pool: &DbPool) -> (i64, HashMap<String, i64>) {
        let user = seed_user(pool, "hook-user").await;
        let project = seed_project(pool, user, "hook-project", None).await;
        let build = sqlx::query_scalar::<_, i64>(
            "INSERT INTO builds (project_id, number, ref_name, commit_sha, author_id, spec, status) \
             VALUES ($1, 1, 'main', 'abc', $2, '{}'::jsonb, 'running') RETURNING id",
        )
        .bind(project)
        .bind(user)
        .fetch_one(pool)
        .await
        .unwrap();
        let job = sqlx::query_scalar::<_, i64>(
            "INSERT INTO build_jobs (build_id, name, spec) VALUES ($1, 'build', '{}'::jsonb) RETURNING id",
        )
        .bind(build)
        .fetch_one(pool)
        .await
        .unwrap();
        (build, HashMap::from([("build".to_owned(), job)]))
    }

    fn step(step_type: &str, field: &str, value: &str) -> Step {
        serde_json::from_value(serde_json::json!({
            "type": step_type,
            "name": "meta",
            field: value,
        }))
        .unwrap()
    }

    fn job_spec() -> onedev_build::models::JobSpec {
        serde_json::from_value(serde_json::json!({"name": "build", "steps": []})).unwrap()
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn progress_is_recorded_and_retries_replace_the_steps() {
        let Some(db) = TestDb::new().await else { return };
        let (build, rows) = seed(&db.pool).await;
        let hooks = Arc::new(SchedulerHooks::new(Handle::current(), db.pool.clone(), build, rows.clone()));
        let job_row = rows["build"];

        let worker = hooks.clone();
        tokio::task::spawn_blocking(move || {
            worker.job_started("build", 1);
            worker.step_finished("build", &StepResult::new("compile", BuildStatus::Failed, Some(2)));
            worker.job_started("build", 2); // a retry
            let mut ok = StepResult::new("compile", BuildStatus::Success, Some(0));
            ok.duration_ms = 2500;
            worker.step_finished("build", &ok);
            worker.step_finished("build", &StepResult::new("test", BuildStatus::Success, Some(0)));
            worker.job_finished(&JobResult {
                name: "build".to_owned(),
                status: BuildStatus::Success,
                steps: vec![],
                artifacts: vec![],
                attempts: 2,
                duration_ms: 4000,
            });
        })
        .await
        .unwrap();

        let steps: Vec<(String, String, i32, Option<i32>)> = sqlx::query_as(
            "SELECT name, status::text, order_num, duration FROM build_steps WHERE job_id = $1 ORDER BY order_num",
        )
        .bind(job_row)
        .fetch_all(&db.pool)
        .await
        .unwrap();
        assert_eq!(
            steps,
            vec![
                ("compile".to_owned(), "success".to_owned(), 0, Some(3)),
                ("test".to_owned(), "success".to_owned(), 1, Some(0)),
            ]
        );
        let (status, duration, error): (String, Option<i32>, Option<String>) = sqlx::query_as(
            "SELECT status::text, duration, error_message FROM build_jobs WHERE id = $1",
        )
        .bind(job_row)
        .fetch_one(&db.pool)
        .await
        .unwrap();
        assert_eq!((status.as_str(), duration, error), ("success", Some(4), None));
    }

    #[test]
    fn failure_reason_names_the_failing_step() {
        let mut result = JobResult {
            name: "build".to_owned(),
            status: BuildStatus::Failed,
            steps: vec![
                StepResult::new("checkout", BuildStatus::Success, Some(0)),
                StepResult::new("compile", BuildStatus::Failed, Some(101)),
            ],
            artifacts: vec![],
            attempts: 1,
            duration_ms: 0,
        };
        assert_eq!(
            failure_reason(&result).as_deref(),
            Some("step `compile` failed with exit code 101")
        );
        result.status = BuildStatus::Success;
        assert_eq!(failure_reason(&result), None);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn metadata_steps_store_the_value_and_update_the_build_version_variable() {
        let Some(db) = TestDb::new().await else { return };
        let (build, rows) = seed(&db.pool).await;
        let hooks = Arc::new(SchedulerHooks::new(Handle::current(), db.pool.clone(), build, rows));

        let worker = hooks.clone();
        let outcome = tokio::task::spawn_blocking(move || {
            let variables = Variables::new().with_job("build_number", "7");
            let job = job_spec();
            let log = LogSink::discard();
            let workspace = std::path::PathBuf::from(".");
            let mut results = Vec::new();
            for (kind, field, value) in [
                ("SetBuildVersionStep", "buildVersion", "1.0.@build_number@"),
                ("SetBuildDescriptionStep", "buildDescription", "nightly build #@build_number@"),
                ("SetBuildVersionStep", "buildVersion", "@secret:missing@"),
                ("SetBuildVersionStep", "buildVersion", "   "),
            ] {
                let step = step(kind, field, value);
                let request = ServerStep {
                    job: &job,
                    step: &step,
                    variables: &variables,
                    log: &log,
                    workspace: &workspace,
                };
                results.push(worker.server_step(&request));
            }
            let other = step("PublishReportStep", "x", "y");
            let request = ServerStep {
                job: &job,
                step: &other,
                variables: &variables,
                log: &log,
                workspace: &workspace,
            };
            (results, worker.server_step(&request), variables.build_version())
        })
        .await
        .unwrap();

        let (results, unknown, version_variable) = outcome;
        assert_eq!(results[0], Some(Ok(())));
        assert_eq!(results[1], Some(Ok(())));
        assert!(matches!(&results[2], Some(Err(message)) if message.contains("Undefined secret")));
        assert!(matches!(&results[3], Some(Err(message)) if message.contains("empty")));
        assert_eq!(unknown, None, "other steps are not handled here");
        assert_eq!(version_variable.as_deref(), Some("1.0.7"));

        let (version, description): (Option<String>, Option<String>) =
            sqlx::query_as("SELECT version, description FROM builds WHERE id = $1")
                .bind(build)
                .fetch_one(&db.pool)
                .await
                .unwrap();
        assert_eq!(version.as_deref(), Some("1.0.7"));
        assert_eq!(description.as_deref(), Some("nightly build #7"));
    }
}
