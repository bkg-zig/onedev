//! Build / CI/CD Domain Model

use super::*;

/// Build status
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, strum::EnumString, strum::Display,
)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case")]
pub enum BuildStatus {
    Pending,
    Running,
    Success,
    Failed,
    Canceled,
    Skipped,
    Manual,
    Scheduled,
    WaitingForResource,
    Preparing,
}

/// Build stage
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, strum::EnumString, strum::Display,
)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case")]
pub enum BuildStage {
    Prepare,
    Clone,
    Setup,
    Build,
    Test,
    Deploy,
    Cleanup,
    Complete,
}

/// Core Build entity
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Build {
    pub id: i64,
    pub uuid: Uuid,
    pub project_id: i64,
    pub number: i64,
    pub job_name: String,
    pub status: BuildStatus,
    pub stage: BuildStage,
    pub ref_name: String, // branch, tag, ref
    pub ref_type: RefType,
    pub commit_sha: String,
    pub commit_message: Option<String>,
    pub commit_author_name: String,
    pub commit_author_email: String,
    pub commit_committer_name: String,
    pub commit_committer_email: String,
    pub commit_date: DateTime<Utc>,
    pub trigger: BuildTrigger,
    pub trigger_user_id: Option<i64>,
    pub pipeline_id: Option<i64>,
    pub parent_build_id: Option<i64>,
    // Timing
    pub queued_at: DateTime<Utc>,
    pub started_at: Option<DateTime<Utc>>,
    pub finished_at: Option<DateTime<Utc>>,
    pub duration: Option<i64>, // seconds
    // Executor
    pub executor_type: ExecutorType,
    pub executor_data: serde_json::Value, // Executor-specific data
    pub runner_id: Option<i64>,
    // Resources
    pub cpu_limit: Option<u32>,
    pub memory_limit: Option<u64>, // bytes
    // Artifacts & Cache
    pub artifacts: Vec<Artifact>,
    pub cache_keys: Vec<String>,
    // Environment
    pub variables: HashMap<String, String>, // Non-secret env vars
    // Results
    pub exit_code: Option<i32>,
    pub failure_reason: Option<String>,
    // Reports
    pub coverage: Option<f64>,
    pub test_report: Option<TestReportSummary>,
    // Timestamps
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, strum::EnumString, strum::Display,
)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case")]
pub enum RefType {
    Branch,
    Tag,
    Commit,
    MergeRequest,
    Pipeline,
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, strum::EnumString, strum::Display,
)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case")]
pub enum BuildTrigger {
    Push,
    PullRequest,
    Schedule,
    Api,
    Webhook,
    Manual,
    Rebuild,
    Rollback,
    MergeTrain,
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, strum::EnumString, strum::Display,
)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case")]
pub enum ExecutorType {
    Local,
    Docker,
    Kubernetes,
    Sandbox,
    RemoteHost,
    RemoteDocker,
}

/// Artifact
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Artifact {
    pub name: String,
    pub path: String,
    pub size: u64,
    pub sha256: String,
    pub mime_type: Option<String>,
    pub expires_at: Option<DateTime<Utc>>,
    pub artifact_type: ArtifactType,
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, strum::EnumString, strum::Display,
)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case")]
pub enum ArtifactType {
    Build,
    Test,
    Coverage,
    Package,
    DockerImage,
    HelmChart,
    Generic,
}

