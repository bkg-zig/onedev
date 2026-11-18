//! Job executors.
//!
//! A job is a list of steps run in order inside a workspace directory:
//!
//! * `CheckoutStep` clones the repository at the build's commit (optionally
//!   shallow, with submodules and LFS content),
//! * `CommandStep` runs shell commands - inside a Docker container (the
//!   default and the only mode that isolates untrusted code) or directly on
//!   the host (`ExecutorMode::Host`, for trusted installations only),
//! * `PublishArtifactStep` copies files matching glob patterns into the
//!   build's artifact store.
//!
//! Every external process runs in its own process group. Cancelling the
//! build, a job/step timeout or the end of a step terminates the whole group
//! (SIGTERM, then SIGKILL after a grace period), so no orphaned processes
//! survive; containers are stopped by name. Step output is streamed line by
//! line (long lines are split) to the log channel.

use std::ffi::OsString;
use std::io::{BufRead, BufReader, Read};
use std::path::{Component, Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crate::errors::BuildResult;
use crate::models::{BuildStatus, ExecuteCondition, JobResult, JobSpec, LogLine, Step, StepResult};
use crate::services::interpolation::{SecretMasker, Variables};

const MAX_LINE: u64 = 16 * 1024;
const POLL: Duration = Duration::from_millis(25);
const READER_DRAIN: Duration = Duration::from_secs(3);
/// Mount point of the workspace inside step containers (same as vanilla).
pub const CONTAINER_WORKSPACE: &str = "/onedev-build/workspace";

// ---------------------------------------------------------------------------
// Cancellation
// ---------------------------------------------------------------------------

/// Why a run was stopped from outside.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StopReason {
    Cancelled,
    TimedOut,
}

/// Cooperative stop signal shared between the scheduler and the runner.
#[derive(Debug, Clone, Default)]
pub struct CancelToken(Arc<AtomicU8>);

impl CancelToken {
    pub fn new() -> Self {
        Self::default()
    }

    /// Stop because a user cancelled the build.
    pub fn cancel(&self) {
        let _ = self
            .0
            .compare_exchange(0, 1, Ordering::SeqCst, Ordering::SeqCst);
    }

    /// Stop because the build exceeded its time budget.
    pub fn time_out(&self) {
        let _ = self
            .0
            .compare_exchange(0, 2, Ordering::SeqCst, Ordering::SeqCst);
    }

    pub fn reason(&self) -> Option<StopReason> {
        match self.0.load(Ordering::SeqCst) {
            1 => Some(StopReason::Cancelled),
            2 => Some(StopReason::TimedOut),
            _ => None,
        }
    }

    pub fn is_stopped(&self) -> bool {
        self.reason().is_some()
    }
}

// ---------------------------------------------------------------------------
// Configuration
// ---------------------------------------------------------------------------

/// Where `CommandStep`s run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ExecutorMode {
    /// Inside Docker containers (isolated; the production default).
    Docker,
    /// Directly on the host with `sh`. Anyone who can push a build spec runs
    /// code as the server user: only for fully trusted installations.
    #[default]
    Host,
}

#[derive(Debug, Clone)]
pub struct ExecutorConfig {
    pub mode: ExecutorMode,
    pub docker_binary: PathBuf,
    pub git_binary: PathBuf,
    /// Image for command steps that do not name one.
    pub default_image: Option<String>,
    /// Time between SIGTERM and SIGKILL.
    pub term_grace: Duration,
    /// `docker run --memory` (e.g. `4g`).
    pub container_memory: Option<String>,
    /// `docker run --cpus` (e.g. `2`).
    pub container_cpus: Option<String>,
    pub container_pids_limit: Option<u32>,
    /// Upper bound of the bytes a job may publish as artifacts.
    pub max_artifact_bytes: u64,
}

impl Default for ExecutorConfig {
    fn default() -> Self {
        Self {
            mode: ExecutorMode::Host,
            docker_binary: PathBuf::from("docker"),
            git_binary: PathBuf::from("git"),
            default_image: None,
            term_grace: Duration::from_secs(5),
            container_memory: None,
            container_cpus: None,
            container_pids_limit: Some(4096),
            max_artifact_bytes: 1024 * 1024 * 1024,
        }
    }
}

/// Repository content a `CheckoutStep` materialises.
#[derive(Debug, Clone)]
pub struct CheckoutSource {
    /// Bare repository on disk.
    pub repo_path: PathBuf,
    pub commit: String,
    /// `objects/` directory of the project's LFS store (for `withLfs`).
    pub lfs_objects_dir: Option<PathBuf>,
}

/// Context a job runs in.
pub struct JobContext<'a> {
    pub work_dir: &'a Path,
    /// Where `PublishArtifactStep` copies files (`<artifact root>/<job>`).
    pub artifact_dir: Option<&'a Path>,
    pub log: LogSink,
    pub cancel: &'a CancelToken,
    /// Hard stop for this job (job timeout and build deadline combined).
    pub deadline: Option<Instant>,
    pub checkout: Option<&'a CheckoutSource>,
    /// Build variables: `@name@` interpolation and `ONEDEV_NAME` environment.
    pub variables: &'a Variables,
    pub config: &'a ExecutorConfig,
    /// Docker network joined by step containers (job services live there).
    pub network: Option<&'a str>,
    /// Progress callbacks and server-side step handlers.
    pub hooks: Option<&'a dyn StepHooks>,
}

/// What happened to a job.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JobOutcome {
    pub steps: Vec<StepResult>,
    /// A non-optional step failed.
    pub failed: bool,
    /// The job hit its deadline.
    pub timed_out: bool,
}

// ---------------------------------------------------------------------------
// Log output and hooks
// ---------------------------------------------------------------------------

/// Output channel of a job: forwards lines to the log consumer with secret
/// values masked (vanilla replaces secrets in build logs by `*******`).
/// Optionally keeps the tail of the output for retry-condition evaluation.
#[derive(Clone, Default)]
pub struct LogSink {
    sender: Option<flume::Sender<LogLine>>,
    masker: Arc<SecretMasker>,
    capture: Option<Arc<Mutex<String>>>,
}

/// Upper bound of the text kept by a capturing sink.
const CAPTURE_LIMIT: usize = 1024 * 1024;

impl LogSink {
    pub fn new(sender: Option<flume::Sender<LogLine>>, masker: SecretMasker) -> Self {
        Self {
            sender,
            masker: Arc::new(masker),
            capture: None,
        }
    }

    /// A sink that drops everything.
    pub fn discard() -> Self {
        Self::default()
    }

    /// Same sink that additionally records the (masked) log text; the handle
    /// reads it back with [`LogSink::captured`].
    pub fn capturing(&self) -> (LogSink, Arc<Mutex<String>>) {
        let buffer = Arc::new(Mutex::new(String::new()));
        let mut sink = self.clone();
        sink.capture = Some(buffer.clone());
        (sink, buffer)
    }

    pub fn line(&self, job: &str, step: &str, stderr: bool, text: impl Into<String>) {
        let mut line = text.into();
        if !self.masker.is_empty() {
            line = self.masker.mask(&line);
        }
        if let Some(capture) = &self.capture {
            let mut buffer = capture
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            buffer.push_str(&line);
            buffer.push('\n');
            if buffer.len() > CAPTURE_LIMIT * 2 {
                // keep the tail: failures are usually reported last
                let mut cut = buffer.len() - CAPTURE_LIMIT;
                while !buffer.is_char_boundary(cut) {
                    cut += 1;
                }
                buffer.drain(..cut);
            }
        }
        if let Some(sender) = &self.sender {
            let _ = sender.send(LogLine {
                job: job.to_owned(),
                step: step.to_owned(),
                stderr,
                line,
            });
        }
    }
}

/// A step that needs the server itself (build metadata, tags, branches,
/// pull requests, reports, ...).
pub struct ServerStep<'a> {
    pub job: &'a JobSpec,
    pub step: &'a Step,
    pub variables: &'a Variables,
    pub log: &'a LogSink,
    pub workspace: &'a Path,
}

