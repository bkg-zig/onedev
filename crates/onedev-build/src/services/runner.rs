//! Build runner: executes a spec's jobs in dependency order.
//!
//! Semantics ported from OneDev:
//!
//! * jobs selected by the trigger event run after the jobs they depend on
//!   (dependencies of selected jobs run even without a trigger of their own),
//! * a job whose required dependency did not succeed is skipped,
//! * with a checkout source every job gets its own workspace and receives the
//!   artifacts of its dependencies (`jobDependencies[].artifacts` into
//!   `destinationPath`); without one all jobs share the caller's directory,
//! * a failed attempt is retried when `retryCondition` matches and attempts
//!   are left, after `retryDelay * 2^(attempt-1)` seconds,
//! * every attempt is limited by the job `timeout` and the build deadline;
//!   cancelling stops the running step and skips everything after it.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::errors::{BuildError, BuildResult};
use crate::models::{
    BuildReport, BuildSpec, BuildStatus, JobResult, JobSpec, LogLine, TriggerEvent,
};
use crate::services::executor::{
    CancelToken, CheckoutSource, ExecutorConfig, JobContext, JobExecutor, LogSink, StepHooks,
    StopReason, collect_artifacts, safe_join,
};
use crate::services::interpolation::{SecretMasker, Variables};
use crate::services::parser::{execution_order, jobs_for_event};
use crate::services::retry_condition::{RetryCondition, RetryContext};
use crate::services::templates::expand_steps;

/// Everything a run needs besides the spec.
pub struct RunContext {
    /// Base directory for workspaces (with a checkout source) or the shared
    /// workspace (without).
    pub work_dir: PathBuf,
    /// Published artifacts land in `<artifact_root>/<job>/`.
    pub artifact_root: PathBuf,
    pub log: LogSink,
    pub cancel: CancelToken,
    /// Deadline of the whole build.
    pub deadline: Option<Instant>,
    pub checkout: Option<CheckoutSource>,
    pub variables: Variables,
    pub config: ExecutorConfig,
    pub hooks: Option<Arc<dyn StepHooks>>,
    /// Scales retry delays (tests use 0.0).
    pub retry_delay_scale: f64,
}

impl RunContext {
    pub fn new(work_dir: impl Into<PathBuf>, artifact_root: impl Into<PathBuf>) -> Self {
        Self {
            work_dir: work_dir.into(),
            artifact_root: artifact_root.into(),
            log: LogSink::discard(),
            cancel: CancelToken::new(),
            deadline: None,
            checkout: None,
            variables: Variables::new(),
            config: ExecutorConfig::default(),
            hooks: None,
            retry_delay_scale: 1.0,
        }
    }

    /// Log to a channel, masking the secrets of `variables`.
    pub fn with_log(mut self, sender: Option<flume::Sender<LogLine>>) -> Self {
        self.log = LogSink::new(sender, SecretMasker::new(self.variables.secret_values()));
        self
    }
}

/// Runs builds.
#[derive(Default)]
pub struct BuildRunner {
    executor: JobExecutor,
}

impl BuildRunner {
    pub fn new() -> Self {
        Self::default()
    }

    /// Run the jobs selected by `event` in the caller's directory (no checkout
    /// source: the caller prepared the workspace). Kept for tools and tests.
    pub fn run(
        &self,
        spec: &BuildSpec,
        work_dir: &Path,
        artifact_root: &Path,
        event: &TriggerEvent,
        log: Option<flume::Sender<LogLine>>,
    ) -> BuildResult<BuildReport> {
        let ctx = RunContext::new(work_dir, artifact_root).with_log(log);
        self.run_with(spec, &ctx, event)
    }

