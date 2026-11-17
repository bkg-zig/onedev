//! Build-spec model (`.onedev-buildspec.yml`), OneDev-style.
//!
//! OneDev has no pipelines/stages/runners: a spec is a set of named jobs,
//! each with steps, triggers and dependencies on other jobs. This is the
//! format the parser accepts (a validated subset of the vanilla format):
//!
//! ```yaml
//! version: 1
//! jobs:
//!   - name: test
//!     steps:
//!       - name: unit tests
//!         run: cargo test
//!     triggers:
//!       - type: push
//!         branches: [main]
//!     needs: [build]
//!     artifacts: [target/debug/app]
//! ```

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// A parsed `.onedev-buildspec.yml`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct BuildSpec {
    pub version: Option<u32>,
    #[serde(default)]
    pub variables: HashMap<String, String>,
    pub jobs: Vec<JobSpec>,
}

/// One named job of a build spec.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct JobSpec {
    pub name: String,
    pub steps: Vec<StepSpec>,
    #[serde(default)]
    pub triggers: Vec<Trigger>,
    /// Names of jobs that must succeed before this job runs.
    #[serde(default)]
    pub needs: Vec<String>,
    #[serde(default)]
    pub env: HashMap<String, String>,
    /// Executor key; only `host` is implemented (see port-review 05, MVP).
    #[serde(default = "default_executor")]
    pub executor: String,
    #[serde(default)]
    pub retries: u32,
    pub timeout_seconds: Option<u64>,
    /// Paths or globs, relative to the work dir, copied to the artifact store.
    #[serde(default)]
    pub artifacts: Vec<String>,
    #[serde(default)]
    pub allow_failure: bool,
}

fn default_executor() -> String {
    "host".to_owned()
}

/// One command step of a job.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StepSpec {
    pub name: Option<String>,
    pub run: String,
}

impl StepSpec {
    pub fn display_name(&self) -> &str {
        self.name.as_deref().unwrap_or(&self.run)
    }
}

/// What starts a job.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Trigger {
    /// Branch or tag push; empty `branches` matches any branch.
    Push {
        #[serde(default)]
        branches: Vec<String>,
    },
    PullRequest,
    Schedule {
        cron: String,
    },
    Manual,
}

impl Trigger {
    /// Whether this trigger fires for the given event.
    pub fn matches(&self, event: &TriggerEvent) -> bool {
        match (self, event) {
            (Trigger::Push { branches }, TriggerEvent::Push { branch }) => {
                branches.is_empty() || branches.iter().any(|b| b == branch)
            }
            (Trigger::PullRequest, TriggerEvent::PullRequest) => true,
            (Trigger::Schedule { .. }, TriggerEvent::Schedule) => true,
            (Trigger::Manual, TriggerEvent::Manual) => true,
            _ => false,
        }
    }
}

/// An event that can start jobs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TriggerEvent {
    Push { branch: String },
    PullRequest,
    Schedule,
    Manual,
}

/// Status of a job or a whole build.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum BuildStatus {
    Success,
    Failed,
    Skipped,
}

/// One executed step.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StepResult {
    pub name: String,
    pub status: BuildStatus,
    pub exit_code: Option<i32>,
}

/// One executed job.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JobResult {
    pub name: String,
    pub status: BuildStatus,
    pub steps: Vec<StepResult>,
    /// Files collected into the artifact store.
    pub artifacts: Vec<String>,
    pub attempts: u32,
}

/// Result of a whole build run.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BuildReport {
    pub status: BuildStatus,
    pub jobs: Vec<JobResult>,
}

/// One line of build output, streamed while jobs run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LogLine {
    pub job: String,
    pub step: String,
    pub stderr: bool,
    pub line: String,
}