/// Callbacks the embedding application (the build scheduler) supplies.
pub trait StepHooks: Send + Sync {
    fn job_started(&self, _job: &str, _attempt: u32) {}
    fn step_finished(&self, _job: &str, _step: &StepResult) {}
    fn job_finished(&self, _result: &JobResult) {}
    /// Run a server-side step. `None` means "not handled here"; the step then
    /// fails with an explicit "not supported" message.
    fn server_step(&self, _request: &ServerStep<'_>) -> Option<Result<(), String>> {
        None
    }
}

// ---------------------------------------------------------------------------
// Process plumbing
// ---------------------------------------------------------------------------

fn emit(log: &LogSink, job: &str, step: &str, stderr: bool, line: String) {
    log.line(job, step, stderr, line);
}

/// How a process ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ProcessEnd {
    Exited(Option<i32>),
    Stopped(StopReason),
    DeadlineHit,
}

struct ProcessSpec {
    program: OsString,
    args: Vec<OsString>,
    envs: Vec<(String, String)>,
    cwd: PathBuf,
    /// Container to stop when the process is terminated early.
    container: Option<String>,
    /// Clear the inherited environment (git helpers) instead of extending it.
    clean_env: bool,
}

fn spawn_reader<R: Read + Send + 'static>(
    reader: R,
    log: LogSink,
    job: String,
    step: String,
    stderr: bool,
) -> std::sync::mpsc::Receiver<()> {
    let (done_tx, done_rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let mut reader = BufReader::new(reader);
        let mut buf = Vec::new();
        loop {
            buf.clear();
            // Bounded: a process printing megabytes without newline cannot blow up memory.
            match (&mut reader).take(MAX_LINE).read_until(b'\n', &mut buf) {
                Ok(0) | Err(_) => break,
                Ok(_) => {
                    while matches!(buf.last(), Some(b'\n' | b'\r')) {
                        buf.pop();
                    }
                    emit(
                        &log,
                        &job,
                        &step,
                        stderr,
                        String::from_utf8_lossy(&buf).into_owned(),
                    );
                }
            }
        }
        let _ = done_tx.send(());
    });
    done_rx
}

#[cfg(unix)]
fn signal_group(pid: u32, signal: libc::c_int) {
    // SAFETY: kill(2) with a negative pid addresses a process group; it only
    // sends a signal and touches no memory of this process.
    unsafe {
        libc::kill(-(pid as libc::pid_t), signal);
    }
}

#[cfg(unix)]
fn terminate_group(child: &mut Child, grace: Duration) {
    signal_group(child.id(), libc::SIGTERM);
    let deadline = Instant::now() + grace;
    while Instant::now() < deadline {
        if matches!(child.try_wait(), Ok(Some(_))) {
            break;
        }
        std::thread::sleep(POLL);
    }
    signal_group(child.id(), libc::SIGKILL);
    let _ = child.wait();
}

#[cfg(not(unix))]
fn terminate_group(child: &mut Child, _grace: Duration) {
    let _ = child.kill();
    let _ = child.wait();
}

fn stop_container(config: &ExecutorConfig, name: &str) {
    let _ = Command::new(&config.docker_binary)
        .args([
            "stop",
            "--time",
            &config.term_grace.as_secs().to_string(),
            name,
        ])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
}

/// Run a process to completion, streaming its output into the job log.
fn run_process(
    spec: ProcessSpec,
    ctx: &JobContext<'_>,
    job: &str,
    step: &str,
    deadline: Option<Instant>,
) -> std::io::Result<ProcessEnd> {
    let mut command = Command::new(&spec.program);
    command.args(&spec.args).current_dir(&spec.cwd);
    if spec.clean_env {
        command.env_clear();
        command.env(
            "PATH",
            std::env::var_os("PATH").unwrap_or_else(|| "/usr/local/bin:/usr/bin:/bin".into()),
        );
    }
    command
        .envs(spec.envs.iter().map(|(k, v)| (k.as_str(), v.as_str())))
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        command.process_group(0);
    }

    let mut child = command.spawn()?;
    let stdout = child
        .stdout
        .take()
        .map(|out| spawn_reader(out, ctx.log.clone(), job.to_owned(), step.to_owned(), false));
    let stderr = child
        .stderr
        .take()
        .map(|err| spawn_reader(err, ctx.log.clone(), job.to_owned(), step.to_owned(), true));

    let end = loop {
        match child.try_wait()? {
            Some(status) => {
                #[cfg(unix)]
                let code = {
                    use std::os::unix::process::ExitStatusExt;
                    status.code().or_else(|| status.signal().map(|s| 128 + s))
                };
                #[cfg(not(unix))]
                let code = status.code();
                break ProcessEnd::Exited(code);
            }
            None => {
                let stop = ctx.cancel.reason();
                let overdue = deadline.is_some_and(|d| Instant::now() >= d);
                if stop.is_some() || overdue {
                    if let Some(container) = &spec.container {
                        stop_container(ctx.config, container);
                    }
                    terminate_group(&mut child, ctx.config.term_grace);
                    break match stop {
                        Some(reason) => ProcessEnd::Stopped(reason),
                        None => ProcessEnd::DeadlineHit,
                    };
                }
                std::thread::sleep(POLL);
            }
        }
    };

    // Whatever the step left running in the background must not outlive it.
    #[cfg(unix)]
    signal_group(child.id(), libc::SIGKILL);
    let _ = child.wait();
    let drain_until = Instant::now() + READER_DRAIN;
    for done in [stdout, stderr].into_iter().flatten() {
        let _ = done.recv_timeout(drain_until.saturating_duration_since(Instant::now()));
    }
    Ok(end)
}

// ---------------------------------------------------------------------------
// Executor
// ---------------------------------------------------------------------------

/// Runs the steps of one job.
#[derive(Debug, Default, Clone, Copy)]
pub struct JobExecutor;

/// Former name of [`JobExecutor`].
pub type HostExecutor = JobExecutor;

