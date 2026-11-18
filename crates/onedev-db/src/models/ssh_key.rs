//! SSH Key Database Model

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

/// SSH key row from database
#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct DbSshKey {
    pub id: i64,
    pub uuid: Uuid,
    pub user_id: i64,
    pub title: String,
    pub public_key: String,
    pub fingerprint_sha256: String,
    pub fingerprint_md5: String,
    pub key_type: DbSshKeyType,
    pub bits: i32,
    pub last_used_at: Option<DateTime<Utc>>,
    pub expires_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// SSH key type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "ssh_key_type", rename_all = "lowercase")]
pub enum DbSshKeyType {
    Rsa,
    Dsa,
    Ecdsa,
    Ed25519,
}

/// SSH key creation input for database
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DbCreateSshKey {
    pub user_id: i64,
    pub title: String,
    pub public_key: String,
    pub fingerprint_sha256: String,
    pub fingerprint_md5: String,
    pub key_type: DbSshKeyType,
    pub bits: i32,
    pub expires_at: Option<DateTime<Utc>>,
}

/// SSH key update input for database
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DbUpdateSshKey {
    pub title: Option<String>,
    pub expires_at: Option<DateTime<Utc>>,
}

/// Database query builder for SSH keys
pub struct SshKeyQueryBuilder;

impl SshKeyQueryBuilder {
    pub fn select_all() -> &'static str {
        r#"
        SELECT id, uuid, user_id, title, public_key, fingerprint_sha256,
               fingerprint_md5, key_type, bits, last_used_at, expires_at,
               created_at, updated_at
        FROM ssh_keys
        "#
    }

    pub fn select_by_id() -> &'static str {
        r#"
        SELECT id, uuid, user_id, title, public_key, fingerprint_sha256,
               fingerprint_md5, key_type, bits, last_used_at, expires_at,
               created_at, updated_at
        FROM ssh_keys WHERE id = $1
        "#
    }

    pub fn select_by_user_id() -> &'static str {
        r#"
        SELECT id, uuid, user_id, title, public_key, fingerprint_sha256,
               fingerprint_md5, key_type, bits, last_used_at, expires_at,
               created_at, updated_at
        FROM ssh_keys WHERE user_id = $1
        ORDER BY created_at DESC
        "#
    }

    pub fn select_by_fingerprint() -> &'static str {
        r#"
        SELECT id, uuid, user_id, title, public_key, fingerprint_sha256,
               fingerprint_md5, key_type, bits, last_used_at, expires_at,
               created_at, updated_at
        FROM ssh_keys WHERE fingerprint_sha256 = $1
        "#
    }

    pub fn insert() -> &'static str {
        r#"
        INSERT INTO ssh_keys (uuid, user_id, title, public_key, fingerprint_sha256,
                              fingerprint_md5, key_type, bits, expires_at, created_at, updated_at)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
        RETURNING id
        "#
    }

    pub fn update() -> &'static str {
        r#"
        UPDATE ssh_keys SET
            title = COALESCE($2, title),
            expires_at = COALESCE($3, expires_at),
            updated_at = NOW()
        WHERE id = $1
        RETURNING id, uuid, user_id, title, public_key, fingerprint_sha256,
                  fingerprint_md5, key_type, bits, last_used_at, expires_at,
                  created_at, updated_at
        "#
    }

    pub fn update_last_used() -> &'static str {
        "UPDATE ssh_keys SET last_used_at = NOW(), updated_at = NOW() WHERE id = $1"
    }

    pub fn delete() -> &'static str {
        "DELETE FROM ssh_keys WHERE id = $1"
    }
}
