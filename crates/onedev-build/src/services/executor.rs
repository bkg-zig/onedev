//! Job executors.
//!
//! The MVP has one executor: `host`, which runs the job's steps with the
//! host shell in the checkout directory (the `serverhost` executor in
//! vanilla terms). Docker/Kubernetes/sandbox executors are separate
//! steps in the port plan; the spec rejects them at validation time.

use std::io::{BufRead, BufReader};
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use crate::errors::BuildResult;
use crate::models::{BuildStatus, JobSpec, LogLine, StepResult};

/// Context a job runs in.
pub struct JobContext<'a> {
    pub work_dir: &'a Path,
    pub log: Option<flume::Sender<LogLine>>,
}

fn emit(log: &Option<flume::Sender<LogLine>>, job: &str, step: &str, stderr: bool, line: String) {
    if let Some(sender) = log {
        let _ = sender.send(LogLine {
            job: job.to_owned(),
            step: step.to_owned(),
            stderr,
            line,
        });
    }
}

/// Runs the steps of one job on the host.
pub struct HostExecutor;

impl HostExecutor {
    /// Execute `job`; returns step results in order. Stops at the first
    /// failing step. Honors `timeout_seconds` (kills the step process).
    pub fn execute(&self, job: &JobSpec, spec_variables: &std::collections::HashMap<String, String>, ctx: &JobContext<'_>) -> BuildResult<Vec<StepResult>> {
        let mut results = Vec::new();

        for step in &job.steps {
            let name = step.display_name().to_owned();
            emit(&ctx.log, &job.name, &name, false, format!("$ {}", step.run));

            let mut command = Command::new("sh");
            command
                .arg("-c")
                .arg(&step.run)
                .current_dir(ctx.work_dir)
                .envs(spec_variables.iter())
                .envs(job.env.iter())
                .env("ONEDEV_JOB_NAME", &job.name)
                .env("ONEDEV_STEP_NAME", &name)
                .stdout(Stdio::piped())
                .stderr(Stdio::piped());

            let mut child = command.spawn()?;
            let deadline = job
                .timeout_seconds
                .map(|seconds| Instant::now() + Duration::from_secs(seconds));

            // Stream stdout/stderr so callers see logs while the job runs.
            let mut readers = Vec::new();
            if let Some(out) = child.stdout.take() {
                let (job_name, step_name, log) = (job.name.clone(), name.clone(), ctx.log.clone());
                readers.push(std::thread::spawn(move || {
                    for line in BufReader::new(out).lines().map_while(Result::ok) {
                        emit(&log, &job_name, &step_name, false, line);
                    }
                }));
            }
            if let Some(err) = child.stderr.take() {
                let (job_name, step_name, log) = (job.name.clone(), name.clone(), ctx.log.clone());
                readers.push(std::thread::spawn(move || {
                    for line in BufReader::new(err).lines().map_while(Result::ok) {
                        emit(&log, &job_name, &step_name, true, line);
                    }
                }));
            }

            let status = loop {
                match child.try_wait()? {
                    Some(status) => break status,
                    None => {
                        if deadline.map_or(false, |d| Instant::now() > d) {
                            let _ = child.kill();
                            let _ = child.wait();
                            for reader in readers {
                                let _ = reader.join();
                            }
                            results.push(StepResult {
                                name,
                                status: BuildStatus::Failed,
                                exit_code: None,
                            });
                            return Ok(results);
                        }
                        std::thread::sleep(Duration::from_millis(20));
                    }
                }
            };
            for reader in readers {
                let _ = reader.join();
            }

            let step_status = if status.success() {
                BuildStatus::Success
            } else {
                BuildStatus::Failed
            };
            results.push(StepResult {
                name,
                status: step_status,
                exit_code: status.code(),
            });
            if step_status == BuildStatus::Failed {
                break;
            }
        }

        Ok(results)
    }
}
