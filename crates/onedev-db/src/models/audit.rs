//! Audit Log Database Model

use super::*;
use crate::DbResult;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, Postgres, Row};
use uuid::Uuid;

/// Audit log row from database
#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct DbAuditLog {
    pub id: i64,
    pub uuid: Uuid,
    pub user_id: Option<i64>,
    pub action: String,
    pub entity_type: String,
    pub entity_id: Option<i64>,
    pub entity_ref: Option<String>,
    pub old_value: Option<serde_json::Value>,
    pub new_value: Option<serde_json::Value>,
    pub ip_address: Option<String>,
    pub user_agent: Option<String>,
    pub request_id: Option<Uuid>, // UUID column (core's AuditEvent keeps it as a string)
    pub created_at: DateTime<Utc>,
}

/// Audit log creation input for database
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DbCreateAuditLog {
    pub user_id: Option<i64>,
    pub action: String,
    pub entity_type: String,
    pub entity_id: Option<i64>,
    pub entity_ref: Option<String>,
    pub old_value: Option<serde_json::Value>,
    pub new_value: Option<serde_json::Value>,
    pub ip_address: Option<String>,
    pub user_agent: Option<String>,
    pub request_id: Option<Uuid>,
}

/// Database query builder for audit logs
pub struct AuditLogQueryBuilder;

impl AuditLogQueryBuilder {
    pub fn select_all() -> &'static str {
        r#"
        SELECT id, uuid, user_id, action, entity_type, entity_id, entity_ref,
               old_value, new_value, ip_address, user_agent, request_id, created_at
        FROM audit_logs
        "#
    }

    pub fn select_by_id() -> &'static str {
        r#"
        SELECT id, uuid, user_id, action, entity_type, entity_id, entity_ref,
               old_value, new_value, ip_address, user_agent, request_id, created_at
        FROM audit_logs WHERE id = $1
        "#
    }

    pub fn select_by_user_id() -> &'static str {
        r#"
        SELECT id, uuid, user_id, action, entity_type, entity_id, entity_ref,
               old_value, new_value, ip_address, user_agent, request_id, created_at
        FROM audit_logs WHERE user_id = $1
        ORDER BY created_at DESC
        LIMIT $2 OFFSET $3
        "#
    }

    pub fn select_by_entity() -> &'static str {
        r#"
        SELECT id, uuid, user_id, action, entity_type, entity_id, entity_ref,
               old_value, new_value, ip_address, user_agent, request_id, created_at
        FROM audit_logs WHERE entity_type = $1 AND entity_id = $2
        ORDER BY created_at DESC
        LIMIT $3 OFFSET $4
        "#
    }

    pub fn select_by_action() -> &'static str {
        r#"
        SELECT id, uuid, user_id, action, entity_type, entity_id, entity_ref,
               old_value, new_value, ip_address, user_agent, request_id, created_at
        FROM audit_logs WHERE action = $1
        ORDER BY created_at DESC
        LIMIT $2 OFFSET $3
        "#
    }

    pub fn select_by_date_range() -> &'static str {
        r#"
        SELECT id, uuid, user_id, action, entity_type, entity_id, entity_ref,
               old_value, new_value, ip_address, user_agent, request_id, created_at
        FROM audit_logs WHERE created_at >= $1 AND created_at <= $2
        ORDER BY created_at DESC
        LIMIT $3 OFFSET $4
        "#
    }

    pub fn insert() -> &'static str {
        r#"
        INSERT INTO audit_logs (uuid, user_id, action, entity_type, entity_id, entity_ref,
                                old_value, new_value, ip_address, user_agent, request_id, created_at)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)
        RETURNING id
        "#
    }

    pub fn delete_old() -> &'static str {
        "DELETE FROM audit_logs WHERE created_at < $1"
    }
}