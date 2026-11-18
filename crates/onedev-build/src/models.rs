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
    /// Sidecar services (databases, caches, ...) jobs can require.
    #[serde(default)]
    pub services: Vec<ServiceSpec>,
    /// Reusable step sequences referenced by `UseTemplateStep`.
    #[serde(default)]
    pub step_templates: Vec<StepTemplate>,
}

/// A service container a job may require (`io.onedev.server.buildspec.Service`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServiceSpec {
    pub name: String,
    pub image: String,
    /// Extra arguments passed to the container entrypoint.
    pub arguments: Option<String>,
    #[serde(default)]
    pub env_vars: Vec<EnvVar>,
    /// Command run inside the service until it exits 0.
    pub readiness_check_command: Option<String>,
    #[serde(default)]
    pub run_as: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EnvVar {
    pub name: String,
    #[serde(default)]
    pub value: String,
}

/// A named, reusable list of steps (`io.onedev.server.buildspec.step.StepTemplate`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StepTemplate {
    pub name: String,
    #[serde(default)]
    pub steps: Vec<Step>,
    #[serde(default)]
    pub param_specs: Vec<ParamSpec>,
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

impl BuildSpec {
    /// A copy that only keeps the triggers applicable to `project_path`
    /// (their `projects` filter admits it).
    pub fn for_project(&self, project_path: &str) -> BuildSpec {
        let mut copy = self.clone();
        for job in &mut copy.jobs {
            job.triggers
                .retain(|trigger| trigger.applies_to_project(project_path));
        }
        copy
    }

    /// Whether any trigger filters on changed files.
    pub fn has_path_filters(&self) -> bool {
        self.jobs
            .iter()
            .flat_map(|job| &job.triggers)
            .any(|trigger| trigger.paths().is_some())
    }

    /// Whether any trigger filters on the branches containing a tagged commit.
    pub fn has_tag_branch_filters(&self) -> bool {
        self.jobs
            .iter()
            .flat_map(|job| &job.triggers)
            .any(Trigger::filters_tag_branches)
    }
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

/// Include/exclude pattern match with path semantics (empty/`None` matches
/// everything), like vanilla's `PatternSet.matches(new PathMatcher(), ..)`.
fn patterns_match(patterns: Option<&str>, value: &str) -> bool {
    patterns.is_none_or(|patterns| crate::utils::PatternSet::parse(patterns).matches_path(value))
}

/// Whether `patterns` admit at least one of `values` (no patterns admit
/// everything, values that are not known admit nothing).
fn any_match(patterns: Option<&str>, values: Option<&[String]>) -> bool {
    match (patterns, values) {
        (None, _) => true,
        (Some(patterns), Some(values)) => {
            let set = crate::utils::PatternSet::parse(patterns);
            values.iter().any(|value| set.matches_path(value))
        }
        (Some(_), None) => false,
    }
}

impl Trigger {
    /// The `projects` filter: projects whose events this trigger reacts to.
    pub fn projects(&self) -> Option<&str> {
        match self {
            Trigger::BranchUpdateTrigger { projects, .. }
            | Trigger::TagCreateTrigger { projects, .. }
            | Trigger::PullRequestUpdateTrigger { projects, .. }
            | Trigger::PullRequestMergeTrigger { projects, .. }
            | Trigger::PullRequestDiscardTrigger { projects, .. }
            | Trigger::ScheduleTrigger { projects, .. }
            | Trigger::DependencyFinishedTrigger { projects, .. }
            | Trigger::IssueInStateTrigger { projects, .. } => projects.as_deref(),
        }
    }

    /// Whether the `projects` filter admits events of `project_path`.
    pub fn applies_to_project(&self, project_path: &str) -> bool {
        patterns_match(self.projects(), project_path)
    }

    /// The `paths` filter (triggers that look at changed files).
    pub fn paths(&self) -> Option<&str> {
        match self {
            Trigger::BranchUpdateTrigger { paths, .. }
            | Trigger::PullRequestUpdateTrigger { paths, .. }
            | Trigger::PullRequestMergeTrigger { paths, .. } => paths.as_deref(),
            _ => None,
        }
    }