impl JobExecutor {
    /// Execute `steps` of `job`. `steps` must already be template-expanded.
    pub fn execute(
        &self,
        job: &JobSpec,
        steps: &[Step],
        ctx: &JobContext<'_>,
    ) -> BuildResult<JobOutcome> {
        let mut results: Vec<StepResult> = Vec::new();
        let mut failed = false;
        let mut timed_out = false;
        let record = |results: &mut Vec<StepResult>, result: StepResult| {
            if let Some(hooks) = ctx.hooks {
                hooks.step_finished(&job.name, &result);
            }
            results.push(result);
        };

        for (index, step) in steps.iter().enumerate() {
            let name = step.display_name().to_owned();

            // Once the job was cancelled or ran out of time nothing else may start,
            // not even steps with condition ALWAYS (the job is being torn down).
            let overdue = ctx.deadline.is_some_and(|d| Instant::now() >= d);
            if overdue {
                timed_out = true;
            }
            match ctx.cancel.reason() {
                Some(StopReason::Cancelled) => {
                    record(
                        &mut results,
                        StepResult::new(name, BuildStatus::Cancelled, None),
                    );
                    continue;
                }
                Some(StopReason::TimedOut) => {
                    timed_out = true;
                    record(
                        &mut results,
                        StepResult::new(name, BuildStatus::Skipped, None),
                    );
                    continue;
                }
                None if overdue => {
                    record(
                        &mut results,
                        StepResult::new(name, BuildStatus::Skipped, None),
                    );
                    continue;
                }
                None => {}
            }
            let should_run = match step.condition {
                ExecuteCondition::Successful => !failed,
                ExecuteCondition::Always => true,
                ExecuteCondition::Failed => failed,
            };
            if !should_run {
                record(
                    &mut results,
                    StepResult::new(name, BuildStatus::Skipped, None),
                );
                continue;
            }

            let started = Instant::now();
            let mut result = self.run_step(job, step, index, ctx);
            result.duration_ms = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);

            if result.status == BuildStatus::Failed {
                if !step.optional {
                    failed = true;
                }
                if ctx.deadline.is_some_and(|d| Instant::now() >= d) {
                    timed_out = true;
                }
            }
            if ctx.cancel.reason() == Some(StopReason::TimedOut) {
                timed_out = true;
            }
            record(&mut results, result);
        }
        Ok(JobOutcome {
            steps: results,
            failed,
            timed_out,
        })
    }

    fn run_step(
        &self,
        job: &JobSpec,
        step: &Step,
        _index: usize,
        ctx: &JobContext<'_>,
    ) -> StepResult {
        let name = step.display_name().to_owned();
        let fail = |message: String| {
            emit(&ctx.log, &job.name, &name, true, message);
            StepResult::new(name.clone(), BuildStatus::Failed, None)
        };
        if let Some(hooks) = ctx.hooks {
            let request = ServerStep {
                job,
                step,
                variables: ctx.variables,
                log: &ctx.log,
                workspace: ctx.work_dir,
            };
            if let Some(outcome) = hooks.server_step(&request) {
                return match outcome {
                    Ok(()) => StepResult::new(name, BuildStatus::Success, Some(0)),
                    Err(message) => fail(message),
                };
            }
        }
        match step.step_type.as_str() {
            "CheckoutStep" => self.checkout(job, step, ctx),
            "CommandStep" => match self.command(job, step, ctx) {
                Ok(result) => result,
                Err(error) => fail(format!("cannot run step: {error}")),
            },
            "PublishArtifactStep" => self.publish_artifacts(job, step, ctx),
            other => {
                let message = format!("step type {other} is not supported by this server");
                if step.optional {
                    emit(
                        &ctx.log,
                        &job.name,
                        &name,
                        true,
                        format!("{message} (optional, skipped)"),
                    );
                    StepResult::new(name, BuildStatus::Skipped, None)
                } else {
                    fail(message)
                }
            }
        }
    }

    fn finish(&self, name: String, end: ProcessEnd, ctx: &JobContext<'_>, job: &str) -> StepResult {
        match end {
            ProcessEnd::Exited(Some(0)) => StepResult::new(name, BuildStatus::Success, Some(0)),
            ProcessEnd::Exited(code) => StepResult::new(name, BuildStatus::Failed, code),
            ProcessEnd::Stopped(StopReason::Cancelled) => {
                emit(&ctx.log, job, &name, true, "step cancelled".to_owned());
                StepResult::new(name, BuildStatus::Cancelled, None)
            }
            ProcessEnd::Stopped(StopReason::TimedOut) | ProcessEnd::DeadlineHit => {
                emit(
                    &ctx.log,
                    job,
                    &name,
                    true,
                    "step stopped: time limit exceeded".to_owned(),
                );
                StepResult::new(name, BuildStatus::Failed, None)
            }
        }
    }

    // -- CommandStep ----------------------------------------------------------

    fn command(
        &self,
        job: &JobSpec,
        step: &Step,
        ctx: &JobContext<'_>,
    ) -> Result<StepResult, String> {
        let name = step.display_name().to_owned();
        let raw_commands = step.commands().ok_or("step has no commands")?;
        let commands = ctx
            .variables
            .interpolate(raw_commands)
            .map_err(|e| e.to_string())?;

        let interpreter = step.interpreter.as_ref();
        if let Some(i) = interpreter
            && !matches!(i.interpreter_type.as_str(), "PosixInterpreter" | "")
        {
            return Err(format!(
                "{} is not available on this server (only POSIX shells)",
                i.interpreter_type
            ));
        }
        let shell = interpreter
            .map(|i| i.shell.clone())
            .unwrap_or_else(|| "sh".to_owned());
        if !shell
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
        {
            return Err(format!("invalid shell `{shell}`"));
        }
        // Vanilla runs posix commands with `set -e`: the first failing command fails the step.
        let script = format!("set -e\n{commands}\n");
        emit(
            &ctx.log,
            &job.name,
            &name,
            false,
            format!("$ {raw_commands}"),
        );

        let mut env: Vec<(String, String)> = ctx.variables.environment();
        for (key, value) in &job.env {
            let value = ctx
                .variables
                .interpolate(value)
                .map_err(|e| e.to_string())?;
            env.push((key.clone(), value));
        }
        env.push(("ONEDEV_JOB_NAME".into(), job.name.clone()));
        env.push(("ONEDEV_STEP_NAME".into(), name.clone()));
        env.push(("CI".into(), "true".into()));

        let end = match ctx.config.mode {
            ExecutorMode::Host => {
                if step.run_in_container == Some(true) {
                    emit(
                        &ctx.log,
                        &job.name,
                        &name,
                        true,
                        "note: the host executor ignores runInContainer".into(),
                    );
                }
                run_process(
                    ProcessSpec {
                        program: shell.into(),
                        args: vec!["-c".into(), script.into()],
                        envs: env,
                        cwd: ctx.work_dir.to_path_buf(),
                        container: None,
                        clean_env: false,
                    },
                    ctx,
                    &job.name,
                    &name,
                    ctx.deadline,
                )
                .map_err(|e| format!("failed to start {}: {e}", "shell"))?
            }
            ExecutorMode::Docker => {
                if step.run_in_container == Some(false) {
                    return Err("this step runs on the host (runInContainer: false), which the docker executor does not allow".into());
                }
                let image = match step.image.as_deref() {
                    Some(image) => Some(
                        ctx.variables
                            .interpolate(image)
                            .map_err(|e| e.to_string())?,
                    ),
                    None => ctx.config.default_image.clone(),
                }
                .ok_or("the step names no image and no default image is configured")?;
                self.run_in_container(job, step, &name, &image, &shell, &script, env, ctx)?
            }
        };
        Ok(self.finish(name, end, ctx, &job.name))
    }

    #[allow(clippy::too_many_arguments)]
    fn run_in_container(
        &self,
        job: &JobSpec,
        step: &Step,
        name: &str,
        image: &str,
        shell: &str,
        script: &str,
        env: Vec<(String, String)>,
        ctx: &JobContext<'_>,
    ) -> Result<ProcessEnd, String> {
        if image.starts_with('-') {
            return Err(format!("invalid image `{image}`"));
        }
        let container = format!("onedev-{}", uuid::Uuid::new_v4().simple());
        let work_dir = ctx
            .work_dir
            .canonicalize()
            .map_err(|e| format!("workspace not accessible: {e}"))?;

        let (uid, gid) = current_ids();
        let run_as = step
            .run_as
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty());
        let (user, as_root) = match run_as {
            Some(spec) => {
                let valid = spec.split(':').all(|part| {
                    !part.is_empty()
                        && part
                            .chars()
                            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
                });
                if !valid || spec.split(':').count() > 2 {
                    return Err(format!("invalid runAs `{spec}`"));
                }
                (
                    spec.to_owned(),
                    spec.split(':').next() == Some("0") || spec.split(':').next() == Some("root"),
                )
            }
            None => (format!("{uid}:{gid}"), false),
        };

        let mut args: Vec<OsString> = vec![
            "run".into(),
            "--rm".into(),
            "--init".into(),
            "--name".into(),
            container.clone().into(),
            "--user".into(),
            user.into(),
            "--workdir".into(),
            CONTAINER_WORKSPACE.into(),
            "--volume".into(),
            format!("{}:{CONTAINER_WORKSPACE}", work_dir.display()).into(),
            "--security-opt".into(),
            "no-new-privileges".into(),
            "--env".into(),
            "HOME=/tmp".into(),
        ];
        if let Some(memory) = &ctx.config.container_memory {
            args.extend(["--memory".into(), memory.into()]);
        }
        if let Some(cpus) = &ctx.config.container_cpus {
            args.extend(["--cpus".into(), cpus.into()]);
        }
        if let Some(pids) = ctx.config.container_pids_limit {
            args.extend(["--pids-limit".into(), pids.to_string().into()]);
        }
        if let Some(network) = ctx.network {
            args.extend(["--network".into(), network.into()]);
        }
        for (key, value) in &env {
            args.push("--env".into());
            args.push(format!("{key}={value}").into());
        }

        // Root steps would leave root-owned files in the workspace: hand them back afterwards.
        let wrapped = if as_root {
            args.extend([
                "--env".into(),
                format!("ONEDEV_STEP_SCRIPT={script}").into(),
            ]);
            format!(
                "{shell} -c \"$ONEDEV_STEP_SCRIPT\"\nstatus=$?\nchown -R {uid}:{gid} {CONTAINER_WORKSPACE} 2>/dev/null || true\nexit $status\n"
            )
        } else {
            script.to_owned()
        };
        args.push("--".into());
        args.push(image.into());
        args.push(if as_root { "sh".into() } else { shell.into() });
        args.push("-c".into());
        args.push(wrapped.into());

        run_process(
            ProcessSpec {
                program: ctx.config.docker_binary.clone().into_os_string(),
                args,
                envs: Vec::new(),
                cwd: work_dir,
                container: Some(container),
                clean_env: false,
            },
            ctx,
            &job.name,
            name,
            ctx.deadline,
        )
        .map_err(|e| {
            format!(
                "failed to start docker ({}): {e}",
                ctx.config.docker_binary.display()
            )
        })
    }

    // -- CheckoutStep -----------------------------------------------------------

    fn checkout(&self, job: &JobSpec, step: &Step, ctx: &JobContext<'_>) -> StepResult {
        let name = step.display_name().to_owned();
        let say = |text: String| emit(&ctx.log, &job.name, &name, false, text);
        let err = |text: String| emit(&ctx.log, &job.name, &name, true, text);

        let Some(source) = ctx.checkout else {
            say("checkout (workspace was prepared by the caller)".to_owned());
            return StepResult::new(name, BuildStatus::Success, Some(0));
        };

        let relative = match ctx
            .variables
            .interpolate(step.checkout_path.as_deref().unwrap_or("."))
        {
            Ok(path) => path,
            Err(error) => {
                err(error.to_string());
                return StepResult::new(name, BuildStatus::Failed, None);
            }
        };
        let dest = match safe_join(ctx.work_dir, &relative) {
            Some(path) => path,
            None => {
                err(format!("invalid checkout path `{relative}`"));
                return StepResult::new(name, BuildStatus::Failed, None);
            }
        };
        if let Err(error) = std::fs::create_dir_all(&dest) {
            err(format!("cannot create {}: {error}", dest.display()));
            return StepResult::new(name, BuildStatus::Failed, None);
        }
        // Dependency artifacts may already sit in the workspace (vanilla copies
        // them before the first step), so a non-empty directory is fine; a
        // second repository in the same place is not.
        if dest.join(".git").exists() {
            err(format!(
                "checkout path `{relative}` already contains a git repository"
            ));
            return StepResult::new(name, BuildStatus::Failed, None);
        }

        let git = |args: Vec<OsString>, cwd: &Path| -> Result<ProcessEnd, std::io::Error> {
            run_process(
                ProcessSpec {
                    program: ctx.config.git_binary.clone().into_os_string(),
                    args,
                    envs: vec![
                        ("GIT_TERMINAL_PROMPT".into(), "0".into()),
                        ("GIT_CONFIG_NOSYSTEM".into(), "1".into()),
                        ("GIT_CONFIG_GLOBAL".into(), "/dev/null".into()),
                        ("HOME".into(), "/nonexistent".into()),
                    ],
                    cwd: cwd.to_path_buf(),
                    container: None,
                    clean_env: true,
                },
                ctx,
                &job.name,
                &name,
                ctx.deadline,
            )
        };
        let ok = |end: Result<ProcessEnd, std::io::Error>| {
            matches!(end, Ok(ProcessEnd::Exited(Some(0))))
        };
        let url = format!("file://{}", source.repo_path.display());

        say(format!(
            "checking out {} ...",
            &source.commit[..source.commit.len().min(12)]
        ));
        // `init` + `fetch` + forced `checkout` (instead of `clone`) so that a
        // workspace that already holds dependency artifacts works as well.
        let mut fetch: Vec<OsString> = vec![
            "-c".into(),
            "uploadpack.allowAnySHA1InWant=true".into(),
            "fetch".into(),
            "--quiet".into(),
            "--no-tags".into(),
        ];
        match step.clone_depth.filter(|d| *d > 0) {
            Some(depth) => {
                fetch.push("--depth".into());
                fetch.push(depth.to_string().into());
                fetch.push(url.into());
            }
            None => {
                fetch.push(url.into());
                fetch.push("+refs/heads/*:refs/remotes/origin/*".into());
                fetch.push("+refs/tags/*:refs/tags/*".into());
            }
        }
        // Builds of commits that are not on a branch or tag (pull requests) need the commit itself.
        fetch.push(source.commit.clone().into());
        let cloned = ok(git(
            vec![
                "init".into(),
                "--quiet".into(),
                dest.clone().into_os_string(),
            ],
            ctx.work_dir,
        )) && ok(git(fetch, &dest))
            && ok(git(
                vec![
                    "checkout".into(),
                    "--quiet".into(),
                    "--force".into(),
                    "--detach".into(),
                    source.commit.clone().into(),
                ],
                &dest,
            ));
        if !cloned {
            if ctx.cancel.is_stopped() {
                return StepResult::new(name, BuildStatus::Cancelled, None);
            }
            err("git checkout failed".to_owned());
            return StepResult::new(name, BuildStatus::Failed, Some(1));
        }
        // No remote is configured (fetched from an anonymous URL), so scripts
        // cannot push around the server's permission checks.

        if step.with_submodules.unwrap_or(false) {
            say("updating submodules ...".to_owned());
            if !ok(git(
                vec![
                    "-c".into(),
                    "protocol.file.allow=never".into(),
                    "submodule".into(),
                    "update".into(),
                    "--init".into(),
                    "--recursive".into(),
                    "--quiet".into(),
                ],
                &dest,
            )) {
                err("submodule update failed".to_owned());
                return StepResult::new(name, BuildStatus::Failed, Some(1));
            }
        }
        if step.with_lfs.unwrap_or(false) {
            match smudge_lfs(
                &dest,
                source.lfs_objects_dir.as_deref(),
                &ctx.config.git_binary,
            ) {
                Ok(count) => say(format!("restored {count} LFS file(s)")),
                Err(error) => {
                    err(format!("LFS checkout failed: {error}"));
                    return StepResult::new(name, BuildStatus::Failed, Some(1));
                }
            }
        }
        StepResult::new(name, BuildStatus::Success, Some(0))
    }

    // -- PublishArtifactStep -------------------------------------------------------

    fn publish_artifacts(&self, job: &JobSpec, step: &Step, ctx: &JobContext<'_>) -> StepResult {
        let name = step.display_name().to_owned();
        let Some(artifact_dir) = ctx.artifact_dir else {
            emit(
                &ctx.log,
                &job.name,
                &name,
                false,
                "no artifact store attached, nothing published".into(),
            );
            return StepResult::new(name, BuildStatus::Success, Some(0));
        };
        let (patterns, source) = match (
            ctx.variables
                .interpolate(step.artifacts.as_deref().unwrap_or("**")),
            ctx.variables
                .interpolate(step.source_path.as_deref().unwrap_or(".")),
        ) {
            (Ok(patterns), Ok(source)) => (patterns, source),
            (Err(error), _) | (_, Err(error)) => {
                emit(&ctx.log, &job.name, &name, true, error.to_string());
                return StepResult::new(name, BuildStatus::Failed, None);
            }
        };
        match collect_artifacts(
            ctx.work_dir,
            &source,
            &patterns,
            artifact_dir,
            ctx.config.max_artifact_bytes,
        ) {
            Ok(files) => {
                emit(
                    &ctx.log,
                    &job.name,
                    &name,
                    false,
                    format!("published {} artifact file(s)", files.len()),
                );
                let mut result = StepResult::new(name, BuildStatus::Success, Some(0));
                result.artifacts = files;
                result
            }
            Err(error) => {
                emit(
                    &ctx.log,
                    &job.name,
                    &name,
                    true,
                    format!("cannot publish artifacts: {error}"),
                );
                StepResult::new(name, BuildStatus::Failed, None)
            }
        }
    }
}

