//! Event System

use crate::models::*;
use crate::utils::*;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Event trait
pub trait Event: Send + Sync + Serialize + for<'de> Deserialize<'de> + std::fmt::Debug {
    fn event_type(&self) -> &'static str;
    fn aggregate_id(&self) -> Option<String>;
    fn correlation_id(&self) -> Option<String>;
    fn causation_id(&self) -> Option<String>;
    fn timestamp(&self) -> DateTime<Utc>;
    fn metadata(&self) -> HashMap<String, serde_json::Value>;
}

/// Base event structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BaseEvent {
    pub event_id: Uuid,
    pub event_type: String,
    pub aggregate_id: Option<String>,
    pub correlation_id: Option<String>,
    pub causation_id: Option<String>,
    pub timestamp: DateTime<Utc>,
    pub metadata: HashMap<String, serde_json::Value>,
    pub payload: serde_json::Value,
}

impl Event for BaseEvent {
    fn event_type(&self) -> &'static str {
        &self.event_type
    }

    fn aggregate_id(&self) -> Option<String> {
        self.aggregate_id.clone()
    }

    fn correlation_id(&self) -> Option<String> {
        self.correlation_id.clone()
    }

    fn causation_id(&self) -> Option<String> {
        self.causation_id.clone()
    }

    fn timestamp(&self) -> DateTime<Utc> {
        self.timestamp
    }

    fn metadata(&self) -> HashMap<String, serde_json::Value> {
        self.metadata.clone()
    }
}

impl BaseEvent {
    pub fn new<E: Event>(event: &E, payload: serde_json::Value) -> Self {
        Self {
            event_id: Uuid::new_v4(),
            event_type: event.event_type().to_string(),
            aggregate_id: event.aggregate_id(),
            correlation_id: event.correlation_id(),
            causation_id: event.causation_id(),
            timestamp: event.timestamp(),
            metadata: event.metadata(),
            payload,
        }
    }
}

/// Domain events
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "event_type")]
pub enum DomainEvent {
    // User events
    UserCreated(UserCreated),
    UserUpdated(UserUpdated),
    UserDeleted(UserDeleted),
    UserEnabled(UserEnabled),
    UserDisabled(UserDisabled),
    UserPasswordChanged(UserPasswordChanged),
    UserTwoFactorEnabled(UserTwoFactorEnabled),
    UserTwoFactorDisabled(UserTwoFactorDisabled),

    // Project events
    ProjectCreated(ProjectCreated),
    ProjectUpdated(ProjectUpdated),
    ProjectDeleted(ProjectDeleted),
    ProjectArchived(ProjectArchived),
    ProjectUnarchived(ProjectUnarchived),
    ProjectTransferred(ProjectTransferred),
    ProjectRenamed(ProjectRenamed),

    // Issue events
    IssueCreated(IssueCreated),
    IssueUpdated(IssueUpdated),
    IssueDeleted(IssueDeleted),
    IssueClosed(IssueClosed),
    IssueReopened(IssueReopened),
    IssueAssigned(IssueAssigned),
    IssueUnassigned(IssueUnassigned),
    IssueLabeled(IssueLabeled),
    IssueUnlabeled(IssueUnlabeled),
    IssueMilestoned(IssueMilestoned),
    IssueUnmilestoned(IssueUnmilestoned),
    IssueCommented(IssueCommented),

    // Merge Request events
    MergeRequestCreated(MergeRequestCreated),
    MergeRequestUpdated(MergeRequestUpdated),
    MergeRequestMerged(MergeRequestMerged),
    MergeRequestClosed(MergeRequestClosed),
    MergeRequestReopened(MergeRequestReopened),
    MergeRequestApproved(MergeRequestApproved),
    MergeRequestUnapproved(MergeRequestUnapproved),
    MergeRequestCommented(MergeRequestCommented),

    // Build events
    BuildCreated(BuildCreated),
    BuildStarted(BuildStarted),
    BuildFinished(BuildFinished),
    BuildCanceled(BuildCanceled),
    BuildRetried(BuildRetried),