    /// Whether this trigger needs the branches that contain a created tag.
    pub fn filters_tag_branches(&self) -> bool {
        matches!(
            self,
            Trigger::TagCreateTrigger {
                branches: Some(_),
                ..
            }
        )
    }

    /// Whether this trigger fires for the given event. (`projects` filters are
    /// about the project the event happened in; the caller applies them.)
    pub fn matches(&self, event: &TriggerEvent) -> bool {
        match (self, event) {
            (
                Trigger::BranchUpdateTrigger {
                    branches, paths, ..
                },
                TriggerEvent::Push {
                    branch,
                    changed_paths,
                },
            ) => {
                patterns_match(branches.as_deref(), branch)
                    && any_match(paths.as_deref(), changed_paths.as_deref())
            }
            (
                Trigger::TagCreateTrigger { tags, branches, .. },
                TriggerEvent::Tag {
                    tag,
                    branches: containing,
                },
            ) => {
                patterns_match(tags.as_deref(), tag)
                    && any_match(branches.as_deref(), containing.as_deref())
            }
            (
                Trigger::PullRequestUpdateTrigger {
                    branches, paths, ..
                },
                TriggerEvent::PullRequest {
                    kind: PullRequestEvent::Updated,
                    target_branch,
                    changed_paths,
                },
            )
            | (
                Trigger::PullRequestMergeTrigger {
                    branches, paths, ..
                },
                TriggerEvent::PullRequest {
                    kind: PullRequestEvent::Merged,
                    target_branch,
                    changed_paths,
                },
            ) => {
                patterns_match(branches.as_deref(), target_branch)
                    && any_match(paths.as_deref(), changed_paths.as_deref())
            }
            (
                Trigger::PullRequestDiscardTrigger { branches, .. },
                TriggerEvent::PullRequest {
                    kind: PullRequestEvent::Discarded,
                    target_branch,
                    ..
                },
            ) => patterns_match(branches.as_deref(), target_branch),
            (Trigger::ScheduleTrigger { .. }, TriggerEvent::Schedule) => true,
            (Trigger::DependencyFinishedTrigger { .. }, TriggerEvent::DependencyFinished) => true,
            _ => false,
        }
    }
}

/// What happened to a pull request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PullRequestEvent {
    /// Opened, or its head commit changed (`PullRequestUpdateTrigger`).
    Updated,
    /// Merged (`PullRequestMergeTrigger`).
    Merged,
    /// Discarded (`PullRequestDiscardTrigger`).
    Discarded,
}

/// An event that can start jobs.
///
/// Data that is `None` could not be determined (for example because git
/// failed): triggers that filter on it do not fire, so a broken repository
/// never starts a job a filter was meant to keep from running.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TriggerEvent {
    /// A branch was created or updated.
    Push {
        branch: String,
        /// Files the push changed (for `paths` filters).
        changed_paths: Option<Vec<String>>,
    },
    /// A tag was created.
    Tag {
        tag: String,
        /// Branches that contain the tagged commit (for `branches` filters).
        branches: Option<Vec<String>>,
    },
    PullRequest {
        kind: PullRequestEvent,
        /// Branch the pull request targets.
        target_branch: String,
        /// Files the pull request changes (for `paths` filters).
        changed_paths: Option<Vec<String>>,
    },
    Schedule,
    DependencyFinished,
    Manual,
}

impl TriggerEvent {
    /// A push to `branch`; `paths` filters match every file.
    pub fn push(branch: impl Into<String>) -> Self {
        TriggerEvent::Push {
            branch: branch.into(),
            changed_paths: None,
        }
    }

    /// A tag creation; `branches` filters do not match.
    pub fn tag(tag: impl Into<String>) -> Self {
        TriggerEvent::Tag {
            tag: tag.into(),
            branches: None,
        }
    }
}

/// Status of a job or a whole build.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type, Default)]
#[sqlx(type_name = "build_status", rename_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub enum BuildStatus {
    #[default]
    Pending,
    Running,
    Success,
    Failed,
    Skipped,
    Cancelled,
}

impl std::fmt::Display for BuildStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BuildStatus::Pending => write!(f, "Pending"),
            BuildStatus::Running => write!(f, "Running"),
            BuildStatus::Success => write!(f, "Success"),
            BuildStatus::Failed => write!(f, "Failed"),
            BuildStatus::Skipped => write!(f, "Skipped"),
            BuildStatus::Cancelled => write!(f, "Cancelled"),
        }
    }
}