    pub fn run_with(
        &self,
        spec: &BuildSpec,
        ctx: &RunContext,
        event: &TriggerEvent,
    ) -> BuildResult<BuildReport> {
        let started = Instant::now();
        let selected = jobs_for_event(spec, event)?;
        let order = execution_order(spec)?;
        let by_name: HashMap<&str, &JobSpec> = spec
            .jobs
            .iter()
            .map(|job| (job.name.as_str(), job))
            .collect();

        // Dependencies of selected jobs run too (vanilla builds them first).
        let mut run_set: HashSet<String> = selected.into_iter().collect();
        loop {
            let missing: Vec<String> = run_set
                .iter()
                .filter_map(|name| by_name.get(name.as_str()))
                .flat_map(|job| job.needs())
                .filter(|need| !run_set.contains(need))
                .collect();
            if missing.is_empty() {
                break;
            }
            run_set.extend(missing);
        }

        let mut results: Vec<JobResult> = Vec::new();
        let mut statuses: HashMap<String, BuildStatus> = HashMap::new();
        let mut error: Option<String> = None;

        for (index, name) in order.iter().enumerate() {
            let job = by_name[name.as_str()];
            let skip = |reason: BuildStatus| JobResult {
                name: name.clone(),
                status: reason,
                steps: Vec::new(),
                artifacts: Vec::new(),
                attempts: 0,
                duration_ms: 0,
            };

            if !run_set.contains(name) {
                let result = skip(BuildStatus::Skipped);
                statuses.insert(name.clone(), result.status);
                results.push(result);
                continue;
            }
            match ctx.cancel.reason() {
                Some(StopReason::Cancelled) => {
                    error.get_or_insert_with(|| "build was cancelled".to_owned());
                    let result = skip(BuildStatus::Cancelled);
                    statuses.insert(name.clone(), result.status);
                    results.push(result);
                    continue;
                }
                Some(StopReason::TimedOut) => {
                    error.get_or_insert_with(|| "build timed out".to_owned());
                    let result = skip(BuildStatus::Skipped);
                    statuses.insert(name.clone(), result.status);
                    results.push(result);
                    continue;
                }
                None => {}
            }
            if ctx
                .deadline
                .is_some_and(|deadline| Instant::now() >= deadline)
            {
                error.get_or_insert_with(|| "build timed out".to_owned());
                let result = skip(BuildStatus::Skipped);
                statuses.insert(name.clone(), result.status);
                results.push(result);
                continue;
            }

            let dependencies_ok = job.job_dependencies.iter().all(|dependency| {
                let status = statuses.get(&dependency.job_name).copied();
                if dependency.require_successful {
                    status == Some(BuildStatus::Success)
                } else {
                    matches!(status, Some(BuildStatus::Success | BuildStatus::Failed))
                }
            });
            if !dependencies_ok {
                ctx.log.line(
                    &job.name,
                    "",
                    true,
                    "skipped: a job it depends on did not succeed",
                );
                let result = skip(BuildStatus::Skipped);
                statuses.insert(name.clone(), result.status);
                results.push(result);
                continue;
            }

            let result = self.run_job(spec, job, index, ctx, &statuses)?;
            if let Some(hooks) = &ctx.hooks {
                hooks.job_finished(&result);
            }
            statuses.insert(name.clone(), result.status);
            results.push(result);
        }

        let cancelled = ctx.cancel.reason() == Some(StopReason::Cancelled);
        let overall = if cancelled {
            BuildStatus::Cancelled
        } else if results
            .iter()
            .filter(|job| job.status != BuildStatus::Skipped)
            .all(|job| job.status == BuildStatus::Success)
            && error.is_none()
        {
            BuildStatus::Success
        } else {
            BuildStatus::Failed
        };
        Ok(BuildReport {
            status: overall,
            jobs: results,
            duration_ms: millis(started.elapsed()),
            error,
        })
    }