/// Test report summary
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct TestReportSummary {
    pub total: u32,
    pub passed: u32,
    pub failed: u32,
    pub skipped: u32,
    pub error: u32,
    pub duration: f64, // seconds
    pub suites: Vec<TestSuiteSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct TestSuiteSummary {
    pub name: String,
    pub total: u32,
    pub passed: u32,
    pub failed: u32,
    pub skipped: u32,
    pub error: u32,
    pub duration: f64,
}

/// Build reference
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BuildRef {
    pub id: i64,
    pub uuid: Uuid,
    pub project_id: i64,
    pub number: i64,
    pub job_name: String,
    pub status: BuildStatus,
    pub ref_name: String,
    pub sha: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Build creation input (internal, from pipeline)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateBuild {
    pub project_id: i64,
    pub job_name: String,
    pub ref_name: String,
    pub ref_type: RefType,
    pub commit_sha: String,
    pub trigger: BuildTrigger,
    pub trigger_user_id: Option<i64>,
    pub pipeline_id: Option<i64>,
    pub parent_build_id: Option<i64>,
    pub executor_type: ExecutorType,
    pub executor_data: serde_json::Value,
    pub variables: HashMap<String, String>,
    pub cpu_limit: Option<u32>,
    pub memory_limit: Option<u64>,
    pub cache_keys: Vec<String>,
}

/// Build query params
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct BuildQuery {
    pub status: Option<BuildStatus>,
    pub job_name: Option<String>,
    pub ref_name: Option<String>,
    pub trigger: Option<BuildTrigger>,
    pub executor_type: Option<ExecutorType>,
    pub project_id: Option<i64>,
    pub pipeline_id: Option<i64>,
    pub from_date: Option<DateTime<Utc>>,
    pub to_date: Option<DateTime<Utc>>,
    pub sort: Option<String>,
    pub order: Option<SortOrder>,
    pub page: u32,
    pub per_page: u32,
}

/// Pipeline (group of builds)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Pipeline {
    pub id: i64,
    pub uuid: Uuid,
    pub project_id: i64,
    pub number: i64,
    pub ref_name: String,
    pub sha: String,
    pub status: BuildStatus, // Aggregated status
    pub trigger: BuildTrigger,
    pub trigger_user_id: Option<i64>,
    pub variables: HashMap<String, String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub finished_at: Option<DateTime<Utc>>,
    pub duration: Option<i64>,
}

/// Build Spec (YAML definition)
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct BuildSpec {
    pub version: String,
    pub variables: HashMap<String, String>,
    pub stages: Vec<String>,
    pub jobs: HashMap<String, JobSpec>,
    pub workflow: Option<WorkflowSpec>,
    pub cache: Option<CacheSpec>,
    pub artifacts: Option<ArtifactsSpec>,
    pub services: HashMap<String, ServiceSpec>,
}

/// Job specification
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct JobSpec {
    pub stage: Option<String>,
    pub image: Option<String>,
    pub services: Vec<String>,
    pub before_script: Vec<String>,
    pub script: Vec<String>,
    pub after_script: Vec<String>,
    pub variables: HashMap<String, String>,
    pub rules: Vec<RuleSpec>,
    pub only: Vec<String>,
    pub except: Vec<String>,
    pub tags: Vec<String>,
    pub allow_failure: bool,
    pub timeout: Option<u32>, // minutes
    pub retry: Option<RetrySpec>,
    pub artifacts: Option<JobArtifactsSpec>,
    pub cache: Option<JobCacheSpec>,
    pub dependencies: Vec<String>,
    pub needs: Vec<NeedSpec>,
    pub coverage: Option<String>,
    pub environment: Option<EnvironmentSpec>,
    pub deploy: Option<DeploySpec>,
    pub release: Option<ReleaseSpec>,
    pub resource_group: Option<String>,
    pub interruptible: bool,
    pub parallel: Option<ParallelSpec>,
}

/// Rule specification (when to run job)
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RuleSpec {
    pub if_condition: Option<String>, // CI expression
    pub changes: Option<Vec<String>>,
    pub exists: Option<Vec<String>>,
    pub when: Option<RuleWhen>,
    pub allow_failure: Option<bool>,
    pub variables: Option<HashMap<String, String>>,
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, strum::EnumString, strum::Display,
)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case")]
pub enum RuleWhen {
    OnSuccess,
    OnFailure,
    Always,
    Manual,
    Delayed,
}

/// Retry specification
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RetrySpec {
    pub max: u32,
    pub when: Vec<RetryWhen>,
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, strum::EnumString, strum::Display,
)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case")]
pub enum RetryWhen {
    Always,
    UnknownFailure,
    StuckOrTimeoutFailure,
    RunnerSystemFailure,
    MissingDependencyFailure,
    RunnerUnsupportedFailure,
    StaleScheduleFailure,
    JobExecutionTimeout,
    ArchivalFailure,
    UnmetPrerequisites,
    SchedulerFailure,
    DataIntegrityFailure,
}

/// Job artifacts specification
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct JobArtifactsSpec {
    pub paths: Vec<String>,
    pub exclude: Vec<String>,
    pub expire_in: Option<String>,
    pub reports: Option<ArtifactReports>,
    pub name: Option<String>,
    pub when: Option<ArtifactWhen>,
    pub access: Option<ArtifactAccess>,
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, strum::EnumString, strum::Display,
)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case")]
pub enum ArtifactWhen {
    OnSuccess,
    OnFailure,
    Always,
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, strum::EnumString, strum::Display,
)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case")]
pub enum ArtifactAccess {
    Public,
    Private,
}

/// Artifact reports
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ArtifactReports {
    pub junit: Vec<String>,
    pub coverage: Option<CoverageReportSpec>,
    pub terraform: Vec<String>,
    pub sast: Vec<String>,
    pub secret_detection: Vec<String>,
    pub dependency_scanning: Vec<String>,
    pub container_scanning: Vec<String>,
    pub dast: Vec<String>,
    pub license_scanning: Vec<String>,
    pub performance: Vec<String>,
    pub metrics: Vec<String>,
    pub test_cases: Vec<String>,
    pub codequality: Vec<String>,
    pub load_performance: Vec<String>,
    pub browser_performance: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CoverageReportSpec {
    pub coverage_format: String,
    pub path: String,
}

/// Job cache specification
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct JobCacheSpec {
    pub key: Option<String>,
    pub key_files: Vec<String>,
    pub prefix: Option<String>,
    pub paths: Vec<String>,
    pub policy: Option<CachePolicy>,
    pub when: Option<CacheWhen>,
}

#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Default,
    Serialize,
    Deserialize,
    strum::EnumString,
    strum::Display,
)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case")]
pub enum CachePolicy {
    Pull,
    Push,
    #[default]
    PullPush,
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, strum::EnumString, strum::Display,
)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case")]
pub enum CacheWhen {
    OnSuccess,
    OnFailure,
    Always,
}

/// Need specification (DAG dependencies)
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct NeedSpec {
    pub job: String,
    pub artifacts: bool,
    pub optional: bool,
}

/// Environment specification
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct EnvironmentSpec {
    pub name: String,
    pub url: Option<String>,
    pub on_stop: Option<String>,
    pub action: Option<EnvironmentAction>,
    pub tier: Option<EnvironmentTier>,
    pub auto_stop_in: Option<String>,
    pub kubernetes: Option<KubernetesEnvironmentSpec>,
    pub deployment_tier: Option<String>,
}

/// Kubernetes environment specification
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct KubernetesEnvironmentSpec {
    pub namespace: Option<String>,
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, strum::EnumString, strum::Display,
)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case")]
pub enum EnvironmentAction {
    Start,
    Stop,
    Prepare,
    Verify,
    Access,
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, strum::EnumString, strum::Display,
)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case")]
pub enum EnvironmentTier {
    Development,
    Testing,
    Staging,
    Production,
}

/// Deploy specification
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DeploySpec {
    pub environment: EnvironmentSpec,
    pub strategy: Option<DeployStrategy>,
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, strum::EnumString, strum::Display,
)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case")]
pub enum DeployStrategy {
    Rolling,
    BlueGreen,
    Canary,
}

/// Release specification
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ReleaseSpec {
    pub tag_name: String,
    pub description: Option<String>,
    #[serde(rename = "ref")]
    pub ref_name: String,
    pub assets: Vec<ReleaseAssetSpec>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ReleaseAssetSpec {
    pub name: String,
    pub url: String,
    pub filepath: Option<String>,
}

/// Parallel specification
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ParallelSpec {
    pub matrix: Option<MatrixSpec>,
    pub parallel: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct MatrixSpec {
    pub include: Vec<HashMap<String, String>>,
    pub exclude: Vec<HashMap<String, String>>,
}

/// Workflow specification
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct WorkflowSpec {
    pub rules: Vec<RuleSpec>,
}

/// Cache specification (global)
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CacheSpec {
    pub key: String,
    pub paths: Vec<String>,
    pub policy: CachePolicy,
}

/// Artifacts specification (global)
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ArtifactsSpec {
    pub paths: Vec<String>,
    pub expire_in: String,
}

/// Service specification
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ServiceSpec {
    pub image: String,
    pub name: Option<String>,
    pub alias: Option<String>,
    pub command: Option<Vec<String>>,
    pub variables: HashMap<String, String>,
    pub ports: Vec<u16>,
    pub expose: Vec<u16>,
    pub volumes: Vec<String>,
    pub tmpfs: HashMap<String, String>,
}

impl Build {
    pub fn is_running(&self) -> bool {
        matches!(
            self.status,
            BuildStatus::Running
                | BuildStatus::Pending
                | BuildStatus::WaitingForResource
                | BuildStatus::Preparing
        )
    }

    pub fn is_finished(&self) -> bool {
        matches!(
            self.status,
            BuildStatus::Success
                | BuildStatus::Failed
                | BuildStatus::Canceled
                | BuildStatus::Skipped
                | BuildStatus::Manual
        )
    }

    pub fn is_success(&self) -> bool {
        matches!(self.status, BuildStatus::Success)
    }

    pub fn reference(&self) -> String {
        format!("#{}", self.number)
    }

    pub fn full_reference(&self, project_path: &str) -> String {
        format!("{}#{}", project_path, self.number)
    }
}
