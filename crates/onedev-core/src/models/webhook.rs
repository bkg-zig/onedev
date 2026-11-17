//! Webhook Domain Model

use super::*;
use crate::utils::*;

/// Webhook entity
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Webhook {
    pub id: i64,
    pub uuid: Uuid,
    pub project_id: Option<i64>,
    pub group_id: Option<i64>,
    pub url: String,
    pub secret: Option<String>, // Encrypted
    pub events: Vec<WebhookEvent>,
    pub enabled: bool,
    pub ssl_verification: bool,
    // Filters
    pub branch_filter: Option<String>,
    pub tag_filter: Option<String>,
    pub author_filter: Option<String>,
    // Retry policy
    pub max_retries: u32,
    pub retry_interval: u32, // seconds
    pub timeout: u32, // seconds
    // Status
    pub last_delivery_at: Option<DateTime<Utc>>,
    pub last_delivery_status: Option<WebhookDeliveryStatus>,
    pub last_delivery_error: Option<String>,
    pub consecutive_failures: u32,
    // Timestamps
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, strum::EnumString, strum::Display)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case")]
pub enum WebhookEvent {
    Push,
    TagPush,
    TagDelete,
    IssueCreated,
    IssueUpdated,
    IssueClosed,
    IssueReopened,
    IssueCommented,
    MergeRequestCreated,
    MergeRequestUpdated,
    MergeRequestMerged,
    MergeRequestClosed,
    MergeRequestReopened,
    MergeRequestCommented,
    MergeRequestApproved,
    MergeRequestUnapproved,
    PipelineCreated,
    PipelineRunning,
    PipelineSuccess,
    PipelineFailed,
    PipelineCanceled,
    PipelineSkipped,
    BuildCreated,
    BuildRunning,
    BuildSuccess,
    BuildFailed,
    BuildCanceled,
    BuildSkipped,
    DeploymentCreated,
    DeploymentSuccess,
    DeploymentFailed,
    ReleaseCreated,
    ReleaseUpdated,
    ReleaseDeleted,
    WikiPageCreated,
    WikiPageUpdated,
    WikiPageDeleted,
    MemberAdded,
    MemberRemoved,
    MemberUpdated,
    ProjectCreated,
    ProjectUpdated,
    ProjectDeleted,
    ProjectRenamed,
    ProjectTransferred,
    GroupCreated,
    GroupUpdated,
    GroupDeleted,
    GroupRenamed,
    SubgroupCreated,
    SubgroupUpdated,
    SubgroupDeleted,
    PackagePublished,
    PackageDeleted,
    Custom(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, strum::EnumString, strum::Display)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case")]
pub enum WebhookDeliveryStatus {
    Success,
    Failed,
    Timeout,
    InvalidResponse,
    SslError,
    ConnectionError,
}

/// Webhook delivery log
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WebhookDelivery {
    pub id: i64,
    pub uuid: Uuid,
    pub webhook_id: i64,
    pub event: WebhookEvent,
    pub request_headers: HashMap<String, String>,
    pub request_body: String,
    pub response_status: Option<u16>,
    pub response_headers: Option<HashMap<String, String>>,
    pub response_body: Option<String>,
    pub status: WebhookDeliveryStatus,
    pub error: Option<String>,
    pub duration_ms: u64,
    pub attempt: u32,
    pub created_at: DateTime<Utc>,
}

/// Webhook creation input
#[derive(Debug, Clone, Serialize, Deserialize, validator::Validate)]
pub struct CreateWebhook {
    #[validate(url)]
    pub url: String,

    pub secret: Option<String>,
    pub events: Vec<WebhookEvent>,
    pub enabled: bool,
    pub ssl_verification: bool,
    pub branch_filter: Option<String>,
    pub tag_filter: Option<String>,
    pub author_filter: Option<String>,
    pub max_retries: Option<u32>,
    pub retry_interval: Option<u32>,
    pub timeout: Option<u32>,
}

/// Webhook update input
#[derive(Debug, Clone, Serialize, Deserialize, validator::Validate, Default)]
pub struct UpdateWebhook {
    #[validate(url)]
    pub url: Option<String>,
    pub secret: Option<Option<String>>,
    pub events: Option<Vec<WebhookEvent>>,
    pub enabled: Option<bool>,
    pub ssl_verification: Option<bool>,
    pub branch_filter: Option<Option<String>>,
    pub tag_filter: Option<Option<String>>,
    pub author_filter: Option<Option<String>>,
    pub max_retries: Option<u32>,
    pub retry_interval: Option<u32>,
    pub timeout: Option<u32>,
}

/// Webhook query params
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct WebhookQuery {
    pub project_id: Option<i64>,
    pub group_id: Option<i64>,
    pub enabled: Option<bool>,
    pub event: Option<WebhookEvent>,
    pub sort: Option<String>,
    pub order: Option<SortOrder>,
    pub page: u32,
    pub per_page: u32,
}

/// Webhook test input
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestWebhook {
    pub event: WebhookEvent,
}

/// Webhook event payload (what gets sent)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebhookPayload {
    pub object_kind: String,
    pub event_name: String,
    pub before: Option<String>,
    pub after: Option<String>,
    pub ref_: Option<String>,
    pub checkout_sha: Option<String>,
    pub user_id: Option<i64>,
    pub user_name: Option<String>,
    pub user_email: Option<String>,
    pub project: Option<ProjectRef>,
    pub repository: Option<RepositoryInfo>,
    pub commits: Option<Vec<CommitInfo>>,
    pub total_commits_count: Option<u32>,
    pub issue: Option<IssueRef>,
    pub merge_request: Option<MergeRequestRef>,
    pub pipeline: Option<BuildRef>,
    pub build: Option<BuildRef>,
    pub release: Option<ReleaseRef>,
    pub wiki_page: Option<WikiPageRef>,
    pub member: Option<GroupMember>,
    pub custom: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RepositoryInfo {
    pub name: String,
    pub url: String,
    pub description: Option<String>,
    pub homepage: String,
    pub git_http_url: String,
    pub git_ssh_url: String,
    pub visibility: ProjectVisibility,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CommitInfo {
    pub id: String,
    pub message: String,
    pub title: String,
    pub timestamp: DateTime<Utc>,
    pub url: String,
    pub author: CommitAuthor,
    pub added: Vec<String>,
    pub modified: Vec<String>,
    pub removed: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CommitAuthor {
    pub name: String,
    pub email: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ReleaseRef {
    pub id: i64,
    pub uuid: Uuid,
    pub tag_name: String,
    pub description: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WikiPageRef {
    pub id: i64,
    pub uuid: Uuid,
    pub title: String,
    pub content: Option<String>,
    pub format: String,
    pub action: String,
    pub url: String,
}

impl Webhook {
    pub fn is_project_webhook(&self) -> bool {
        self.project_id.is_some()
    }

    pub fn is_group_webhook(&self) -> bool {
        self.group_id.is_some()
    }

    pub fn is_system_webhook(&self) -> bool {
        self.project_id.is_none() && self.group_id.is_none()
    }

    pub fn matches_event(&self, event: WebhookEvent) -> bool {
        self.events.iter().any(|e| e == &event)
    }

    pub fn should_deliver(&self) -> bool {
        self.enabled && self.consecutive_failures < self.max_retries + 5
    }
}