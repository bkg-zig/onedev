//! SSO Account Database Model

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

/// SSO account row from database
#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct DbSsoAccount {
    pub id: i64,
    pub uuid: Uuid,
    pub user_id: i64,
    pub provider: String,
    pub provider_user_id: String,
    pub provider_username: Option<String>,
    pub provider_email: Option<String>,
    pub access_token: Option<String>,
    pub refresh_token: Option<String>,
    pub token_expires_at: Option<DateTime<Utc>>,
    pub raw_data: Option<serde_json::Value>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// SSO account creation input for database
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DbCreateSsoAccount {
    pub user_id: i64,
    pub provider: String,
    pub provider_user_id: String,
    pub provider_username: Option<String>,
    pub provider_email: Option<String>,
    pub access_token: Option<String>,
    pub refresh_token: Option<String>,
    pub token_expires_at: Option<DateTime<Utc>>,
    pub raw_data: Option<serde_json::Value>,
}

/// SSO account update input for database
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DbUpdateSsoAccount {
    pub provider_username: Option<String>,
    pub provider_email: Option<String>,
    pub access_token: Option<String>,
    pub refresh_token: Option<String>,
    pub token_expires_at: Option<DateTime<Utc>>,
    pub raw_data: Option<serde_json::Value>,
}

/// Database query builder for SSO accounts
pub struct SsoAccountQueryBuilder;

impl SsoAccountQueryBuilder {
    pub fn select_all() -> &'static str {
        r#"
        SELECT id, uuid, user_id, provider, provider_user_id, provider_username,
               provider_email, access_token, refresh_token, token_expires_at,
               raw_data, created_at, updated_at
        FROM sso_accounts
        "#
    }

    pub fn select_by_id() -> &'static str {
        r#"
        SELECT id, uuid, user_id, provider, provider_user_id, provider_username,
               provider_email, access_token, refresh_token, token_expires_at,
               raw_data, created_at, updated_at
        FROM sso_accounts WHERE id = $1
        "#
    }

    pub fn select_by_user_id() -> &'static str {
        r#"
        SELECT id, uuid, user_id, provider, provider_user_id, provider_username,
               provider_email, access_token, refresh_token, token_expires_at,
               raw_data, created_at, updated_at
        FROM sso_accounts WHERE user_id = $1
        ORDER BY provider
        "#
    }

    pub fn select_by_provider_and_user_id() -> &'static str {
        r#"
        SELECT id, uuid, user_id, provider, provider_user_id, provider_username,
               provider_email, access_token, refresh_token, token_expires_at,
               raw_data, created_at, updated_at
        FROM sso_accounts WHERE provider = $1 AND provider_user_id = $2
        "#
    }

    pub fn insert() -> &'static str {
        r#"
        INSERT INTO sso_accounts (uuid, user_id, provider, provider_user_id,
                                  provider_username, provider_email, access_token,
                                  refresh_token, token_expires_at, raw_data,
                                  created_at, updated_at)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)
        RETURNING id
        "#
    }

    pub fn update() -> &'static str {
        r#"
        UPDATE sso_accounts SET
            provider_username = COALESCE($2, provider_username),
            provider_email = COALESCE($3, provider_email),
            access_token = COALESCE($4, access_token),
            refresh_token = COALESCE($5, refresh_token),
            token_expires_at = COALESCE($6, token_expires_at),
            raw_data = COALESCE($7, raw_data),
            updated_at = NOW()
        WHERE id = $1
        RETURNING id, uuid, user_id, provider, provider_user_id, provider_username,
                  provider_email, access_token, refresh_token, token_expires_at,
                  raw_data, created_at, updated_at
        "#
    }

    pub fn delete() -> &'static str {
        "DELETE FROM sso_accounts WHERE id = $1"
    }
}