    // Pipeline events
    PipelineCreated(PipelineCreated),
    PipelineRunning(PipelineRunning),
    PipelineSuccess(PipelineSuccess),
    PipelineFailed(PipelineFailed),
    PipelineCanceled(PipelineCanceled),

    // Package events
    PackagePublished(PackagePublished),
    PackageDeleted(PackageDeleted),

    // Release events
    ReleaseCreated(ReleaseCreated),
    ReleaseUpdated(ReleaseUpdated),
    ReleaseDeleted(ReleaseDeleted),

    // Git events
    PushReceived(PushReceived),
    TagCreated(TagCreated),
    TagDeleted(TagDeleted),
    BranchCreated(BranchCreated),
    BranchDeleted(BranchDeleted),

    // Member events
    MemberAdded(MemberAdded),
    MemberRemoved(MemberRemoved),
    MemberRoleChanged(MemberRoleChanged),

    // Webhook events
    WebhookTriggered(WebhookTriggered),
    WebhookDelivered(WebhookDelivered),
    WebhookFailed(WebhookFailed),

    // System events
    SystemSettingChanged(SystemSettingChanged),
    BackupCompleted(BackupCompleted),
    RestoreCompleted(RestoreCompleted),
    UpgradeCompleted(UpgradeCompleted),
}

macro_rules! define_event {
    ($name:ident, $fields:tt) => {
        #[derive(Debug, Clone, Serialize, Deserialize)]
        pub struct $name $fields

        impl Event for $name {
            fn event_type(&self) -> &'static str {
                stringify!($name)
            }

            fn aggregate_id(&self) -> Option<String> {
                Some(self.id.to_string())
            }

            fn correlation_id(&self) -> Option<String> {
                None
            }

            fn causation_id(&self) -> Option<String> {
                None
            }

            fn timestamp(&self) -> DateTime<Utc> {
                self.created_at
            }

            fn metadata(&self) -> HashMap<String, serde_json::Value> {
                HashMap::new()
            }
        }
    };
}

define_event!(UserCreated, { pub id: i64, pub uuid: Uuid, pub name: String, pub email: Option<String>, pub created_by_id: i64, pub created_at: DateTime<Utc> });
define_event!(UserUpdated, { pub id: i64, pub uuid: Uuid, pub changes: HashMap<String, serde_json::Value>, pub updated_by_id: i64, pub created_at: DateTime<Utc> });
define_event!(UserDeleted, { pub id: i64, pub uuid: Uuid, pub deleted_by_id: i64, pub created_at: DateTime<Utc> });
define_event!(UserEnabled, { pub id: i64, pub uuid: Uuid, pub enabled_by_id: i64, pub created_at: DateTime<Utc> });
define_event!(UserDisabled, { pub id: i64, pub uuid: Uuid, pub disabled_by_id: i64, pub created_at: DateTime<Utc> });
define_event!(UserPasswordChanged, { pub id: i64, pub uuid: Uuid, pub changed_by_id: i64, pub created_at: DateTime<Utc> });
define_event!(UserTwoFactorEnabled, { pub id: i64, pub uuid: Uuid, pub created_at: DateTime<Utc> });
define_event!(UserTwoFactorDisabled, { pub id: i64, pub uuid: Uuid, pub created_at: DateTime<Utc> });

define_event!(ProjectCreated, { pub id: i64, pub uuid: Uuid, pub name: String, pub path: String, pub owner_id: i64, pub visibility: ProjectVisibility, pub created_at: DateTime<Utc> });
define_event!(ProjectUpdated, { pub id: i64, pub uuid: Uuid, pub changes: HashMap<String, serde_json::Value>, pub updated_by_id: i64, pub created_at: DateTime<Utc> });
define_event!(ProjectDeleted, { pub id: i64, pub uuid: Uuid, pub deleted_by_id: i64, pub created_at: DateTime<Utc> });
define_event!(ProjectArchived, { pub id: i64, pub uuid: Uuid, pub archived_by_id: i64, pub created_at: DateTime<Utc> });
define_event!(ProjectUnarchived, { pub id: i64, pub uuid: Uuid, pub unarchived_by_id: i64, pub created_at: DateTime<Utc> });
define_event!(ProjectTransferred, { pub id: i64, pub uuid: Uuid, pub from_namespace_id: i64, pub to_namespace_id: i64, pub transferred_by_id: i64, pub created_at: DateTime<Utc> });
define_event!(ProjectRenamed, { pub id: i64, pub uuid: Uuid, pub old_path: String, pub new_path: String, pub renamed_by_id: i64, pub created_at: DateTime<Utc> });