    fn run_job(
        &self,
        spec: &BuildSpec,
        job: &JobSpec,
        index: usize,
        ctx: &RunContext,
        statuses: &HashMap<String, BuildStatus>,
    ) -> BuildResult<JobResult> {
        let job_started = Instant::now();
        let artifact_dir = ctx.artifact_root.join(safe_component(&job.name));
        let own_workspace = ctx.checkout.is_some();
        let workspace = if own_workspace {
            ctx.work_dir
                .join("jobs")
                .join(format!("{index}-{}", safe_component(&job.name)))
        } else {
            ctx.work_dir.clone()
        };
        let retry_condition = RetryCondition::parse(&job.retry_condition)
            .map_err(|e| BuildError::Spec(format!("job {}: {e}", job.name)))?;
        let max_attempts = job.max_attempts.max(1);
        let job_token = ctx
            .variables
            .job
            .get("job_token")
            .cloned()
            .unwrap_or_default();
        let params: BTreeMap<String, Vec<String>> = ctx
            .variables
            .params
            .iter()
            .map(|(name, value)| (name.clone(), value.split(',').map(str::to_owned).collect()))
            .collect();

        let mut attempt: u32 = 0;
        loop {
            attempt += 1;
            if let Some(hooks) = &ctx.hooks {
                hooks.job_started(&job.name, attempt);
            }
            if own_workspace {
                // every attempt starts from an empty workspace (the checkout step needs one)
                let _ = std::fs::remove_dir_all(&workspace);
            }
            std::fs::create_dir_all(&workspace)?;

            let (sink, captured) = ctx.log.capturing();
            let variables = ctx
                .variables
                .for_job(&job.name, &job_token)
                .for_workspace(&workspace);
            if attempt > 1 {
                sink.line(
                    &job.name,
                    "",
                    false,
                    format!("Retrying job (attempt {attempt} of {max_attempts})..."),
                );
            }

            let mut error_message: Option<String> = None;
            let mut timed_out = false;
            let mut steps = Vec::new();
            let mut status = BuildStatus::Failed;

            if own_workspace {
                for dependency in &job.job_dependencies {
                    if statuses.get(&dependency.job_name).is_none() {
                        continue;
                    }
                    let source = ctx.artifact_root.join(safe_component(&dependency.job_name));
                    if !source.is_dir() {
                        continue;
                    }
                    let relative = dependency.destination_path.as_deref().unwrap_or(".");
                    let Some(destination) = safe_join(&workspace, relative) else {
                        error_message = Some(format!(
                            "invalid destination path `{relative}` of dependency {}",
                            dependency.job_name
                        ));
                        break;
                    };
                    match collect_artifacts(
                        &source,
                        ".",
                        &dependency.artifacts,
                        &destination,
                        ctx.config.max_artifact_bytes,
                    ) {
                        Ok(files) => sink.line(
                            &job.name,
                            "",
                            false,
                            format!(
                                "downloaded {} artifact file(s) of job {}",
                                files.len(),
                                dependency.job_name
                            ),
                        ),
                        Err(error) => {
                            error_message = Some(format!(
                                "cannot download artifacts of job {}: {error}",
                                dependency.job_name
                            ));
                            break;
                        }
                    }
                }
            }

            if error_message.is_none() {
                match expand_steps(spec, &job.steps, &variables) {
                    Err(error) => error_message = Some(error),
                    Ok(expanded) => {
                        let attempt_deadline =
                            Instant::now() + Duration::from_secs(job.timeout.max(1));
                        let deadline = Some(
                            ctx.deadline
                                .map_or(attempt_deadline, |d| d.min(attempt_deadline)),
                        );
                        let job_ctx = JobContext {
                            work_dir: &workspace,
                            artifact_dir: Some(&artifact_dir),
                            log: sink.clone(),
                            cancel: &ctx.cancel,
                            deadline,
                            checkout: ctx.checkout.as_ref(),
                            variables: &variables,
                            config: &ctx.config,
                            network: None,
                            hooks: ctx.hooks.as_deref(),
                        };
                        match self.executor.execute(job, &expanded, &job_ctx) {
                            Err(error) => error_message = Some(error.to_string()),
                            Ok(outcome) => {
                                timed_out = outcome.timed_out;
                                if outcome.timed_out {
                                    error_message = Some("job timed out".to_owned());
                                } else if outcome.failed {
                                    error_message = outcome
                                        .steps
                                        .iter()
                                        .find(|step| step.status == BuildStatus::Failed)
                                        .map(|step| match step.exit_code {
                                            Some(code) => format!(
                                                "step \"{}\" failed with exit code {code}",
                                                step.name
                                            ),
                                            None => format!("step \"{}\" failed", step.name),
                                        });
                                }
                                steps = outcome.steps;
                                if ctx.cancel.reason() == Some(StopReason::Cancelled) {
                                    status = BuildStatus::Cancelled;
                                } else if !outcome.failed && !outcome.timed_out {
                                    status = BuildStatus::Success;
                                }
                            }
                        }
                    }
                }
            }
            if let Some(message) = &error_message
                && steps.is_empty()
            {
                sink.line(&job.name, "", true, message.clone());
            }

            let retry = status == BuildStatus::Failed
                && attempt < max_attempts
                && ctx.cancel.reason().is_none()
                && !retry_condition.is_never()
                && {
                    let log = captured
                        .lock()
                        .unwrap_or_else(|poisoned| poisoned.into_inner())
                        .clone();
                    retry_condition.matches(&RetryContext {
                        timed_out,
                        error_message: error_message.as_deref(),
                        log: &log,
                        params: &params,
                    })
                };
            if retry {
                sink.line(&job.name, "", true, "Job will be retried after a while...");
                let delay = Duration::from_secs_f64(
                    f64::from(job.retry_delay)
                        * 2f64.powi(attempt as i32 - 1)
                        * ctx.retry_delay_scale.max(0.0),
                );
                if !sleep_unless_stopped(delay, ctx) {
                    status = if ctx.cancel.reason() == Some(StopReason::Cancelled) {
                        BuildStatus::Cancelled
                    } else {
                        BuildStatus::Failed
                    };
                } else {
                    continue;
                }
            }

            let artifacts: Vec<String> = steps
                .iter()
                .flat_map(|step| step.artifacts.iter().cloned())
                .collect();
            return Ok(JobResult {
                name: job.name.clone(),
                status,
                steps,
                artifacts,
                attempts: attempt,
                duration_ms: millis(job_started.elapsed()),
            });
        }
    }
}

