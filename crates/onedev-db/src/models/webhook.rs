//! Webhook Database Model

use super::*;
use crate::DbResult;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, Postgres, Row};
use uuid::Uuid;

/// Webhook row from database
#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct DbWebhook {
    pub id: i64,
    pub uuid: Uuid,
    pub project_id: Option<i64>, // None for system webhooks
    pub url: String,
    pub secret: Option<String>,
    /// Subscribed event names, stored in the JSONB `scopes` column
    #[sqlx(rename = "scopes", json)]
    pub events: Vec<String>,
    pub branch_filter: Option<String>,
    pub tag_filter: Option<String>,
    pub ssl_verification: bool,
    pub active: bool,
    pub last_delivery_at: Option<DateTime<Utc>>,
    pub last_delivery_status: Option<i32>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Webhook creation input for database
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DbCreateWebhook {
    pub project_id: Option<i64>,
    pub url: String,
    pub secret: Option<String>,
    pub events: Vec<String>,
    pub branch_filter: Option<String>,
    pub tag_filter: Option<String>,
    pub ssl_verification: bool,
}

/// Webhook update input for database
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DbUpdateWebhook {
    pub url: Option<String>,
    pub secret: Option<String>,
    pub events: Option<Vec<String>>,
    pub branch_filter: Option<String>,
    pub tag_filter: Option<String>,
    pub ssl_verification: Option<bool>,
    pub active: Option<bool>,
}

/// Database query builder for webhooks
pub struct WebhookQueryBuilder;

impl WebhookQueryBuilder {
    pub fn select_all() -> &'static str {
        r#"
        SELECT id, uuid, project_id, url, secret, scopes, branch_filter,
               tag_filter, ssl_verification, active, last_delivery_at,
               last_delivery_status, created_at, updated_at
        FROM webhooks
        "#
    }

    pub fn select_by_id() -> &'static str {
        r#"
        SELECT id, uuid, project_id, url, secret, scopes, branch_filter,
               tag_filter, ssl_verification, active, last_delivery_at,
               last_delivery_status, created_at, updated_at
        FROM webhooks WHERE id = $1
        "#
    }

    pub fn select_by_project_id() -> &'static str {
        r#"
        SELECT id, uuid, project_id, url, secret, scopes, branch_filter,
               tag_filter, ssl_verification, active, last_delivery_at,
               last_delivery_status, created_at, updated_at
        FROM webhooks WHERE project_id = $1
        ORDER BY created_at DESC
        "#
    }

    pub fn select_system_webhooks() -> &'static str {
        r#"
        SELECT id, uuid, project_id, url, secret, scopes, branch_filter,
               tag_filter, ssl_verification, active, last_delivery_at,
               last_delivery_status, created_at, updated_at
        FROM webhooks WHERE project_id IS NULL
        ORDER BY created_at DESC
        "#
    }

    pub fn insert() -> &'static str {
        r#"
        INSERT INTO webhooks (uuid, project_id, url, secret, scopes,
                              branch_filter, tag_filter, ssl_verification,
                              active, created_at, updated_at)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
        RETURNING id
        "#
    }

    pub fn update() -> &'static str {
        r#"
        UPDATE webhooks SET
            url = COALESCE($2, url),
            secret = COALESCE($3, secret),
            scopes = COALESCE($4, scopes),
            branch_filter = COALESCE($5, branch_filter),
            tag_filter = COALESCE($6, tag_filter),
            ssl_verification = COALESCE($7, ssl_verification),
            active = COALESCE($8, active),
            updated_at = NOW()
        WHERE id = $1
        RETURNING id, uuid, project_id, url, secret, scopes, branch_filter,
                  tag_filter, ssl_verification, active, last_delivery_at,
                  last_delivery_status, created_at, updated_at
        "#
    }

    pub fn update_delivery_status() -> &'static str {
        r#"
        UPDATE webhooks SET
            last_delivery_at = NOW(),
            last_delivery_status = $2,
            updated_at = NOW()
        WHERE id = $1
        "#
    }

    pub fn delete() -> &'static str {
        "DELETE FROM webhooks WHERE id = $1"
    }
}