define_event!(IssueCreated, { pub id: i64, pub uuid: Uuid, pub project_id: i64, pub number: i64, pub title: String, pub author_id: i64, pub created_at: DateTime<Utc> });
define_event!(IssueUpdated, { pub id: i64, pub uuid: Uuid, pub changes: HashMap<String, serde_json::Value>, pub updated_by_id: i64, pub created_at: DateTime<Utc> });
define_event!(IssueDeleted, { pub id: i64, pub uuid: Uuid, pub deleted_by_id: i64, pub created_at: DateTime<Utc> });
define_event!(IssueClosed, { pub id: i64, pub uuid: Uuid, pub closed_by_id: i64, pub created_at: DateTime<Utc> });
define_event!(IssueReopened, { pub id: i64, pub uuid: Uuid, pub reopened_by_id: i64, pub created_at: DateTime<Utc> });
define_event!(IssueAssigned, { pub id: i64, pub uuid: Uuid, pub assignee_id: i64, pub assigned_by_id: i64, pub created_at: DateTime<Utc> });
define_event!(IssueUnassigned, { pub id: i64, pub uuid: Uuid, pub unassigned_by_id: i64, pub created_at: DateTime<Utc> });
define_event!(IssueLabeled, { pub id: i64, pub uuid: Uuid, pub label_ids: Vec<i64>, pub labeled_by_id: i64, pub created_at: DateTime<Utc> });
define_event!(IssueUnlabeled, { pub id: i64, pub uuid: Uuid, pub label_ids: Vec<i64>, pub unlabeled_by_id: i64, pub created_at: DateTime<Utc> });
define_event!(IssueMilestoned, { pub id: i64, pub uuid: Uuid, pub milestone_id: i64, pub milestoned_by_id: i64, pub created_at: DateTime<Utc> });
define_event!(IssueUnmilestoned, { pub id: i64, pub uuid: Uuid, pub unmilestoned_by_id: i64, pub created_at: DateTime<Utc> });
define_event!(IssueCommented, { pub id: i64, pub uuid: Uuid, pub comment_id: i64, pub comment_author_id: i64, pub created_at: DateTime<Utc> });

define_event!(MergeRequestCreated, { pub id: i64, pub uuid: Uuid, pub project_id: i64, pub number: i64, pub title: String, pub author_id: i64, pub created_at: DateTime<Utc> });
define_event!(MergeRequestUpdated, { pub id: i64, pub uuid: Uuid, pub changes: HashMap<String, serde_json::Value>, pub updated_by_id: i64, pub created_at: DateTime<Utc> });
define_event!(MergeRequestMerged, { pub id: i64, pub uuid: Uuid, pub merged_by_id: i64, pub merge_commit_sha: String, pub created_at: DateTime<Utc> });
define_event!(MergeRequestClosed, { pub id: i64, pub uuid: Uuid, pub closed_by_id: i64, pub created_at: DateTime<Utc> });
define_event!(MergeRequestReopened, { pub id: i64, pub uuid: Uuid, pub reopened_by_id: i64, pub created_at: DateTime<Utc> });
define_event!(MergeRequestApproved, { pub id: i64, pub uuid: Uuid, pub approved_by_id: i64, pub created_at: DateTime<Utc> });
define_event!(MergeRequestUnapproved, { pub id: i64, pub uuid: Uuid, pub unapproved_by_id: i64, pub created_at: DateTime<Utc> });
define_event!(MergeRequestCommented, { pub id: i64, pub uuid: Uuid, pub comment_id: i64, pub comment_author_id: i64, pub created_at: DateTime<Utc> });

