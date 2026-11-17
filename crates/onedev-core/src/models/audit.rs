//! Audit Log Domain Model

use super::*;
use crate::utils::*;

/// Audit event entity
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AuditEvent {
    pub id: i64,
    pub uuid: Uuid,
    pub entity_type: EntityType,
    pub entity_id: i64,
    pub entity_uuid: Uuid,
    pub action: AuditAction,
    pub actor_id: Option<i64>, // None for system actions
    pub actor_type: ActorType,
    pub ip_address: Option<String>,
    pub user_agent: Option<String>,
    pub request_id: Option<String>,
    // Changes
    pub old_values: Option<serde_json::Value>,
    pub new_values: Option<serde_json::Value>,
    // Context
    pub project_id: Option<i64>,
    pub group_id: Option<i64>,
    pub metadata: serde_json::Value,
    // Timestamp
    pub created_at: DateTime<Utc>,
}

/// Entity types for audit
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, strum::EnumString, strum::Display)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case")]
pub enum EntityType {
    User,
    Group,
    Project,
    Issue,
    MergeRequest,
    Build,
    Pipeline,
    Package,
    Release,
    Workspace,
    Runner,
    Webhook,
    DeployKey,
    ProtectedBranch,
    ProtectedTag,
    Setting,
    Role,
    AccessToken,
    SshKey,
    GpgKey,
    EmailAddress,
    OAuthApplication,
    OAuthProvider,
    SsoAccount,
    WikiPage,
    Snippet,
    Milestone,
    Label,
    Board,
    Environment,
    Cluster,
    KubernetesNamespace,
    FeatureFlag,
    Secret,
    Variable,
    RunnerRegistrationToken,
    DeployToken,
    PersonalAccessToken,
    MirrorRepository,
    ImportExport,
    Backup,
    Restore,
    Upgrade,
    Custom(String),
}

/// Audit actions
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, strum::EnumString, strum::Display)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case")]
pub enum AuditAction {
    Created,
    Updated,
    Deleted,
    Restored,
    Archived,
    Unarchived,
    Enabled,
    Disabled,
    Locked,
    Unlocked,
    Assigned,
    Unassigned,
    Mentioned,
    Labeled,
    Unlabeled,
    Milestoned,
    Unmilestoned,
    Merged,
    Closed,
    Reopened,
    Approved,
    Rejected,
    Commented,
    Pushed,
    ForcePushed,
    Tagged,
    Untagged,
    Branched,
    MergedBranch,
    DeletedBranch,
    Protected,
    Unprotected,
    Deployed,
    RolledBack,
    Started,
    Stopped,
    Canceled,
    Retried,
    Scheduled,
    Unsuspended,
    Invited,
    Joined,
    Left,
    Removed,
    Promoted,
    Demoted,
    Transferred,
    Imported,
    Exported,
    BackedUp,
    Upgraded,
    Configured,
    LoggedIn,
    LoggedOut,
    FailedLogin,
    PasswordChanged,
    TwoFactorEnabled,
    TwoFactorDisabled,
    AccessTokenCreated,
    AccessTokenRevoked,
    SshKeyAdded,
    SshKeyRemoved,
    GpgKeyAdded,
    GpgKeyRemoved,
    EmailAdded,
    EmailVerified,
    EmailRemoved,
    Custom(String),
}

/// Actor type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, strum::EnumString, strum::Display)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case")]
pub enum ActorType {
    User,
    System,
    DeployKey,
    AccessToken,
    OAuthApplication,
    Webhook,
    Scheduler,
    Migration,
    Unknown,
}

/// Audit query params
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AuditQuery {
    pub entity_type: Option<EntityType>,
    pub entity_id: Option<i64>,
    pub action: Option<AuditAction>,
    pub actor_id: Option<i64>,
    pub actor_type: Option<ActorType>,
    pub project_id: Option<i64>,
    pub group_id: Option<i64>,
    pub from_date: Option<DateTime<Utc>>,
    pub to_date: Option<DateTime<Utc>>,
    pub ip_address: Option<String>,
    pub sort: Option<String>,
    pub order: Option<SortOrder>,
    pub page: u32,
    pub per_page: u32,
}

/// Audit event reference (for lists)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AuditEventRef {
    pub id: i64,
    pub uuid: Uuid,
    pub entity_type: EntityType,
    pub entity_id: i64,
    pub action: AuditAction,
    pub actor_type: ActorType,
    pub actor_name: Option<String>,
    pub created_at: DateTime<Utc>,
}

impl AuditEvent {
    pub fn is_system_action(&self) -> bool {
        self.actor_type == ActorType::System
    }

    pub fn is_user_action(&self) -> bool {
        self.actor_type == ActorType::User
    }

    pub fn is_security_relevant(&self) -> bool {
        matches!(
            self.action,
            AuditAction::LoggedIn
                | AuditAction::FailedLogin
                | AuditAction::PasswordChanged
                | AuditAction::TwoFactorEnabled
                | AuditAction::TwoFactorDisabled
                | AuditAction::AccessTokenCreated
                | AuditAction::AccessTokenRevoked
                | AuditAction::SshKeyAdded
                | AuditAction::SshKeyRemoved
                | AuditAction::GpgKeyAdded
                | AuditAction::GpgKeyRemoved
        )
    }
}