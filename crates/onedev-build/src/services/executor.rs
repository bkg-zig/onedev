//! Job executors.
//!
//! The host executor runs `CommandStep`s with the host shell in the
//! checkout directory (the `serverhost` executor in vanilla terms).
//! `CheckoutStep` is a no-op here because the runner already operates on
//! a checkout; container/image/template steps are reported as skipped
//! until their executors are ported.

use std::io::{BufRead, BufReader};
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use crate::errors::BuildResult;
use crate::models::{BuildStatus, ExecuteCondition, JobSpec, LogLine, StepResult};

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
    /// Execute `job`; returns step results in order. Honors each step's
    /// `condition` and the job `timeout` (seconds).
    pub fn execute(&self, job: &JobSpec, spec_variables: &std::collections::HashMap<String, String>, ctx: &JobContext<'_>) -> BuildResult<Vec<StepResult>> {
        let mut results = Vec::new();
        let mut failed = false;

        for step in &job.steps {
            let name = step.display_name().to_owned();
            let should_run = match step.condition {
                ExecuteCondition::Successful => !failed,
                ExecuteCondition::Always => true,
                ExecuteCondition::Failed => failed,
            };
            if !should_run {
                results.push(StepResult {
                    name,
                    status: BuildStatus::Skipped,
                    exit_code: None,
                });
                continue;
            }

            match step.step_type.as_str() {
                "CheckoutStep" => {
                    emit(&ctx.log, &job.name, &name, false, "checkout (working directory already prepared)".to_owned());
                    results.push(StepResult {
                        name,
                        status: BuildStatus::Success,
                        exit_code: Some(0),
                    });
                }
                "CommandStep" => {
                    let Some(commands) = step.commands() else {
                        results.push(StepResult {
                            name,
                            status: BuildStatus::Failed,
                            exit_code: None,
                        });
                        failed = true;
                        continue;
                    };
                    let result = self.run_commands(job, step, commands, spec_variables, ctx)?;
                    if result.status == BuildStatus::Failed && !step.optional {
                        failed = true;
                    }
                    results.push(result);
                }
                "PublishArtifactStep" => {
                    // Artifacts are collected by the runner after success.
                    results.push(StepResult {
                        name,
                        status: BuildStatus::Success,
                        exit_code: Some(0),
                    });
                }
                other => {
                    emit(
                        &ctx.log,
                        &job.name,
                        &name,
                        true,
                        format!("step type {other} is not supported by the host executor yet"),
                    );
                    results.push(StepResult {
                        name,
                        status: BuildStatus::Skipped,
                        exit_code: None,
                    });
                }
            }
        }

        Ok(results)
    }

    fn run_commands(
        &self,
        job: &JobSpec,
        step: &crate::models::Step,
        commands: &str,
        spec_variables: &std::collections::HashMap<String, String>,
        ctx: &JobContext<'_>,
    ) -> BuildResult<StepResult> {
        let name = step.display_name().to_owned();
        emit(&ctx.log, &job.name, &name, false, format!("$ {commands}"));

        let shell = step
            .interpreter
            .as_ref()
            .map(|interpreter| interpreter.shell.clone())
            .unwrap_or_else(|| "sh".to_owned());

        let mut command = Command::new(&shell);
        command
            .arg("-c")
            .arg(commands)
            .current_dir(ctx.work_dir)
            .envs(spec_variables.iter())
            .envs(job.env.iter())
            .env("ONEDEV_JOB_NAME", &job.name)
            .env("ONEDEV_STEP_NAME", &name)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        let mut child = command.spawn()?;
        let deadline = Instant::now() + Duration::from_secs(job.timeout.max(1));

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
                    if Instant::now() > deadline {
                        let _ = child.kill();
                        let _ = child.wait();
                        for reader in readers {
                            let _ = reader.join();
                        }
                        return Ok(StepResult {
                            name,
                            status: BuildStatus::Failed,
                            exit_code: None,
                        });
                    }
                    std::thread::sleep(Duration::from_millis(20));
                }
            }
        };
        for reader in readers {
            let _ = reader.join();
        }

        Ok(StepResult {
            name,
            status: if status.success() {
                BuildStatus::Success
            } else {
                BuildStatus::Failed
            },
            exit_code: status.code(),
        })
    }
}