/// One executed step.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StepResult {
    pub name: String,
    pub status: BuildStatus,
    pub exit_code: Option<i32>,
    /// Wall-clock duration of the step.
    #[serde(default)]
    pub duration_ms: u64,
    /// Files this step published (relative to the job's artifact directory).
    #[serde(default)]
    pub artifacts: Vec<String>,
}

impl StepResult {
    pub fn new(name: impl Into<String>, status: BuildStatus, exit_code: Option<i32>) -> Self {
        StepResult {
            name: name.into(),
            status,
            exit_code,
            duration_ms: 0,
            artifacts: Vec::new(),
        }
    }
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
    #[serde(default)]
    pub duration_ms: u64,
}

/// Result of a whole build run.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BuildReport {
    pub status: BuildStatus,
    pub jobs: Vec<JobResult>,
    #[serde(default)]
    pub duration_ms: u64,
    /// Why the build did not complete normally (`cancelled`, `timed out`, ...).
    #[serde(default)]
    pub error: Option<String>,
}

/// One line of build output, streamed while jobs run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LogLine {
    pub job: String,
    pub step: String,
    pub stderr: bool,
    pub line: String,
}

#[cfg(test)]
mod trigger_tests {
    use super::*;

    fn branch_trigger(branches: Option<&str>, paths: Option<&str>) -> Trigger {
        Trigger::BranchUpdateTrigger {
            branches: branches.map(str::to_owned),
            paths: paths.map(str::to_owned),
            projects: None,
        }
    }

    fn push(branch: &str, changed: Option<&[&str]>) -> TriggerEvent {
        TriggerEvent::Push {
            branch: branch.to_owned(),
            changed_paths: changed.map(|files| files.iter().map(|f| (*f).to_owned()).collect()),
        }
    }

    #[test]
    fn branch_update_filters_branches_and_changed_paths() {
        let any = branch_trigger(None, None);
        assert!(any.matches(&TriggerEvent::push("whatever")));

        let main_only = branch_trigger(Some("main release/**"), None);
        assert!(main_only.matches(&TriggerEvent::push("main")));
        assert!(main_only.matches(&TriggerEvent::push("release/1.0")));
        assert!(!main_only.matches(&TriggerEvent::push("feature/x")));
        let not_wip = branch_trigger(Some("** -wip/**"), None);
        assert!(not_wip.matches(&TriggerEvent::push("main")));
        assert!(!not_wip.matches(&TriggerEvent::push("wip/x")));

        let docs_only = branch_trigger(None, Some("docs/** *.md"));
        assert!(docs_only.matches(&push("main", Some(&["src/a.rs", "docs/guide/x.txt"]))));
        assert!(docs_only.matches(&push("main", Some(&["README.md"]))));
        assert!(!docs_only.matches(&push("main", Some(&["src/a.rs"]))));
        assert!(!docs_only.matches(&push("main", Some(&[]))), "no changed file, nothing matches");
        // unknown changed files: a path filter must not fire blindly
        assert!(!docs_only.matches(&push("main", None)));
        // but without a path filter they do not matter
        assert!(any.matches(&push("main", None)));

        let not_docs = branch_trigger(None, Some("** -docs/**"));
        assert!(not_docs.matches(&push("main", Some(&["docs/a.md", "src/lib.rs"]))));
        assert!(!not_docs.matches(&push("main", Some(&["docs/a.md"]))));
    }

    #[test]
    fn tag_create_filters_tags_and_the_branches_containing_the_commit() {
        let trigger = Trigger::TagCreateTrigger {
            tags: Some("v* -v*-rc*".to_owned()),
            branches: Some("main".to_owned()),
            projects: None,
        };
        let tag = |name: &str, branches: Option<&[&str]>| TriggerEvent::Tag {
            tag: name.to_owned(),
            branches: branches.map(|b| b.iter().map(|x| (*x).to_owned()).collect()),
        };
        assert!(trigger.matches(&tag("v1.0", Some(&["main", "dev"]))));
        assert!(!trigger.matches(&tag("v1.0-rc1", Some(&["main"]))));
        assert!(!trigger.matches(&tag("nightly", Some(&["main"]))));
        assert!(!trigger.matches(&tag("v1.0", Some(&["dev"]))), "tagged commit is not on main");
        assert!(!trigger.matches(&tag("v1.0", None)), "unknown branches never satisfy a branch filter");
        assert!(!trigger.matches(&TriggerEvent::push("main")));

        let unrestricted = Trigger::TagCreateTrigger {
            tags: None,
            branches: None,
            projects: None,
        };
        assert!(unrestricted.matches(&TriggerEvent::tag("anything")));
    }

