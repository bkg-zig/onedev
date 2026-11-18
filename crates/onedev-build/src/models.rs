//! Build-spec model — 1-to-1 port of the Vanilla `.onedev-buildspec.yml`
//! format (`io.onedev.server.buildspec`, v17.2.1).
//!
//! OneDev has no pipelines/stages/runners: a spec is a set of named jobs,
//! each with typed steps (`type: CommandStep`, ...), typed triggers
//! (`type: BranchUpdateTrigger`, ...) and job dependencies.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// A parsed `.onedev-buildspec.yml`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct BuildSpec {
    pub version: Option<u32>,
    #[serde(default)]
    pub imports: Vec<SpecImport>,
    #[serde(default)]
    pub jobs: Vec<JobSpec>,
    /// Project properties referenced as `@property:name@` (kept raw).
    #[serde(default)]
    pub properties: Vec<serde_json::Value>,
}

/// `imports:` entry — reuse jobs/templates of another project.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SpecImport {
    pub project_path: String,
    pub revision: Option<String>,
}

/// One named job (`io.onedev.server.buildspec.job.Job`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JobSpec {
    pub name: String,
    #[serde(default)]
    pub execute_in_sandbox: bool,
    pub job_executor: Option<String>,
    pub sandbox_template: Option<String>,
    #[serde(default)]
    pub steps: Vec<Step>,
    #[serde(default)]
    pub param_specs: Vec<ParamSpec>,
    #[serde(default)]
    pub job_dependencies: Vec<JobDependency>,
    #[serde(default)]
    pub project_dependencies: Vec<serde_json::Value>,
    #[serde(default)]
    pub required_services: Vec<String>,
    #[serde(default)]
    pub triggers: Vec<Trigger>,
    /// Seconds; vanilla default is 14400 (4h).
    #[serde(default = "default_timeout")]
    pub timeout: u64,
    #[serde(default)]
    pub post_build_actions: Vec<serde_json::Value>,
    pub sequential_group: Option<String>,
    #[serde(default = "default_retry_condition")]
    pub retry_condition: String,
    #[serde(default = "default_max_attempts")]
    pub max_attempts: u32,
    #[serde(default = "default_retry_delay")]
    pub retry_delay: u32,
    #[serde(default)]
    pub env: HashMap<String, String>,
}

fn default_timeout() -> u64 {
    14400
}
fn default_retry_condition() -> String {
    "never".to_owned()
}
fn default_max_attempts() -> u32 {
    3
}
fn default_retry_delay() -> u32 {
    30
}

impl JobSpec {
    /// Names of jobs this job depends on.
    pub fn needs(&self) -> Vec<String> {
        self.job_dependencies
            .iter()
            .map(|dependency| dependency.job_name.clone())
            .collect()
    }

    /// Artifact patterns published by this job's steps.
    pub fn artifact_patterns(&self) -> Vec<String> {
        self.steps
            .iter()
            .filter(|step| step.step_type == "PublishArtifactStep")
            .filter_map(|step| step.artifacts.clone())
            .collect()
    }
}

/// Job parameter specification (subset; unknown fields are preserved).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ParamSpec {
    pub name: String,
    #[serde(rename = "type")]
    pub param_type: Option<String>,
    #[serde(flatten)]
    pub extra: serde_json::Map<String, serde_json::Value>,
}

/// Dependency on another job (`JobDependency`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JobDependency {
    pub job_name: String,
    #[serde(default = "default_true")]
    pub require_successful: bool,
    #[serde(default = "default_artifacts")]
    pub artifacts: String,
    pub destination_path: Option<String>,
}

fn default_true() -> bool {
    true
}
fn default_artifacts() -> String {
    "**".to_owned()
}

/// When a step runs relative to previous steps.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "UPPERCASE")]
pub enum ExecuteCondition {
    #[default]
    Successful,
    Always,
    Failed,
}

/// One step of a job. `type` is the Vanilla step class name
/// (`CheckoutStep`, `CommandStep`, ...); fields of the concrete classes
/// are flattened, unknown fields are preserved in `extra`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Step {
    #[serde(rename = "type")]
    pub step_type: String,
    pub name: Option<String>,
    #[serde(default)]
    pub condition: ExecuteCondition,
    #[serde(default)]
    pub optional: bool,

    // CommandStep
    pub run_in_container: Option<bool>,
    pub image: Option<String>,
    pub interpreter: Option<Interpreter>,
    pub run_as: Option<String>,
    pub use_tty: Option<bool>,
    /// Simple `run:` form (also accepted by the parser for hand-written specs).
    pub run: Option<String>,

    // CheckoutStep
    pub with_lfs: Option<bool>,
    pub with_submodules: Option<bool>,
    pub clone_depth: Option<u32>,
    pub checkout_path: Option<String>,

    // PublishArtifactStep
    pub source_path: Option<String>,
    pub artifacts: Option<String>,

    // UseTemplateStep
    pub template_name: Option<String>,

    #[serde(flatten)]
    pub extra: serde_json::Map<String, serde_json::Value>,
}

