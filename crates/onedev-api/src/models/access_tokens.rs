//! Access Token API models

use serde::{Deserialize, Serialize};
use utoipa::{PartialSchema, ToSchema};
use chrono::{DateTime, Utc};
use uuid::Uuid;
use validator::Validate;
use bitflags::bitflags;

/// Access token type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum AccessTokenType {
    Personal,
    Deploy,
    Runner,
    Temporary,
    OAuth,
    ServiceAccount,
}

/// Token scopes (bitflags)
bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
    pub struct TokenScope: u64 {
        const API                  = 1 << 0;
        const READ_REPOSITORY      = 1 << 1;
        const WRITE_REPOSITORY     = 1 << 2;
        const READ_REGISTRY        = 1 << 3;
        const WRITE_REGISTRY       = 1 << 4;
        const READ_PACKAGE         = 1 << 5;
        const WRITE_PACKAGE        = 1 << 6;
        const READ_USER            = 1 << 7;
        const WRITE_USER           = 1 << 8;
        const ADMIN_USER           = 1 << 9;
        const ADMIN_PROJECT        = 1 << 10;
        const ADMIN_GROUP          = 1 << 11;
        const ADMIN_SYSTEM         = 1 << 12;
        const RUN_BUILDS           = 1 << 13;
        const MANAGE_BUILDS        = 1 << 14;
        const MANAGE_RUNNERS       = 1 << 15;
        const MANAGE_WEBHOOKS      = 1 << 16;
        const MANAGE_HOOKS         = 1 << 17;
        const READ_AUDIT_LOG       = 1 << 18;
        const MANAGE_CLUSTER       = 1 << 19;
        const MANAGE_KUBERNETES    = 1 << 20;
        const SUDO                 = 1 << 21;
    }
}

impl utoipa::PartialSchema for TokenScope {
    fn schema() -> utoipa::openapi::RefOr<utoipa::openapi::schema::Schema> {
        u64::schema()
    }
}

impl utoipa::ToSchema for TokenScope {}

/// Access token with raw token (only on creation)
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct AccessTokenWithRawResponse {
    #[serde(flatten)]
    pub token: super::users::AccessTokenResponse,
    pub raw_token: String,
}

/// Create access token request
#[derive(Debug, Clone, Deserialize, Validate, ToSchema)]
pub struct CreateAccessTokenRequest {
    #[validate(length(min = 1, max = 100))]
    #[schema(min_length = 1, max_length = 100)]
    pub name: String,

    #[schema(default = "personal")]
    pub token_type: AccessTokenType,

    pub scopes: TokenScope,

    pub expires_at: Option<DateTime<Utc>>,

    pub project_id: Option<i64>,
    pub group_id: Option<i64>,
    pub impersonated_user_id: Option<i64>,
}

/// Update access token request
#[derive(Debug, Clone, Deserialize, Validate, ToSchema, Default)]
pub struct UpdateAccessTokenRequest {
    #[validate(length(min = 1, max = 100))]
    #[schema(min_length = 1, max_length = 100)]
    pub name: Option<String>,

    pub scopes: Option<TokenScope>,
    pub expires_at: Option<Option<DateTime<Utc>>>,
    pub revoked: Option<bool>,
}

/// Access token query parameters
#[derive(Debug, Clone, Deserialize, Serialize, ToSchema, Default)]
pub struct AccessTokenQuery {
    pub token_type: Option<AccessTokenType>,
    pub owner_id: Option<i64>,
    pub project_id: Option<i64>,
    pub group_id: Option<i64>,
    pub revoked: Option<bool>,
    pub expired: Option<bool>,
    #[serde(flatten)]
    pub pagination: super::pagination::OffsetPagination,
    pub sort: Option<String>,
    pub order: Option<super::pagination::SortOrder>,
}

/// Token validation request
#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct ValidateTokenRequest {
    pub token: String,
}

/// Token validation response
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct TokenValidationResponse {
    pub valid: bool,
    pub token: Option<super::users::AccessTokenResponse>,
    pub user: Option<super::users::UserRef>,
    pub scopes: TokenScope,
    pub error: Option<String>,
}