#[cfg(unix)]
fn current_ids() -> (u32, u32) {
    // SAFETY: getuid/getgid have no preconditions and cannot fail.
    unsafe { (libc::getuid(), libc::getgid()) }
}

#[cfg(not(unix))]
fn current_ids() -> (u32, u32) {
    (1000, 1000)
}

/// `base/relative`, rejecting absolute paths and `..`.
pub fn safe_join(base: &Path, relative: &str) -> Option<PathBuf> {
    let relative = Path::new(relative);
    if relative.is_absolute() {
        return None;
    }
    let mut joined = base.to_path_buf();
    for component in relative.components() {
        match component {
            Component::Normal(part) => joined.push(part),
            Component::CurDir => {}
            _ => return None,
        }
    }
    Some(joined)
}

/// Replace LFS pointer files of a checkout with the stored objects.
fn smudge_lfs(repo: &Path, objects_dir: Option<&Path>, git: &Path) -> Result<usize, String> {
    let output = Command::new(git)
        .current_dir(repo)
        .args(["ls-files", "-z"])
        .env_clear()
        .env("PATH", std::env::var_os("PATH").unwrap_or_default())
        .output()
        .map_err(|e| format!("git ls-files: {e}"))?;
    if !output.status.success() {
        return Err("git ls-files failed".into());
    }
    let mut restored = 0;
    for path in output.stdout.split(|b| *b == 0).filter(|p| !p.is_empty()) {
        let Ok(relative) = std::str::from_utf8(path) else {
            continue;
        };
        let file = repo.join(relative);
        let Ok(meta) = std::fs::symlink_metadata(&file) else {
            continue;
        };
        if !meta.is_file() || meta.len() > 1024 {
            continue;
        }
        let Ok(content) = std::fs::read(&file) else {
            continue;
        };
        let Some(pointer) = onedev_git::parse_lfs_pointer(&content) else {
            continue;
        };
        let objects_dir =
            objects_dir.ok_or("repository uses LFS but the server has no LFS store")?;
        let object = objects_dir
            .join(&pointer.oid[0..2])
            .join(&pointer.oid[2..4])
            .join(&pointer.oid);
        let size = std::fs::metadata(&object)
            .map_err(|_| {
                format!(
                    "LFS object {} ({relative}) is missing on the server",
                    pointer.oid
                )
            })?
            .len();
        if size != pointer.size {
            return Err(format!(
                "LFS object {} has {size} bytes, pointer says {}",
                pointer.oid, pointer.size
            ));
        }
        std::fs::copy(&object, &file).map_err(|e| format!("cannot restore {relative}: {e}"))?;
        restored += 1;
    }
    Ok(restored)
}

