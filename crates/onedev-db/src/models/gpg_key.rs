//! GPG Key Database Model

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

/// GPG key row from database
#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct DbGpgKey {
    pub id: i64,
    pub uuid: Uuid,
    pub user_id: i64,
    pub key_id: String,
    pub public_key: String,
    pub fingerprint: String,
    pub email: Option<String>,
    pub expires_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// GPG key creation input for database
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DbCreateGpgKey {
    pub user_id: i64,
    pub key_id: String,
    pub public_key: String,
    pub fingerprint: String,
    pub email: Option<String>,
    pub expires_at: Option<DateTime<Utc>>,
}

/// GPG key update input for database
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DbUpdateGpgKey {
    pub expires_at: Option<DateTime<Utc>>,
}

/// Database query builder for GPG keys
pub struct GpgKeyQueryBuilder;

impl GpgKeyQueryBuilder {
    pub fn select_all() -> &'static str {
        r#"
        SELECT id, uuid, user_id, key_id, public_key, fingerprint, email,
               expires_at, created_at, updated_at
        FROM gpg_keys
        "#
    }

    pub fn select_by_id() -> &'static str {
        r#"
        SELECT id, uuid, user_id, key_id, public_key, fingerprint, email,
               expires_at, created_at, updated_at
        FROM gpg_keys WHERE id = $1
        "#
    }

    pub fn select_by_user_id() -> &'static str {
        r#"
        SELECT id, uuid, user_id, key_id, public_key, fingerprint, email,
               expires_at, created_at, updated_at
        FROM gpg_keys WHERE user_id = $1
        ORDER BY created_at DESC
        "#
    }

    pub fn select_by_key_id() -> &'static str {
        r#"
        SELECT id, uuid, user_id, key_id, public_key, fingerprint, email,
               expires_at, created_at, updated_at
        FROM gpg_keys WHERE key_id = $1
        "#
    }

    pub fn insert() -> &'static str {
        r#"
        INSERT INTO gpg_keys (uuid, user_id, key_id, public_key, fingerprint,
                              email, expires_at, created_at, updated_at)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
        RETURNING id
        "#
    }

    pub fn update() -> &'static str {
        r#"
        UPDATE gpg_keys SET
            expires_at = COALESCE($2, expires_at),
            updated_at = NOW()
        WHERE id = $1
        RETURNING id, uuid, user_id, key_id, public_key, fingerprint, email,
                  expires_at, created_at, updated_at
        "#
    }

    pub fn delete() -> &'static str {
        "DELETE FROM gpg_keys WHERE id = $1"
    }
}