define_event!(BuildCreated, { pub id: i64, pub uuid: Uuid, pub project_id: i64, pub number: i64, pub job_name: String, pub pipeline_id: Option<i64>, pub created_at: DateTime<Utc> });
define_event!(BuildStarted, { pub id: i64, pub uuid: Uuid, pub started_at: DateTime<Utc>, pub runner_id: Option<i64>, pub created_at: DateTime<Utc> });
define_event!(BuildFinished, { pub id: i64, pub uuid: Uuid, pub status: BuildStatus, pub finished_at: DateTime<Utc>, pub duration: i64, pub created_at: DateTime<Utc> });
define_event!(BuildCanceled, { pub id: i64, pub uuid: Uuid, pub canceled_by_id: i64, pub created_at: DateTime<Utc> });
define_event!(BuildRetried, { pub id: i64, pub uuid: Uuid, pub retried_by_id: i64, pub created_at: DateTime<Utc> });

define_event!(PipelineCreated, { pub id: i64, pub uuid: Uuid, pub project_id: i64, pub number: i64, pub ref_name: String, pub sha: String, pub created_at: DateTime<Utc> });
define_event!(PipelineRunning, { pub id: i64, pub uuid: Uuid, pub created_at: DateTime<Utc> });
define_event!(PipelineSuccess, { pub id: i64, pub uuid: Uuid, pub finished_at: DateTime<Utc>, pub duration: i64, pub created_at: DateTime<Utc> });
define_event!(PipelineFailed, { pub id: i64, pub uuid: Uuid, pub finished_at: DateTime<Utc>, pub duration: i64, pub created_at: DateTime<Utc> });
define_event!(PipelineCanceled, { pub id: i64, pub uuid: Uuid, pub canceled_by_id: i64, pub created_at: DateTime<Utc> });

define_event!(PackagePublished, { pub id: i64, pub uuid: Uuid, pub project_id: i64, pub name: String, pub version: String, pub package_type: PackageType, pub published_by_id: i64, pub created_at: DateTime<Utc> });
define_event!(PackageDeleted, { pub id: i64, pub uuid: Uuid, pub deleted_by_id: i64, pub created_at: DateTime<Utc> });

define_event!(ReleaseCreated, { pub id: i64, pub uuid: Uuid, pub project_id: i64, pub tag_name: String, pub created_by_id: i64, pub created_at: DateTime<Utc> });
define_event!(ReleaseUpdated, { pub id: i64, pub uuid: Uuid, pub updated_by_id: i64, pub created_at: DateTime<Utc> });
define_event!(ReleaseDeleted, { pub id: i64, pub uuid: Uuid, pub deleted_by_id: i64, pub created_at: DateTime<Utc> });

define_event!(PushReceived, { pub project_id: i64, pub ref_name: String, pub before_sha: String, pub after_sha: String, pub commits: Vec<CommitInfo>, pub pusher_id: i64, pub created_at: DateTime<Utc> });
define_event!(TagCreated, { pub project_id: i64, pub tag_name: String, pub target_sha: String, pub created_by_id: i64, pub created_at: DateTime<Utc> });
define_event!(TagDeleted, { pub project_id: i64, pub tag_name: String, pub deleted_by_id: i64, pub created_at: DateTime<Utc> });
define_event!(BranchCreated, { pub project_id: i64, pub branch_name: String, pub target_sha: String, pub created_by_id: i64, pub created_at: DateTime<Utc> });
define_event!(BranchDeleted, { pub project_id: i64, pub branch_name: String, pub deleted_by_id: i64, pub created_at: DateTime<Utc> });

define_event!(MemberAdded, { pub project_id: Option<i64>, pub group_id: Option<i64>, pub user_id: i64, pub access_level: AccessLevel, pub added_by_id: i64, pub created_at: DateTime<Utc> });
define_event!(MemberRemoved, { pub project_id: Option<i64>, pub group_id: Option<i64>, pub user_id: i64, pub removed_by_id: i64, pub created_at: DateTime<Utc> });
define_event!(MemberRoleChanged, { pub project_id: Option<i64>, pub group_id: Option<i64>, pub user_id: i64, pub old_level: AccessLevel, pub new_level: AccessLevel, pub changed_by_id: i64, pub created_at: DateTime<Utc> });

