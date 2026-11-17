//! User API models

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use chrono::{DateTime, Utc};
use uuid::Uuid;
use validator::Validate;

/// User reference (lightweight)
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct UserRef {
    pub id: i64,
    pub uuid: Uuid,
    pub name: String,
    pub full_name: Option<String>,
    pub avatar_url: Option<String>,
    pub user_type: UserType,
}

/// User type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum UserType {
    Ordinary,
    Service,
    Ai,
}

/// User status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum UserStatus {
    Active,
    Disabled,
    PendingVerification,
    Locked,
}

/// Full user response
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct UserResponse {
    pub id: i64,
    pub uuid: Uuid,
    pub name: String,
    pub full_name: Option<String>,
    pub email: Option<String>,
    pub user_type: UserType,
    pub status: UserStatus,
    pub avatar_url: Option<String>,
    pub two_factor_enabled: bool,
    pub last_login_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub notify_own_events: bool,
    pub keep_emails_private: bool,
    pub timezone: String,
    pub locale: String,
}

/// User list item (for lists)
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct UserListItem {
    #[serde(flatten)]
    pub user: UserRef,
    pub status: UserStatus,
    pub last_login_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

/// Create user request
#[derive(Debug, Clone, Deserialize, Validate, ToSchema)]
pub struct CreateUserRequest {
    #[validate(length(min = 3, max = 100))]
    #[schema(min_length = 3, max_length = 100)]
    pub name: String,

    #[validate(length(max = 200))]
    #[schema(max_length = 200)]
    pub full_name: Option<String>,

    #[validate(email)]
    #[schema(format = "email")]
    pub email: Option<String>,

    #[validate(length(min = 8, max = 128))]
    #[schema(min_length = 8, max_length = 128)]
    pub password: Option<String>,

    #[schema(default = "ordinary")]
    pub user_type: UserType,

    pub send_invite_email: Option<bool>,
}

/// Update user request
#[derive(Debug, Clone, Deserialize, Validate, ToSchema, Default)]
pub struct UpdateUserRequest {
    #[validate(length(max = 200))]
    #[schema(max_length = 200)]
    pub full_name: Option<String>,

    #[validate(email)]
    #[schema(format = "email")]
    pub email: Option<String>,

    #[schema(format = "uri")]
    pub avatar_url: Option<String>,

    pub notify_own_events: Option<bool>,
    pub keep_emails_private: Option<bool>,
    pub timezone: Option<String>,
    pub locale: Option<String>,
}

/// Change password request
#[derive(Debug, Clone, Deserialize, Validate, ToSchema)]
pub struct ChangePasswordRequest {
    #[validate(length(min = 8, max = 128))]
    #[schema(min_length = 8, max_length = 128)]
    pub current_password: String,

    #[validate(length(min = 8, max = 128))]
    #[schema(min_length = 8, max_length = 128)]
    pub new_password: String,
}

/// User query parameters
#[derive(Debug, Clone, Deserialize, Serialize, ToSchema, Default)]
pub struct UserQuery {
    pub term: Option<String>,
    pub user_type: Option<UserType>,
    pub status: Option<UserStatus>,
    #[serde(flatten)]
    pub pagination: super::pagination::OffsetPagination,
}

/// User with email addresses
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct UserWithEmails {
    #[serde(flatten)]
    pub user: UserResponse,
    pub emails: Vec<EmailResponse>,
}

/// Email response
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct EmailResponse {
    pub id: i64,
    pub uuid: Uuid,
    pub email: String,
    pub verified: bool,
    pub primary: bool,
    pub created_at: DateTime<Utc>,
}

/// SSH key response
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct SshKeyResponse {
    pub id: i64,
    pub uuid: Uuid,
    pub title: String,
    pub key_type: String,
    pub fingerprint: String,
    pub public_key: String,
    pub last_used_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub expires_at: Option<DateTime<Utc>>,
}

/// GPG key response
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct GpgKeyResponse {
    pub id: i64,
    pub uuid: Uuid,
    pub key_id: String,
    pub fingerprint: String,
    pub public_key: String,
    pub verified: bool,
    pub created_at: DateTime<Utc>,
    pub expires_at: Option<DateTime<Utc>>,
}

/// Access token response (without raw token)
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct AccessTokenResponse {
    pub id: i64,
    pub uuid: Uuid,
    pub name: String,
    pub token_prefix: String,
    pub token_type: super::access_tokens::AccessTokenType,
    pub scopes: super::access_tokens::TokenScope,
    pub owner_id: i64,
    pub impersonated_user_id: Option<i64>,
    pub project_id: Option<i64>,
    pub group_id: Option<i64>,
    pub expires_at: Option<DateTime<Utc>>,
    pub last_used_at: Option<DateTime<Utc>>,
    pub last_used_ip: Option<String>,
    pub revoked: bool,
    pub revoked_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}