/// Copy files below `work_dir/source` that match the space separated `patterns`
/// (`-pattern` excludes) into `target`. Symlinks and anything outside the
/// workspace are ignored. Returns the published paths relative to `source`.
pub fn collect_artifacts(
    work_dir: &Path,
    source: &str,
    patterns: &str,
    target: &Path,
    max_bytes: u64,
) -> Result<Vec<String>, String> {
    let source_dir =
        safe_join(work_dir, source).ok_or_else(|| format!("invalid source path `{source}`"))?;
    if !source_dir.is_dir() {
        return Err(format!("source path `{source}` does not exist"));
    }
    let real_work = work_dir.canonicalize().map_err(|e| e.to_string())?;
    let real_source = source_dir.canonicalize().map_err(|e| e.to_string())?;
    if !real_source.starts_with(&real_work) {
        return Err(format!("source path `{source}` leaves the workspace"));
    }

    let options = glob::MatchOptions {
        case_sensitive: true,
        require_literal_separator: true,
        require_literal_leading_dot: false,
    };
    let mut includes = Vec::new();
    let mut excludes = Vec::new();
    for raw in patterns.split_whitespace() {
        let (list, text) = match raw.strip_prefix('-') {
            Some(rest) => (&mut excludes, rest),
            None => (&mut includes, raw),
        };
        // `dir/` and `dir` mean "everything below dir"
        let text = text.trim_start_matches("./");
        let pattern = glob::Pattern::new(text).map_err(|e| format!("bad pattern `{raw}`: {e}"))?;
        list.push(pattern);
    }
    if includes.is_empty() {
        return Ok(Vec::new());
    }

    let mut published = Vec::new();
    let mut total: u64 = 0;
    for entry in walkdir::WalkDir::new(&real_source)
        .follow_links(false)
        .sort_by_file_name()
    {
        let entry = entry.map_err(|e| e.to_string())?;
        if !entry.file_type().is_file() {
            continue; // directories, symlinks, sockets
        }
        let relative = entry
            .path()
            .strip_prefix(&real_source)
            .map_err(|e| e.to_string())?
            .to_path_buf();
        let matches = |patterns: &[glob::Pattern]| {
            patterns
                .iter()
                .any(|p| p.matches_path_with(&relative, options))
        };
        if !matches(&includes) || matches(&excludes) {
            continue;
        }
        total = total.saturating_add(entry.metadata().map_err(|e| e.to_string())?.len());
        if total > max_bytes {
            return Err(format!("artifacts exceed the limit of {max_bytes} bytes"));
        }
        let destination = target.join(&relative);
        if let Some(parent) = destination.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        std::fs::copy(entry.path(), &destination)
            .map_err(|e| format!("copy {}: {e}", relative.display()))?;
        published.push(relative.to_string_lossy().into_owned());
    }
    Ok(published)
}