define_event!(WebhookTriggered, { pub webhook_id: i64, pub event: WebhookEvent, pub delivery_id: i64, pub created_at: DateTime<Utc> });
define_event!(WebhookDelivered, { pub webhook_id: i64, pub delivery_id: i64, pub status_code: u16, pub duration_ms: u64, pub created_at: DateTime<Utc> });
define_event!(WebhookFailed, { pub webhook_id: i64, pub delivery_id: i64, pub error: String, pub attempt: u32, pub created_at: DateTime<Utc> });

define_event!(SystemSettingChanged, { pub key: String, pub old_value: Option<serde_json::Value>, pub new_value: serde_json::Value, pub changed_by_id: i64, pub created_at: DateTime<Utc> });
define_event!(BackupCompleted, { pub backup_id: String, pub size: u64, pub duration_ms: u64, pub created_at: DateTime<Utc> });
define_event!(RestoreCompleted, { pub backup_id: String, pub duration_ms: u64, pub created_at: DateTime<Utc> });
define_event!(UpgradeCompleted, { pub from_version: String, pub to_version: String, pub duration_ms: u64, pub created_at: DateTime<Utc> });

/// Event bus trait
#[async_trait]
pub trait EventBus: Send + Sync {
    async fn publish(&self, event: DomainEvent) -> Result<(), EventError>;
    async fn subscribe(&self, event_type: &str, handler: Box<dyn EventHandler>) -> Result<(), EventError>;
    async fn unsubscribe(&self, event_type: &str, handler_id: &str) -> Result<(), EventError>;
}

#[async_trait]
pub trait EventHandler: Send + Sync {
    async fn handle(&self, event: &DomainEvent) -> Result<(), EventError>;
    fn event_types(&self) -> Vec<&'static str>;
    fn handler_id(&self) -> &str;
}

/// Event error
#[derive(Debug, thiserror::Error)]
pub enum EventError {
    #[error("Event publishing failed: {0}")]
    PublishFailed(String),
    #[error("Handler registration failed: {0}")]
    HandlerRegistrationFailed(String),
    #[error("Handler not found: {0}")]
    HandlerNotFound(String),
    #[error("Event processing failed: {0}")]
    ProcessingFailed(String),
}

/// Event store trait
#[async_trait]
pub trait EventStore: Send + Sync {
    async fn append(&self, event: &DomainEvent) -> Result<(), EventError>;
    async fn get_events(&self, aggregate_id: &str, from_version: Option<u64>) -> Result<Vec<DomainEvent>, EventError>;
    async fn get_events_by_correlation(&self, correlation_id: &str) -> Result<Vec<DomainEvent>, EventError>;
}

/// In-memory event bus for development
pub struct InMemoryEventBus {
    handlers: parking_lot::RwLock<HashMap<String, Vec<Box<dyn EventHandler>>>>,
}

impl InMemoryEventBus {
    pub fn new() -> Self {
        Self {
            handlers: parking_lot::RwLock::new(HashMap::new()),
        }
    }
}

#[async_trait]
impl EventBus for InMemoryEventBus {
    async fn publish(&self, event: DomainEvent) -> Result<(), EventError> {
        let handlers = self.handlers.read();
        if let Some(event_handlers) = handlers.get(event.event_type()) {
            for handler in event_handlers {
                handler.handle(&event).await?;
            }
        }
        Ok(())
    }

    async fn subscribe(&self, event_type: &str, handler: Box<dyn EventHandler>) -> Result<(), EventError> {
        let mut handlers = self.handlers.write();
        handlers.entry(event_type.to_string()).or_default().push(handler);
        Ok(())
    }

    async fn unsubscribe(&self, event_type: &str, handler_id: &str) -> Result<(), EventError> {
        let mut handlers = self.handlers.write();
        if let Some(event_handlers) = handlers.get_mut(event_type) {
            event_handlers.retain(|h| h.handler_id() != handler_id);
        }
        Ok(())
    }
}

impl Default for InMemoryEventBus {
    fn default() -> Self {
        Self::new()
    }
}