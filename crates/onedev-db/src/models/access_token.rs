//! Access Token Database Model

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

/// Access token row from database
#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct DbAccessToken {
    pub id: i64,
    pub uuid: Uuid,
    pub user_id: i64,
    pub name: String,
    pub token_hash: String, // SHA256 hash of the token
    pub scopes: Vec<String>,
    pub expires_at: Option<DateTime<Utc>>,
    pub last_used_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub revoked_at: Option<DateTime<Utc>>,
}

/// Access token creation input for database
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DbCreateAccessToken {
    pub user_id: i64,
    pub name: String,
    pub token_hash: String,
    pub scopes: Vec<String>,
    pub expires_at: Option<DateTime<Utc>>,
}

/// Access token update input for database
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DbUpdateAccessToken {
    pub name: Option<String>,
    pub scopes: Option<Vec<String>>,
    pub expires_at: Option<DateTime<Utc>>,
    pub revoked_at: Option<DateTime<Utc>>,
}

impl DbAccessToken {
    /// Check if token is expired
    pub fn is_expired(&self) -> bool {
        self.expires_at.is_some_and(|exp| exp < Utc::now())
    }

    /// Check if token is revoked
    pub fn is_revoked(&self) -> bool {
        self.revoked_at.is_some()
    }

    /// Check if token is active
    pub fn is_active(&self) -> bool {
        !self.is_expired() && !self.is_revoked()
    }
}

/// Database query builder for access tokens
pub struct AccessTokenQueryBuilder;

impl AccessTokenQueryBuilder {
    pub fn select_all() -> &'static str {
        r#"
        SELECT id, uuid, user_id, name, token_hash, scopes, expires_at,
               last_used_at, created_at, updated_at, revoked_at
        FROM access_tokens
        "#
    }

    pub fn select_by_id() -> &'static str {
        r#"
        SELECT id, uuid, user_id, name, token_hash, scopes, expires_at,
               last_used_at, created_at, updated_at, revoked_at
        FROM access_tokens WHERE id = $1
        "#
    }

    pub fn select_by_token_hash() -> &'static str {
        r#"
        SELECT id, uuid, user_id, name, token_hash, scopes, expires_at,
               last_used_at, created_at, updated_at, revoked_at
        FROM access_tokens WHERE token_hash = $1
        "#
    }

    pub fn select_by_user_id() -> &'static str {
        r#"
        SELECT id, uuid, user_id, name, token_hash, scopes, expires_at,
               last_used_at, created_at, updated_at, revoked_at
        FROM access_tokens WHERE user_id = $1 AND revoked_at IS NULL
        ORDER BY created_at DESC
        "#
    }

    pub fn insert() -> &'static str {
        r#"
        INSERT INTO access_tokens (uuid, user_id, name, token_hash, scopes,
                                   expires_at, created_at, updated_at)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
        RETURNING id
        "#
    }

    pub fn update() -> &'static str {
        r#"
        UPDATE access_tokens SET
            name = COALESCE($2, name),
            scopes = COALESCE($3, scopes),
            expires_at = COALESCE($4, expires_at),
            revoked_at = COALESCE($5, revoked_at),
            updated_at = NOW()
        WHERE id = $1
        RETURNING id, uuid, user_id, name, token_hash, scopes, expires_at,
                  last_used_at, created_at, updated_at, revoked_at
        "#
    }

    pub fn update_last_used() -> &'static str {
        "UPDATE access_tokens SET last_used_at = NOW(), updated_at = NOW() WHERE id = $1"
    }

    pub fn delete() -> &'static str {
        "UPDATE access_tokens SET revoked_at = NOW(), updated_at = NOW() WHERE id = $1"
    }
}
