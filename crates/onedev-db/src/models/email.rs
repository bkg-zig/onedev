//! Email Address Database Model

use super::*;
use crate::DbResult;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, Postgres, Row};
use uuid::Uuid;

/// Email address row from database
#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct DbEmailAddress {
    pub id: i64,
    pub uuid: Uuid,
    pub user_id: i64,
    pub value: String,
    pub verified: bool,
    pub primary: bool,
    pub verification_code: Option<String>,
    pub verification_sent_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Email address creation input for database
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DbCreateEmailAddress {
    pub user_id: i64,
    pub value: String,
    pub primary: bool,
    pub verification_code: Option<String>,
}

/// Email address update input for database
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DbUpdateEmailAddress {
    pub verified: Option<bool>,
    pub primary: Option<bool>,
    pub verification_code: Option<String>,
    pub verification_sent_at: Option<DateTime<Utc>>,
}

impl DbEmailAddress {
    /// Check if email is verified
    pub fn is_verified(&self) -> bool {
        self.verified
    }
}

/// Database query builder for email addresses
pub struct EmailAddressQueryBuilder;

impl EmailAddressQueryBuilder {
    pub fn select_all() -> &'static str {
        r#"
        SELECT id, uuid, user_id, value, verified, primary, verification_code,
               verification_sent_at, created_at, updated_at
        FROM email_addresses
        "#
    }

    pub fn select_by_id() -> &'static str {
        r#"
        SELECT id, uuid, user_id, value, verified, primary, verification_code,
               verification_sent_at, created_at, updated_at
        FROM email_addresses WHERE id = $1
        "#
    }

    pub fn select_by_user_id() -> &'static str {
        r#"
        SELECT id, uuid, user_id, value, verified, primary, verification_code,
               verification_sent_at, created_at, updated_at
        FROM email_addresses WHERE user_id = $1
        ORDER BY primary DESC, created_at ASC
        "#
    }

    pub fn select_by_value() -> &'static str {
        r#"
        SELECT id, uuid, user_id, value, verified, primary, verification_code,
               verification_sent_at, created_at, updated_at
        FROM email_addresses WHERE value = $1
        "#
    }

    pub fn select_primary_by_user_id() -> &'static str {
        r#"
        SELECT id, uuid, user_id, value, verified, primary, verification_code,
               verification_sent_at, created_at, updated_at
        FROM email_addresses WHERE user_id = $1 AND primary = true
        LIMIT 1
        "#
    }

    pub fn insert() -> &'static str {
        r#"
        INSERT INTO email_addresses (uuid, user_id, value, verified, primary,
                                     verification_code, verification_sent_at, created_at, updated_at)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
        RETURNING id
        "#
    }

    pub fn update() -> &'static str {
        r#"
        UPDATE email_addresses SET
            verified = COALESCE($2, verified),
            primary = COALESCE($3, primary),
            verification_code = COALESCE($4, verification_code),
            verification_sent_at = COALESCE($5, verification_sent_at),
            updated_at = NOW()
        WHERE id = $1
        RETURNING id, uuid, user_id, value, verified, primary, verification_code,
                  verification_sent_at, created_at, updated_at
        "#
    }

    pub fn delete() -> &'static str {
        "DELETE FROM email_addresses WHERE id = $1"
    }
}