/// Sleep `delay` unless the run is cancelled or its deadline passes first.
/// Returns whether the full delay elapsed.
fn sleep_unless_stopped(delay: Duration, ctx: &RunContext) -> bool {
    let end = Instant::now() + delay;
    loop {
        if ctx.cancel.is_stopped() || ctx.deadline.is_some_and(|d| Instant::now() >= d) {
            return false;
        }
        let now = Instant::now();
        if now >= end {
            return true;
        }
        std::thread::sleep((end - now).min(Duration::from_millis(100)));
    }
}

fn millis(duration: Duration) -> u64 {
    u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
}

/// A job name as a single, harmless path component.
pub fn safe_component(name: &str) -> String {
    let cleaned: String = name
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || matches!(c, '-' | '_' | '.' | ' ') {
                c
            } else {
                '_'
            }
        })
        .collect();
    let trimmed = cleaned.trim();
    if trimmed.is_empty() || trimmed.chars().all(|c| c == '.') {
        "_".to_owned()
    } else {
        trimmed.to_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::parser::parse_buildspec;
    use std::sync::Mutex;

    fn spec(yaml: &str) -> BuildSpec {
        parse_buildspec(yaml).expect("valid spec")
    }

    fn command_job(name: &str, commands: &str) -> String {
        format!(
            "  - name: {name}\n    steps:\n      - type: CommandStep\n        name: run\n        interpreter:\n          type: PosixInterpreter\n          commands: {commands:?}\n"
        )
    }

    fn manual(ctx: &RunContext, spec: &BuildSpec) -> BuildReport {
        BuildRunner::new()
            .run_with(spec, ctx, &TriggerEvent::Manual)
            .unwrap()
    }

    #[test]
    fn safe_component_never_escapes() {
        assert_eq!(safe_component("build"), "build");
        assert_eq!(safe_component("../../etc"), ".._.._etc");
        assert_eq!(safe_component(".."), "_");
        assert_eq!(safe_component("a/b"), "a_b");
        assert_eq!(safe_component("  "), "_");
        assert_eq!(safe_component("Ünïcode job"), "Ünïcode job");
    }

    #[test]
    fn dependencies_of_triggered_jobs_run_even_without_their_own_trigger() {
        let s = spec(&format!(
            "version: 59\njobs:\n{}  - name: test\n    jobDependencies:\n      - jobName: prepare\n    triggers:\n      - type: BranchUpdateTrigger\n        branches: main\n    steps:\n      - type: CommandStep\n        interpreter:\n          type: PosixInterpreter\n          commands: test -f prepared\n",
            command_job("prepare", "touch prepared")
        ));
        let dir = tempfile::tempdir().unwrap();
        let ctx = RunContext::new(dir.path(), dir.path().join("artifacts"));
        let report = BuildRunner::new()
            .run_with(
                &s,
                &ctx,
                &TriggerEvent::push("main"),
            )
            .unwrap();
        assert_eq!(report.status, BuildStatus::Success, "{report:?}");
        assert_eq!(
            report
                .jobs
                .iter()
                .filter(|j| j.status == BuildStatus::Success)
                .count(),
            2
        );
    }

    #[test]
    fn retries_follow_the_condition_and_back_off() {
        // fails twice (counter file), succeeds on the third attempt
        let script = "n=$(cat counter 2>/dev/null || echo 0); n=$((n+1)); echo $n > counter; echo attempt-$n; test $n -ge 3";
        let yaml = format!(
            "version: 59\njobs:\n  - name: flaky\n    maxAttempts: 3\n    retryDelay: 1\n    retryCondition: '\"Log\" contains \"attempt-\"'\n    steps:\n      - type: CommandStep\n        interpreter:\n          type: PosixInterpreter\n          commands: {script:?}\n"
        );
        let s = spec(&yaml);
        let dir = tempfile::tempdir().unwrap();
        let mut ctx = RunContext::new(dir.path(), dir.path().join("artifacts"));
        ctx.retry_delay_scale = 0.0;
        let report = manual(&ctx, &s);
        assert_eq!(report.status, BuildStatus::Success, "{report:?}");
        assert_eq!(report.jobs[0].attempts, 3);

        // `never` does not retry
        let never = yaml.replace("'\"Log\" contains \"attempt-\"'", "never");
        let dir = tempfile::tempdir().unwrap();
        let mut ctx = RunContext::new(dir.path(), dir.path().join("artifacts"));
        ctx.retry_delay_scale = 0.0;
        let report = manual(&ctx, &spec(&never));
        assert_eq!(report.status, BuildStatus::Failed);
        assert_eq!(report.jobs[0].attempts, 1);

        // a condition that does not match does not retry either
        let other = yaml.replace("contains \"attempt-\"", "contains \"nothing like that\"");
        let dir = tempfile::tempdir().unwrap();
        let mut ctx = RunContext::new(dir.path(), dir.path().join("artifacts"));
        ctx.retry_delay_scale = 0.0;
        let report = manual(&ctx, &spec(&other));
        assert_eq!(report.jobs[0].attempts, 1);

        // attempts are capped by maxAttempts
        let capped = yaml.replace("maxAttempts: 3", "maxAttempts: 2");
        let dir = tempfile::tempdir().unwrap();
        let mut ctx = RunContext::new(dir.path(), dir.path().join("artifacts"));
        ctx.retry_delay_scale = 0.0;
        let report = manual(&ctx, &spec(&capped));
        assert_eq!(report.status, BuildStatus::Failed);
        assert_eq!(report.jobs[0].attempts, 2);
    }

    #[test]
    fn job_timeout_fails_the_job_and_kills_its_processes() {
        let yaml = "version: 59\njobs:\n  - name: slow\n    timeout: 1\n    steps:\n      - type: CommandStep\n        interpreter:\n          type: PosixInterpreter\n          commands: sleep 300\n      - type: CommandStep\n        condition: ALWAYS\n        interpreter:\n          type: PosixInterpreter\n          commands: echo cleanup\n";
        let dir = tempfile::tempdir().unwrap();
        let (tx, rx) = flume::unbounded();
        let ctx = RunContext::new(dir.path(), dir.path().join("artifacts")).with_log(Some(tx));
        let started = Instant::now();
        let report = manual(&ctx, &spec(yaml));
        assert!(started.elapsed() < Duration::from_secs(30));
        assert_eq!(report.status, BuildStatus::Failed);
        assert_eq!(report.jobs[0].status, BuildStatus::Failed);
        assert_eq!(
            report.jobs[0].steps[1].status,
            BuildStatus::Skipped,
            "nothing starts after the deadline"
        );
        assert!(!rx.try_iter().any(|l| l.line == "cleanup"));
    }

    #[test]
    fn cancelling_stops_the_run_and_marks_the_rest_cancelled() {
        let yaml = format!(
            "version: 59\njobs:\n{}  - name: after\n    jobDependencies:\n      - jobName: first\n    steps:\n      - type: CommandStep\n        interpreter:\n          type: PosixInterpreter\n          commands: echo never\n",
            command_job("first", "sleep 300")
        );
        let dir = tempfile::tempdir().unwrap();
        let ctx = RunContext::new(dir.path(), dir.path().join("artifacts"));
        let cancel = ctx.cancel.clone();
        let handle = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(500));
            cancel.cancel();
        });
        let started = Instant::now();
        let report = manual(&ctx, &spec(&yaml));
        handle.join().unwrap();
        assert!(started.elapsed() < Duration::from_secs(30));
        assert_eq!(report.status, BuildStatus::Cancelled);
        assert_eq!(report.jobs[0].status, BuildStatus::Cancelled);
        assert_eq!(report.jobs[1].status, BuildStatus::Cancelled);
        assert_eq!(report.error.as_deref(), Some("build was cancelled"));
    }

    #[test]
    fn jobs_get_their_own_workspace_and_receive_dependency_artifacts() {
        let work = tempfile::tempdir().unwrap();
        let (_keep, bare, commit) =
            crate::services::executor::test_repo(&[("seed.txt", "from-repo")]);
        let yaml = "version: 59\njobs:\n  - name: build\n    steps:\n      - type: CheckoutStep\n        name: checkout\n      - type: CommandStep\n        name: make\n        interpreter:\n          type: PosixInterpreter\n          commands: mkdir -p out && echo built-by-build > out/app.txt && echo private > only-in-build.txt\n      - type: PublishArtifactStep\n        name: publish\n        artifacts: out/app.txt\n  - name: test\n    jobDependencies:\n      - jobName: build\n        artifacts: out/**\n        destinationPath: deps\n    steps:\n      - type: CheckoutStep\n        name: checkout\n      - type: CommandStep\n        name: verify\n        interpreter:\n          type: PosixInterpreter\n          commands: test \"$(cat deps/out/app.txt)\" = built-by-build && test -f seed.txt && test ! -e only-in-build.txt && echo isolated\n";
        let mut ctx = RunContext::new(work.path(), work.path().join("artifacts"));
        ctx.checkout = Some(CheckoutSource {
            repo_path: bare,
            commit,
            lfs_objects_dir: None,
        });
        let (tx, rx) = flume::unbounded();
        ctx = ctx.with_log(Some(tx));
        let report = manual(&ctx, &spec(yaml));
        let logs: Vec<String> = rx.try_iter().map(|l| l.line).collect();
        assert_eq!(report.status, BuildStatus::Success, "{report:?}\n{logs:#?}");
        assert!(logs.iter().any(|l| l == "isolated"), "{logs:?}");
        assert!(work.path().join("artifacts/build/out/app.txt").is_file());
        assert_eq!(report.jobs[0].artifacts, vec!["out/app.txt"]);
    }

    #[test]
    fn templates_expand_and_unknown_templates_fail_the_job() {
        let yaml = "version: 59\nstepTemplates:\n  - name: greet\n    paramSpecs:\n      - type: TextParam\n        name: who\n    steps:\n      - type: CommandStep\n        name: hello\n        interpreter:\n          type: PosixInterpreter\n          commands: echo hello @param:who@\njobs:\n  - name: j\n    steps:\n      - type: UseTemplateStep\n        templateName: greet\n        paramMatrix:\n          - name: who\n            valuesProvider:\n              type: SpecifiedValues\n              values:\n                - [world]\n                - [rust]\n";
        let dir = tempfile::tempdir().unwrap();
        let (tx, rx) = flume::unbounded();
        let ctx = RunContext::new(dir.path(), dir.path().join("artifacts")).with_log(Some(tx));
        let report = manual(&ctx, &spec(yaml));
        assert_eq!(report.status, BuildStatus::Success, "{report:?}");
        let logs: Vec<String> = rx.try_iter().map(|l| l.line).collect();
        assert!(
            logs.contains(&"hello world".to_owned()) && logs.contains(&"hello rust".to_owned()),
            "{logs:?}"
        );
        let names: Vec<_> = report.jobs[0]
            .steps
            .iter()
            .map(|s| s.name.as_str())
            .collect();
        assert_eq!(names, ["hello", "hello (2)"]);

        let bad = yaml.replace("templateName: greet", "templateName: missing");
        let report = manual(
            &RunContext::new(dir.path(), dir.path().join("a2")),
            &spec(&bad),
        );
        assert_eq!(report.status, BuildStatus::Failed);
    }

    #[test]
    fn hooks_receive_job_and_step_events() {
        #[derive(Default)]
        struct Events(Mutex<Vec<String>>);
        impl StepHooks for Events {
            fn job_started(&self, job: &str, attempt: u32) {
                self.0
                    .lock()
                    .unwrap()
                    .push(format!("start {job} #{attempt}"));
            }
            fn step_finished(&self, job: &str, step: &crate::models::StepResult) {
                self.0
                    .lock()
                    .unwrap()
                    .push(format!("step {job}/{} {}", step.name, step.status));
            }
            fn job_finished(&self, result: &JobResult) {
                self.0
                    .lock()
                    .unwrap()
                    .push(format!("done {} {}", result.name, result.status));
            }
        }
        let events = Arc::new(Events::default());
        let dir = tempfile::tempdir().unwrap();
        let mut ctx = RunContext::new(dir.path(), dir.path().join("artifacts"));
        ctx.hooks = Some(events.clone());
        let report = manual(
            &ctx,
            &spec(&format!(
                "version: 59\njobs:\n{}",
                command_job("only", "true")
            )),
        );
        assert_eq!(report.status, BuildStatus::Success);
        assert_eq!(
            *events.0.lock().unwrap(),
            [
                "start only #1",
                "step only/run Success",
                "done only Success"
            ]
        );
    }
}
