//! Build runner: executes a spec's jobs in dependency order.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::errors::BuildResult;
use crate::models::{BuildReport, BuildSpec, BuildStatus, JobResult, LogLine, TriggerEvent};
use crate::services::executor::{HostExecutor, JobContext};
use crate::services::parser::{execution_order, jobs_for_event};

/// Runs builds on the host executor.
pub struct BuildRunner {
    executor: HostExecutor,
}

impl Default for BuildRunner {
    fn default() -> Self {
        Self::new()
    }
}

impl BuildRunner {
    pub fn new() -> Self {
        BuildRunner { executor: HostExecutor }
    }

    /// Run the jobs selected by `event`. Jobs whose dependencies failed
    /// (or were not selected) are reported as skipped. Artifacts of
    /// successful jobs are copied into `artifact_root/<job>/`.
    pub fn run(
        &self,
        spec: &BuildSpec,
        work_dir: &Path,
        artifact_root: &Path,
        event: &TriggerEvent,
        log: Option<flume::Sender<LogLine>>,
    ) -> BuildResult<BuildReport> {
        let selected: Vec<String> = jobs_for_event(spec, event)?;
        let selected_set: std::collections::HashSet<&str> =
            selected.iter().map(String::as_str).collect();
        let order = execution_order(spec)?;
        let by_name: HashMap<&str, &crate::models::JobSpec> =
            spec.jobs.iter().map(|job| (job.name.as_str(), job)).collect();

        let mut results: Vec<JobResult> = Vec::new();
        let mut statuses: HashMap<String, BuildStatus> = HashMap::new();

        for name in &order {
            let job = by_name[name.as_str()];
            let dependencies_ok = job
                .job_dependencies
                .iter()
                .all(|dependency| {
                    !dependency.require_successful
                        || statuses.get(&dependency.job_name) == Some(&BuildStatus::Success)
                });

            if !selected_set.contains(name.as_str()) || !dependencies_ok {
                let result = JobResult {
                    name: name.clone(),
                    status: BuildStatus::Skipped,
                    steps: Vec::new(),
                    artifacts: Vec::new(),
                    attempts: 0,
                };
                statuses.insert(name.clone(), result.status);
                results.push(result);
                continue;
            }

            // Vanilla: retryCondition "never" disables retries, otherwise
            // the job is attempted up to maxAttempts times.
            let max_attempts = if job.retry_condition == "never" {
                1
            } else {
                job.max_attempts.max(1)
            };
            let no_variables = HashMap::new();
            let mut attempts = 0;
            let mut step_results = Vec::new();
            let mut status = BuildStatus::Failed;
            while attempts < max_attempts {
                attempts += 1;
                step_results = self.executor.execute(
                    job,
                    &no_variables,
                    &JobContext { work_dir, log: log.clone() },
                )?;
                status = if step_results
                    .iter()
                    .filter(|step| step.status != BuildStatus::Skipped)
                    .all(|step| step.status == BuildStatus::Success)
                {
                    BuildStatus::Success
                } else {
                    BuildStatus::Failed
                };
                if status == BuildStatus::Success {
                    break;
                }
            }

            let artifacts = if status == BuildStatus::Success {
                collect_artifacts(job, work_dir, &artifact_root.join(&job.name))?
            } else {
                Vec::new()
            };

            statuses.insert(name.clone(), status);
            results.push(JobResult {
                name: name.clone(),
                status,
                steps: step_results,
                artifacts,
                attempts,
            });
        }

        let overall = if results
            .iter()
            .filter(|job| job.status != BuildStatus::Skipped)
            .all(|job| job.status == BuildStatus::Success)
        {
            BuildStatus::Success
        } else {
            BuildStatus::Failed
        };

        Ok(BuildReport { status: overall, jobs: results })
    }
}

/// Copy artifact paths/globs of a job into `target_dir`.
fn collect_artifacts(
    job: &crate::models::JobSpec,
    work_dir: &Path,
    target_dir: &Path,
) -> BuildResult<Vec<String>> {
    let mut collected = Vec::new();
    let patterns: Vec<String> = job
        .artifact_patterns()
        .iter()
        .flat_map(|patterns| patterns.split_whitespace().map(str::to_owned).collect::<Vec<_>>())
        .collect();
    for pattern in &patterns {
        let full = work_dir.join(pattern);
        let matches: Vec<PathBuf> = if pattern.contains('*') {
            match glob::glob(full.to_string_lossy().as_ref()) {
                Ok(paths) => paths.filter_map(Result::ok).collect(),
                Err(_) => Vec::new(),
            }
        } else if full.exists() {
            vec![full]
        } else {
            Vec::new()
        };

        for source in matches {
            if !source.is_file() {
                continue;
            }
            let relative = source.strip_prefix(work_dir).unwrap_or(&source);
            let target = target_dir.join(relative);
            if let Some(parent) = target.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::copy(&source, &target)?;
            collected.push(relative.display().to_string());
        }
    }
    Ok(collected)
}
