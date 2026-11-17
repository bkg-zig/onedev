//! Email Domain Model

use super::*;
use crate::utils::*;

/// Email address verification status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, strum::EnumString, strum::Display)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case")]
pub enum EmailVerificationStatus {
    Unverified,
    Pending,
    Verified,
    Failed,
}

/// Email address entity
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EmailAddress {
    pub id: i64,
    pub uuid: Uuid,
    pub user_id: i64,
    pub email: String,
    pub is_primary: bool,
    pub is_verified: bool,
    pub verification_status: EmailVerificationStatus,
    pub verification_token: Option<String>, // Hashed
    pub verification_token_expires_at: Option<DateTime<Utc>>,
    pub verified_at: Option<DateTime<Utc>>,
    pub bounce_count: u32,
    pub last_bounce_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Email address creation input
#[derive(Debug, Clone, Serialize, Deserialize, validator::Validate)]
pub struct CreateEmailAddress {
    #[validate(email)]
    pub email: String,
}

/// Email verification request
#[derive(Debug, Clone, Serialize, Deserialize, validator::Validate)]
pub struct VerifyEmailAddress {
    pub token: String,
}

/// Email verification response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmailVerificationResult {
    pub success: bool,
    pub email: Option<EmailAddress>,
    pub error: Option<String>,
}

/// Email notification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmailNotification {
    pub id: i64,
    pub uuid: Uuid,
    pub recipient_id: i64,
    pub notification_type: NotificationType,
    pub subject: String,
    pub body_text: String,
    pub body_html: Option<String>,
    pub status: EmailNotificationStatus,
    pub sent_at: Option<DateTime<Utc>>,
    pub delivered_at: Option<DateTime<Utc>>,
    pub opened_at: Option<DateTime<Utc>>,
    pub clicked_at: Option<DateTime<Utc>>,
    pub error: Option<String>,
    pub retry_count: u32,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, strum::EnumString, strum::Display)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case")]
pub enum NotificationType {
    IssueCreated,
    IssueAssigned,
    IssueMentioned,
    IssueCommented,
    IssueClosed,
    MergeRequestCreated,
    MergeRequestAssigned,
    MergeRequestMentioned,
    MergeRequestCommented,
    MergeRequestMerged,
    MergeRequestClosed,
    BuildFailed,
    BuildSuccess,
    BuildCanceled,
    PipelineFailed,
    PipelineSuccess,
    Mention,
    Todo,
    Milestone,
    Release,
    SecurityAlert,
    DailySummary,
    WeeklySummary,
    PasswordReset,
    EmailVerification,
    TwoFactorBackupCodes,
    AccessTokenCreated,
    AccessTokenExpiring,
    AccessTokenRevoked,
    GroupInvitation,
    ProjectInvitation,
    AccessRequest,
    AccessGranted,
    AccessDenied,
    SystemAnnouncement,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, strum::EnumString, strum::Display)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case")]
pub enum EmailNotificationStatus {
    Pending,
    Queued,
    Sending,
    Sent,
    Delivered,
    Opened,
    Clicked,
    Bounced,
    Failed,
    Suppressed,
}

/// Email template
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmailTemplate {
    pub id: i64,
    pub uuid: Uuid,
    pub name: String,
    pub notification_type: NotificationType,
    pub subject_template: String,
    pub body_text_template: String,
    pub body_html_template: Option<String>,
    pub language: String,
    pub is_system: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Email queue item
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmailQueueItem {
    pub id: i64,
    pub uuid: Uuid,
    pub to: Vec<String>,
    pub cc: Vec<String>,
    pub bcc: Vec<String>,
    pub subject: String,
    pub body_text: String,
    pub body_html: Option<String>,
    pub headers: HashMap<String, String>,
    pub priority: EmailPriority,
    pub scheduled_at: Option<DateTime<Utc>>,
    pub attempts: u32,
    pub max_attempts: u32,
    pub last_error: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, strum::EnumString, strum::Display)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case")]
pub enum EmailPriority {
    Low,
    Normal,
    High,
    Critical,
}

impl EmailAddress {
    pub fn is_verified(&self) -> bool {
        self.is_verified && self.verification_status == EmailVerificationStatus::Verified
    }

    pub fn can_receive_email(&self) -> bool {
        self.is_verified() && self.bounce_count < 5
    }
}