/// Test helper: a bare repository with one commit containing `files`.
/// Returns (keep-alive temp dir, repository path, commit id).
#[cfg(test)]
pub(crate) fn test_repo(files: &[(&str, &str)]) -> (tempfile::TempDir, PathBuf, String) {
    fn git(dir: &Path, args: &[&str]) -> String {
        let output = Command::new("git")
            .current_dir(dir)
            .args([
                "-c",
                "user.name=T",
                "-c",
                "user.email=t@example.com",
                "-c",
                "commit.gpgsign=false",
            ])
            .args(args)
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "git {args:?}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8_lossy(&output.stdout).trim().to_owned()
    }
    let tmp = tempfile::tempdir().unwrap();
    let work = tmp.path().join("work");
    std::fs::create_dir_all(&work).unwrap();
    git(&work, &["init", "-q", "-b", "main"]);
    for (path, content) in files {
        let target = work.join(path);
        std::fs::create_dir_all(target.parent().unwrap()).unwrap();
        std::fs::write(target, content).unwrap();
    }
    git(&work, &["add", "-A"]);
    git(&work, &["commit", "-q", "-m", "one"]);
    let commit = git(&work, &["rev-parse", "HEAD"]);
    let bare = tmp.path().join("repo.git");
    git(
        tmp.path(),
        &[
            "clone",
            "-q",
            "--bare",
            work.to_str().unwrap(),
            bare.to_str().unwrap(),
        ],
    );
    (tmp, bare, commit)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{Interpreter, Step};
    use std::collections::HashMap;
    use std::sync::Mutex;

    fn sink(tx: &flume::Sender<LogLine>) -> LogSink {
        LogSink::new(Some(tx.clone()), SecretMasker::default())
    }

    fn command_step(name: &str, commands: &str) -> Step {
        Step {
            step_type: "CommandStep".into(),
            name: Some(name.into()),
            condition: ExecuteCondition::Successful,
            optional: false,
            run_in_container: None,
            image: None,
            interpreter: Some(Interpreter {
                interpreter_type: "PosixInterpreter".into(),
                shell: "sh".into(),
                powershell: None,
                commands: commands.into(),
            }),
            run_as: None,
            use_tty: None,
            run: None,
            with_lfs: None,
            with_submodules: None,
            clone_depth: None,
            checkout_path: None,
            source_path: None,
            artifacts: None,
            template_name: None,
            extra: Default::default(),
        }
    }

    fn job(steps: Vec<Step>) -> JobSpec {
        JobSpec {
            name: "j".into(),
            execute_in_sandbox: false,
            job_executor: None,
            sandbox_template: None,
            steps,
            param_specs: vec![],
            job_dependencies: vec![],
            project_dependencies: vec![],
            required_services: vec![],
            triggers: vec![],
            timeout: 3600,
            post_build_actions: vec![],
            sequential_group: None,
            retry_condition: "never".into(),
            max_attempts: 1,
            retry_delay: 0,
            env: HashMap::new(),
        }
    }

    struct Fixture {
        dir: tempfile::TempDir,
        cancel: CancelToken,
        config: ExecutorConfig,
        variables: Variables,
        masker: SecretMasker,
        log_tx: flume::Sender<LogLine>,
        log_rx: flume::Receiver<LogLine>,
    }

    impl Fixture {
        fn new() -> Self {
            let (log_tx, log_rx) = flume::unbounded();
            Fixture {
                dir: tempfile::tempdir().unwrap(),
                cancel: CancelToken::new(),
                config: ExecutorConfig::default(),
                variables: Variables::new(),
                masker: SecretMasker::default(),
                log_tx,
                log_rx,
            }
        }

        fn run(&self, job: &JobSpec, deadline: Option<Instant>) -> JobOutcome {
            let ctx = JobContext {
                work_dir: self.dir.path(),
                artifact_dir: None,
                log: LogSink::new(Some(self.log_tx.clone()), self.masker.clone()),
                cancel: &self.cancel,
                deadline,
                checkout: None,
                variables: &self.variables,
                config: &self.config,
                network: None,
                hooks: None,
            };
            JobExecutor.execute(job, &job.steps, &ctx).unwrap()
        }

        fn logs(&self) -> Vec<String> {
            self.log_rx.try_iter().map(|l| l.line).collect()
        }
    }

    #[test]
    fn commands_run_with_set_e_variables_and_streamed_output() {
        let mut fx = Fixture::new();
        fx.variables = Variables::new().with_job("build_number", "7");
        let j = job(vec![
            command_step(
                "print",
                "echo out-$ONEDEV_BUILD_NUMBER; echo err >&2; echo @build_number@",
            ),
            command_step("stops-at-first-failure", "false\necho should-not-run"),
        ]);
        let outcome = fx.run(&j, None);
        assert_eq!(outcome.steps[0].status, BuildStatus::Success);
        assert_eq!(outcome.steps[1].status, BuildStatus::Failed);
        assert_eq!(outcome.steps[1].exit_code, Some(1));
        assert!(outcome.failed);
        let logs = fx.logs();
        assert!(logs.contains(&"out-7".to_owned()), "{logs:?}");
        assert!(logs.contains(&"err".to_owned()));
        assert!(!logs.iter().any(|l| l == "should-not-run"));
        assert!(
            logs.iter().filter(|l| l.as_str() == "7").count() >= 1,
            "@build_number@ interpolated: {logs:?}"
        );
    }

    #[test]
    fn conditions_and_optional_steps() {
        let fx = Fixture::new();
        let mut cleanup = command_step("cleanup", "echo cleanup");
        cleanup.condition = ExecuteCondition::Always;
        let mut on_failure = command_step("on-failure", "echo notify");
        on_failure.condition = ExecuteCondition::Failed;
        let mut optional = command_step("flaky", "exit 3");
        optional.optional = true;
        let j = job(vec![
            optional,
            command_step("after-optional", "echo still-running"),
            on_failure,
            cleanup,
        ]);
        let outcome = fx.run(&j, None);
        let statuses: Vec<_> = outcome.steps.iter().map(|s| s.status).collect();
        assert_eq!(
            statuses,
            vec![
                BuildStatus::Failed,
                BuildStatus::Success,
                BuildStatus::Skipped,
                BuildStatus::Success
            ]
        );
        assert!(!outcome.failed, "optional failure does not fail the job");
    }

    #[test]
    fn unsupported_steps_fail_unless_optional() {
        let fx = Fixture::new();
        let mut unsupported = command_step("weird", "");
        unsupported.step_type = "BuildImageStep".into();
        let outcome = fx.run(&job(vec![unsupported.clone()]), None);
        assert_eq!(outcome.steps[0].status, BuildStatus::Failed);
        assert!(outcome.failed);
        assert!(
            fx.logs()
                .iter()
                .any(|l| l.contains("BuildImageStep") && l.contains("not supported"))
        );

        unsupported.optional = true;
        let outcome = fx.run(&job(vec![unsupported]), None);
        assert_eq!(outcome.steps[0].status, BuildStatus::Skipped);
        assert!(!outcome.failed);
    }

    #[test]
    fn deadline_kills_the_whole_process_tree() {
        let fx = Fixture::new();
        let marker = fx.dir.path().join("grandchild.pid");
        // The shell starts a background grandchild, records its pid and then sleeps.
        let script = format!("sleep 300 &\necho $! > {}\nsleep 300", marker.display());
        let j = job(vec![command_step("hang", &script)]);
        let started = Instant::now();
        let outcome = fx.run(&j, Some(Instant::now() + Duration::from_millis(600)));
        assert!(
            started.elapsed() < Duration::from_secs(20),
            "deadline must stop the step quickly"
        );
        assert_eq!(outcome.steps[0].status, BuildStatus::Failed);
        assert!(outcome.timed_out);
        let pid: i32 = std::fs::read_to_string(&marker)
            .unwrap()
            .trim()
            .parse()
            .unwrap();
        std::thread::sleep(Duration::from_millis(300));
        // SAFETY: signal 0 only probes for existence.
        let alive = unsafe { libc::kill(pid, 0) } == 0;
        assert!(
            !alive,
            "grandchild {pid} must have been killed with its group"
        );
    }

    #[test]
    fn cancellation_stops_a_running_step_and_skips_the_rest() {
        let fx = Fixture::new();
        let j = job(vec![
            command_step("long", "sleep 300"),
            command_step("never", "echo never"),
        ]);
        let cancel = fx.cancel.clone();
        let canceller = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(400));
            cancel.cancel();
        });
        let started = Instant::now();
        let outcome = fx.run(&j, None);
        canceller.join().unwrap();
        assert!(started.elapsed() < Duration::from_secs(20));
        assert_eq!(outcome.steps[0].status, BuildStatus::Cancelled);
        assert_eq!(outcome.steps[1].status, BuildStatus::Cancelled);
        assert!(!fx.logs().iter().any(|l| l == "never"));
    }

    #[test]
    fn background_processes_do_not_outlive_their_step_or_hang_it() {
        let fx = Fixture::new();
        let marker = fx.dir.path().join("bg.pid");
        // `sleep 300 &` keeps stdout open; the step must still finish promptly.
        let script = format!("sleep 300 &\necho $! > {}\necho done", marker.display());
        let started = Instant::now();
        let outcome = fx.run(&job(vec![command_step("bg", &script)]), None);
        assert!(
            started.elapsed() < Duration::from_secs(15),
            "step must not wait for the background process"
        );
        assert_eq!(outcome.steps[0].status, BuildStatus::Success);
        let pid: i32 = std::fs::read_to_string(&marker)
            .unwrap()
            .trim()
            .parse()
            .unwrap();
        std::thread::sleep(Duration::from_millis(200));
        // SAFETY: signal 0 only probes for existence.
        assert!(
            unsafe { libc::kill(pid, 0) } != 0,
            "background process must be reaped"
        );
    }

    #[test]
    fn very_long_lines_are_split_instead_of_buffered() {
        let fx = Fixture::new();
        let outcome = fx.run(
            &job(vec![command_step(
                "long",
                "head -c 100000 /dev/zero | tr '\\0' 'a'; echo",
            )]),
            None,
        );
        assert_eq!(outcome.steps[0].status, BuildStatus::Success);
        let logs = fx.logs();
        let long: Vec<&String> = logs.iter().filter(|l| l.starts_with('a')).collect();
        assert!(
            long.len() >= 6,
            "100 kB must arrive in several chunks, got {}",
            long.len()
        );
        assert!(long.iter().all(|l| l.len() <= MAX_LINE as usize));
        assert_eq!(long.iter().map(|l| l.len()).sum::<usize>(), 100_000);
    }

    #[test]
    fn host_mode_ignores_run_in_container_but_docker_mode_requires_an_image() {
        let mut fx = Fixture::new();
        let mut containerised = command_step("c", "echo hi");
        containerised.run_in_container = Some(true);
        let outcome = fx.run(&job(vec![containerised.clone()]), None);
        assert_eq!(outcome.steps[0].status, BuildStatus::Success);

        fx.config.mode = ExecutorMode::Docker;
        let outcome = fx.run(&job(vec![containerised]), None);
        assert_eq!(outcome.steps[0].status, BuildStatus::Failed);
        assert!(fx.logs().iter().any(|l| l.contains("no image")));

        let mut on_host = command_step("h", "echo hi");
        on_host.run_in_container = Some(false);
        on_host.image = Some("alpine".into());
        let outcome = fx.run(&job(vec![on_host]), None);
        assert_eq!(outcome.steps[0].status, BuildStatus::Failed);
        assert!(fx.logs().iter().any(|l| l.contains("does not allow")));
    }

    #[test]
    fn safe_join_rejects_escapes() {
        let base = Path::new("/work");
        assert_eq!(safe_join(base, "a/b"), Some(PathBuf::from("/work/a/b")));
        assert_eq!(safe_join(base, "."), Some(PathBuf::from("/work")));
        assert_eq!(safe_join(base, "./x"), Some(PathBuf::from("/work/x")));
        assert!(safe_join(base, "../x").is_none());
        assert!(safe_join(base, "a/../../x").is_none());
        assert!(safe_join(base, "/etc").is_none());
    }

    #[test]
    fn artifact_collection_honours_globs_excludes_and_ignores_symlinks() {
        let dir = tempfile::tempdir().unwrap();
        let work = dir.path().join("w");
        let out = dir.path().join("out");
        std::fs::create_dir_all(work.join("target/release")).unwrap();
        std::fs::write(work.join("target/release/app"), "bin").unwrap();
        std::fs::write(work.join("target/release/app.d"), "dep").unwrap();
        std::fs::write(work.join("target/notes.txt"), "n").unwrap();
        std::fs::write(dir.path().join("secret.txt"), "secret").unwrap();
        std::os::unix::fs::symlink(dir.path().join("secret.txt"), work.join("target/link.txt"))
            .unwrap();

        let mut files =
            collect_artifacts(&work, ".", "target/**/* -**/*.d", &out, 1 << 20).unwrap();
        files.sort();
        assert_eq!(files, vec!["target/notes.txt", "target/release/app"]);
        assert!(out.join("target/release/app").is_file());
        assert!(
            !out.join("target/link.txt").exists(),
            "symlinks must not be followed out of the workspace"
        );

        let from_subdir =
            collect_artifacts(&work, "target", "release/*", &out.join("sub"), 1 << 20).unwrap();
        assert_eq!(from_subdir.len(), 2);

        assert!(collect_artifacts(&work, "../", "**", &out, 1 << 20).is_err());
        assert!(
            collect_artifacts(&work, ".", "target/**/*", &out.join("tiny"), 2).is_err(),
            "size limit"
        );
        assert!(collect_artifacts(&work, "missing", "**", &out, 1 << 20).is_err());
    }

    fn git(dir: &Path, args: &[&str]) -> String {
        let output = Command::new("git")
            .current_dir(dir)
            .args([
                "-c",
                "user.name=T",
                "-c",
                "user.email=t@example.com",
                "-c",
                "commit.gpgsign=false",
            ])
            .args(args)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "git {args:?}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8_lossy(&output.stdout).trim().to_owned()
    }

    fn bare_repo_with(files: &[(&str, &str)]) -> (tempfile::TempDir, PathBuf, String) {
        super::test_repo(files)
    }

    #[test]
    fn checkout_materialises_the_commit_without_a_push_back_remote() {
        let (_keep, bare, commit) =
            bare_repo_with(&[("src/main.rs", "fn main() {}"), ("README.md", "hi")]);
        let fx = Fixture::new();
        let source = CheckoutSource {
            repo_path: bare,
            commit: commit.clone(),
            lfs_objects_dir: None,
        };
        let mut step = command_step("checkout", "");
        step.step_type = "CheckoutStep".into();
        step.interpreter = None;
        let j = job(vec![step]);
        let ctx = JobContext {
            work_dir: fx.dir.path(),
            artifact_dir: None,
            log: sink(&fx.log_tx),
            cancel: &fx.cancel,
            deadline: None,
            checkout: Some(&source),
            variables: &fx.variables,
            config: &fx.config,
            network: None,
            hooks: None,
        };
        let outcome = JobExecutor.execute(&j, &j.steps, &ctx).unwrap();
        assert_eq!(
            outcome.steps[0].status,
            BuildStatus::Success,
            "{:?}",
            fx.logs()
        );
        assert_eq!(
            std::fs::read_to_string(fx.dir.path().join("src/main.rs")).unwrap(),
            "fn main() {}"
        );
        assert_eq!(git(fx.dir.path(), &["rev-parse", "HEAD"]), commit);
        assert_eq!(
            git(fx.dir.path(), &["remote"]),
            "",
            "origin must be removed"
        );
    }

    #[test]
    fn shallow_checkout_into_subdirectory_and_path_validation() {
        let (_keep, bare, commit) = bare_repo_with(&[("a.txt", "a")]);
        let fx = Fixture::new();
        let source = CheckoutSource {
            repo_path: bare,
            commit,
            lfs_objects_dir: None,
        };
        let run = |checkout_path: &str, depth: Option<u32>| {
            let mut step = command_step("checkout", "");
            step.step_type = "CheckoutStep".into();
            step.interpreter = None;
            step.checkout_path = Some(checkout_path.into());
            step.clone_depth = depth;
            let j = job(vec![step]);
            let ctx = JobContext {
                work_dir: fx.dir.path(),
                artifact_dir: None,
                log: sink(&fx.log_tx),
                cancel: &fx.cancel,
                deadline: None,
                checkout: Some(&source),
                variables: &fx.variables,
                config: &fx.config,
                network: None,
                hooks: None,
            };
            JobExecutor.execute(&j, &j.steps, &ctx).unwrap().steps[0].status
        };
        assert_eq!(
            run("code", Some(1)),
            BuildStatus::Success,
            "{:?}",
            fx.logs()
        );
        assert!(fx.dir.path().join("code/a.txt").is_file());
        assert_eq!(
            git(
                &fx.dir.path().join("code"),
                &["rev-list", "--count", "HEAD"]
            ),
            "1"
        );
        assert_eq!(run("../escape", None), BuildStatus::Failed);
        assert_eq!(run("/abs", None), BuildStatus::Failed);
        assert_eq!(
            run("code", None),
            BuildStatus::Failed,
            "non-empty checkout path is refused"
        );
    }

    #[test]
    fn lfs_pointers_are_restored_from_the_store() {
        let content = b"real large content";
        let oid = onedev_git::sha256_hex(content);
        let pointer = onedev_git::format_lfs_pointer(&oid, content.len() as u64);
        let (_keep, bare, commit) =
            bare_repo_with(&[("data.bin", &pointer), ("plain.txt", "plain")]);
        let store = tempfile::tempdir().unwrap();
        let object = store.path().join(&oid[0..2]).join(&oid[2..4]).join(&oid);
        std::fs::create_dir_all(object.parent().unwrap()).unwrap();
        std::fs::write(&object, content).unwrap();

        let fx = Fixture::new();
        let source = CheckoutSource {
            repo_path: bare.clone(),
            commit: commit.clone(),
            lfs_objects_dir: Some(store.path().to_path_buf()),
        };
        let mut step = command_step("checkout", "");
        step.step_type = "CheckoutStep".into();
        step.interpreter = None;
        step.with_lfs = Some(true);
        let j = job(vec![step.clone()]);
        let ctx = JobContext {
            work_dir: fx.dir.path(),
            artifact_dir: None,
            log: sink(&fx.log_tx),
            cancel: &fx.cancel,
            deadline: None,
            checkout: Some(&source),
            variables: &fx.variables,
            config: &fx.config,
            network: None,
            hooks: None,
        };
        let outcome = JobExecutor.execute(&j, &j.steps, &ctx).unwrap();
        assert_eq!(
            outcome.steps[0].status,
            BuildStatus::Success,
            "{:?}",
            fx.logs()
        );
        assert_eq!(
            std::fs::read(fx.dir.path().join("data.bin")).unwrap(),
            content
        );
        assert_eq!(
            std::fs::read_to_string(fx.dir.path().join("plain.txt")).unwrap(),
            "plain"
        );

        // missing object => the step fails loudly
        let fx2 = Fixture::new();
        let empty_store = tempfile::tempdir().unwrap();
        let source2 = CheckoutSource {
            repo_path: bare,
            commit,
            lfs_objects_dir: Some(empty_store.path().to_path_buf()),
        };
        let ctx2 = JobContext {
            work_dir: fx2.dir.path(),
            artifact_dir: None,
            log: sink(&fx2.log_tx),
            cancel: &fx2.cancel,
            deadline: None,
            checkout: Some(&source2),
            variables: &fx2.variables,
            config: &fx2.config,
            network: None,
            hooks: None,
        };
        let outcome = JobExecutor.execute(&j, &j.steps, &ctx2).unwrap();
        assert_eq!(outcome.steps[0].status, BuildStatus::Failed);
        assert!(
            fx2.logs()
                .iter()
                .any(|l| l.contains("missing on the server"))
        );
    }

    #[test]
    fn secrets_are_masked_in_logs_and_not_exported_to_the_environment() {
        let mut fx = Fixture::new();
        fx.variables = Variables::new().with_secret("token", "hunter2-secret");
        fx.masker = SecretMasker::new(fx.variables.secret_values());
        let j = job(vec![command_step(
            "use",
            "echo token=@secret:token@\necho \"again hunter2-secret\" >&2\nenv | grep -i hunter2 && exit 9 || true\ntest -z \"$ONEDEV_SECRET_TOKEN\"",
        )]);
        let outcome = fx.run(&j, None);
        assert_eq!(
            outcome.steps[0].status,
            BuildStatus::Success,
            "{:?}",
            fx.logs()
        );
        let logs = fx.logs();
        assert!(
            !logs.iter().any(|l| l.contains("hunter2-secret")),
            "secret leaked: {logs:?}"
        );
        assert!(logs.iter().any(|l| l == "token=*******"), "{logs:?}");
        assert!(logs.iter().any(|l| l == "again *******"), "{logs:?}");
    }

    #[test]
    fn an_unknown_variable_fails_the_step_with_a_clear_message() {
        let fx = Fixture::new();
        let outcome = fx.run(&job(vec![command_step("bad", "echo @nope@")]), None);
        assert_eq!(outcome.steps[0].status, BuildStatus::Failed);
        assert!(
            fx.logs()
                .iter()
                .any(|l| l.contains("Unrecognized interpolation variable: nope"))
        );
        // a literal @ is written @@
        let outcome = fx.run(&job(vec![command_step("ok", "echo a@@b")]), None);
        assert_eq!(outcome.steps[0].status, BuildStatus::Success);
        assert!(fx.logs().iter().any(|l| l == "a@b"));
    }

    #[test]
    fn hooks_observe_steps_and_run_server_side_steps() {
        #[derive(Default)]
        struct Recorder {
            finished: Mutex<Vec<(String, BuildStatus)>>,
        }
        impl StepHooks for Recorder {
            fn step_finished(&self, _job: &str, step: &StepResult) {
                self.finished
                    .lock()
                    .unwrap()
                    .push((step.name.clone(), step.status));
            }
            fn server_step(&self, request: &ServerStep<'_>) -> Option<Result<(), String>> {
                match request.step.step_type.as_str() {
                    "SetBuildVersionStep" => {
                        request.variables.set_build_version("9.9".into());
                        Some(Ok(()))
                    }
                    "CreateTagStep" => Some(Err("tag already exists".into())),
                    _ => None,
                }
            }
        }
        let fx = Fixture::new();
        let recorder = Recorder::default();
        let mut version = command_step("version", "");
        version.step_type = "SetBuildVersionStep".into();
        let mut tag = command_step("tag", "");
        tag.step_type = "CreateTagStep".into();
        let mut cleanup = command_step("cleanup", "echo cleanup");
        cleanup.condition = ExecuteCondition::Always;
        let j = job(vec![
            version,
            command_step("print", "echo v=@build_version@"),
            tag,
            cleanup,
        ]);
        let ctx = JobContext {
            work_dir: fx.dir.path(),
            artifact_dir: None,
            log: sink(&fx.log_tx),
            cancel: &fx.cancel,
            deadline: None,
            checkout: None,
            variables: &fx.variables,
            config: &fx.config,
            network: None,
            hooks: Some(&recorder),
        };
        let outcome = JobExecutor.execute(&j, &j.steps, &ctx).unwrap();
        let statuses: Vec<_> = outcome.steps.iter().map(|s| s.status).collect();
        assert_eq!(
            statuses,
            vec![
                BuildStatus::Success,
                BuildStatus::Success,
                BuildStatus::Failed,
                BuildStatus::Success
            ]
        );
        assert_eq!(
            recorder.finished.lock().unwrap().len(),
            4,
            "every step is reported"
        );
        let logs = fx.logs();
        assert!(logs.iter().any(|l| l == "v=9.9"), "{logs:?}");
        assert!(logs.iter().any(|l| l == "tag already exists"));
    }

    #[test]
    fn nothing_starts_once_the_deadline_has_passed() {
        let fx = Fixture::new();
        let mut cleanup = command_step("cleanup", "echo cleanup");
        cleanup.condition = ExecuteCondition::Always;
        let j = job(vec![command_step("a", "echo a"), cleanup]);
        let past = Instant::now() - Duration::from_secs(1);
        let outcome = fx.run(&j, Some(past));
        assert!(outcome.timed_out);
        assert!(
            outcome
                .steps
                .iter()
                .all(|s| s.status == BuildStatus::Skipped)
        );
        assert!(fx.logs().is_empty(), "no process may have been started");
    }

    /// Needs a working Docker daemon and the `alpine` image; skipped otherwise.
    fn docker_ready() -> bool {
        Command::new("docker")
            .args(["image", "inspect", "alpine:3"])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    }

    #[test]
    fn docker_steps_run_isolated_as_the_server_user_and_can_be_cancelled() {
        if !docker_ready() {
            eprintln!("skipping: docker or alpine:3 not available");
            return;
        }
        let mut fx = Fixture::new();
        fx.config.mode = ExecutorMode::Docker;
        let mut step = command_step(
            "container",
            "echo inside-$(id -u); echo data > made-in-container.txt; test -f /onedev-build/workspace/made-in-container.txt; test ! -e /etc/hostname-from-host-marker",
        );
        step.image = Some("alpine:3".into());
        let outcome = fx.run(&job(vec![step]), None);
        assert_eq!(
            outcome.steps[0].status,
            BuildStatus::Success,
            "{:?}",
            fx.logs()
        );
        let (uid, _) = current_ids();
        assert!(fx.logs().iter().any(|l| l == &format!("inside-{uid}")));
        let made = fx.dir.path().join("made-in-container.txt");
        assert_eq!(std::fs::read_to_string(&made).unwrap().trim(), "data");
        // files are owned by the server user, so cleanup never needs root
        use std::os::unix::fs::MetadataExt;
        assert_eq!(std::fs::metadata(&made).unwrap().uid(), uid);

        // cancellation stops the container
        let fx = {
            let mut fx = Fixture::new();
            fx.config.mode = ExecutorMode::Docker;
            fx
        };
        let mut long = command_step("sleeper", "sleep 300");
        long.image = Some("alpine:3".into());
        let cancel = fx.cancel.clone();
        let canceller = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_secs(2));
            cancel.cancel();
        });
        let started = Instant::now();
        let outcome = fx.run(&job(vec![long]), None);
        canceller.join().unwrap();
        assert_eq!(outcome.steps[0].status, BuildStatus::Cancelled);
        assert!(started.elapsed() < Duration::from_secs(40));
    }
}