impl Step {
    pub fn display_name(&self) -> &str {
        self.name.as_deref().unwrap_or(&self.step_type)
    }

    /// Shell commands of a `CommandStep` (via its interpreter).
    pub fn commands(&self) -> Option<&str> {
        if let Some(run) = &self.run {
            return Some(run);
        }
        self.interpreter.as_ref().map(|i| i.commands.as_str())
    }
}

/// Command interpreter (`PosixInterpreter`, `WindowsBatchInterpreter`,
/// `PowerShellInterpreter`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Interpreter {
    #[serde(rename = "type")]
    pub interpreter_type: String,
    #[serde(default = "default_shell")]
    pub shell: String,
    pub powershell: Option<String>,
    #[serde(default)]
    pub commands: String,
}

fn default_shell() -> String {
    "sh".to_owned()
}

/// What starts a job — the Vanilla trigger class names.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Trigger {
    BranchUpdateTrigger {
        #[serde(default)]
        branches: Option<String>,
        #[serde(default)]
        paths: Option<String>,
        #[serde(default)]
        projects: Option<String>,
    },
    TagCreateTrigger {
        #[serde(default)]
        tags: Option<String>,
        #[serde(default)]
        branches: Option<String>,
        #[serde(default)]
        projects: Option<String>,
    },
    PullRequestUpdateTrigger {
        #[serde(default)]
        branches: Option<String>,
        #[serde(default)]
        paths: Option<String>,
        #[serde(default)]
        projects: Option<String>,
    },
    PullRequestMergeTrigger {
        #[serde(default)]
        branches: Option<String>,
        #[serde(default)]
        paths: Option<String>,
        #[serde(default)]
        projects: Option<String>,
    },
    PullRequestDiscardTrigger {
        #[serde(default)]
        branches: Option<String>,
        #[serde(default)]
        projects: Option<String>,
    },
    ScheduleTrigger {
        #[serde(rename = "cronExpression", default)]
        cron: Option<String>,
        #[serde(default)]
        branches: Option<String>,
        #[serde(default)]
        projects: Option<String>,
    },
    DependencyFinishedTrigger {
        #[serde(default)]
        projects: Option<String>,
    },
    IssueInStateTrigger {
        #[serde(default)]
        state: Option<String>,
        #[serde(default)]
        projects: Option<String>,
    },
}

/// Space-separated pattern match (empty/`None` matches everything).
fn patterns_match(patterns: Option<&str>, value: &str) -> bool {
    match patterns {
        None => true,
        Some(patterns) => {
            let patterns = patterns.trim();
            patterns.is_empty()
                || patterns
                    .split_whitespace()
                    .any(|pattern| crate::utils::glob_matches(pattern, value))
        }
    }
}

impl Trigger {
    /// Whether this trigger fires for the given event.
    pub fn matches(&self, event: &TriggerEvent) -> bool {
        match (self, event) {
            (Trigger::BranchUpdateTrigger { branches, .. }, TriggerEvent::Push { branch }) => {
                patterns_match(branches.as_deref(), branch)
            }
            (Trigger::TagCreateTrigger { tags, .. }, TriggerEvent::Tag { tag }) => {
                patterns_match(tags.as_deref(), tag)
            }
            (
                Trigger::PullRequestUpdateTrigger { .. }
                | Trigger::PullRequestMergeTrigger { .. }
                | Trigger::PullRequestDiscardTrigger { .. },
                TriggerEvent::PullRequest,
            ) => true,
            (Trigger::ScheduleTrigger { .. }, TriggerEvent::Schedule) => true,
            (Trigger::DependencyFinishedTrigger { .. }, TriggerEvent::DependencyFinished) => true,
            (Trigger::IssueInStateTrigger { .. }, TriggerEvent::Manual) => false,
            _ => false,
        }
    }
}

/// An event that can start jobs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TriggerEvent {
    Push { branch: String },
    Tag { tag: String },
    PullRequest,
    Schedule,
    DependencyFinished,
    Manual,
}

/// Status of a job or a whole build.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "build_status", rename_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub enum BuildStatus {
    Pending,
    Running,
    Success,
    Failed,
    Skipped,
    Cancelled,
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