    #[test]
    fn pull_request_triggers_follow_the_kind_target_branch_and_paths() {
        let pr = |kind, target: &str, changed: Option<&[&str]>| TriggerEvent::PullRequest {
            kind,
            target_branch: target.to_owned(),
            changed_paths: changed.map(|files| files.iter().map(|f| (*f).to_owned()).collect()),
        };
        let update = Trigger::PullRequestUpdateTrigger {
            branches: Some("main".to_owned()),
            paths: Some("src/**".to_owned()),
            projects: None,
        };
        assert!(update.matches(&pr(PullRequestEvent::Updated, "main", Some(&["src/a.rs"]))));
        assert!(!update.matches(&pr(PullRequestEvent::Updated, "dev", Some(&["src/a.rs"]))));
        assert!(!update.matches(&pr(PullRequestEvent::Updated, "main", Some(&["docs/a.md"]))));
        assert!(!update.matches(&pr(PullRequestEvent::Merged, "main", Some(&["src/a.rs"]))));
        assert!(!update.matches(&pr(PullRequestEvent::Discarded, "main", None)));

        let merge = Trigger::PullRequestMergeTrigger {
            branches: None,
            paths: None,
            projects: None,
        };
        assert!(merge.matches(&pr(PullRequestEvent::Merged, "anything", None)));
        assert!(!merge.matches(&pr(PullRequestEvent::Updated, "anything", None)));

        let discard = Trigger::PullRequestDiscardTrigger {
            branches: Some("release/**".to_owned()),
            projects: None,
        };
        assert!(discard.matches(&pr(PullRequestEvent::Discarded, "release/1", None)));
        assert!(!discard.matches(&pr(PullRequestEvent::Discarded, "main", None)));
    }

    #[test]
    fn schedule_and_dependency_triggers_only_fire_for_their_event() {
        let schedule = Trigger::ScheduleTrigger {
            cron: Some("0 0 * * * ?".to_owned()),
            branches: None,
            projects: None,
        };
        assert!(schedule.matches(&TriggerEvent::Schedule));
        assert!(!schedule.matches(&TriggerEvent::push("main")));
        let dependency = Trigger::DependencyFinishedTrigger { projects: None };
        assert!(dependency.matches(&TriggerEvent::DependencyFinished));
        assert!(!dependency.matches(&TriggerEvent::Schedule));
        assert!(!branch_trigger(None, None).matches(&TriggerEvent::Manual));
    }

    #[test]
    fn project_filters_restrict_triggers_to_matching_projects() {
        let spec = BuildSpec {
            jobs: vec![serde_json::from_value(serde_json::json!({
                "name": "build",
                "steps": [],
                "triggers": [
                    {"type": "BranchUpdateTrigger", "projects": "team/**", "paths": "src/**"},
                    {"type": "TagCreateTrigger", "branches": "main"},
                    {"type": "ScheduleTrigger", "projects": "-team/legacy"},
                ],
            }))
            .unwrap()],
            ..BuildSpec::default()
        };
        assert!(spec.has_path_filters());
        assert!(spec.has_tag_branch_filters());

        let in_team = spec.for_project("team/app");
        assert_eq!(in_team.jobs[0].triggers.len(), 3);
        let elsewhere = spec.for_project("other/app");
        // only the triggers without (or with a matching) `projects` filter remain
        assert_eq!(elsewhere.jobs[0].triggers.len(), 2);
        assert!(!elsewhere.has_path_filters());
        let legacy = spec.for_project("team/legacy");
        assert_eq!(legacy.jobs[0].triggers.len(), 2, "excluded by `-team/legacy`");
        assert!(!BuildSpec::default().has_path_filters());
    }
}
