//! Agent Database Model

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

/// Agent row from database
#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct DbAgent {
    pub id: i64,
    pub uuid: Uuid,
    pub name: String,
    pub token_hash: String,
    pub status: DbAgentStatus,
    pub executor_type: DbExecutorType,
    pub executor_config: Option<serde_json::Value>,
    /// Stored as JSONB array in the `labels` column
    pub labels: sqlx::types::Json<Vec<String>>,
    pub max_concurrent_jobs: i32,
    pub current_jobs: i32,
    pub last_seen_at: Option<DateTime<Utc>>,
    pub last_used_at: Option<DateTime<Utc>>,
    pub version: Option<String>,
    pub os: Option<String>,
    pub arch: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
}

/// Agent status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "agent_status", rename_all = "lowercase")]
pub enum DbAgentStatus {
    Offline,
    Online,
    Busy,
    Disabled,
}

/// Executor type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "executor_type", rename_all = "lowercase")]
pub enum DbExecutorType {
    Local,
    Docker,
    Kubernetes,
    Sandbox,
}

/// Agent creation input for database
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DbCreateAgent {
    pub name: String,
    pub token_hash: String,
    pub executor_type: DbExecutorType,
    pub executor_config: Option<serde_json::Value>,
    pub labels: Vec<String>,
    pub max_concurrent_jobs: i32,
}

/// Agent update input for database
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DbUpdateAgent {
    pub name: Option<String>,
    pub status: Option<DbAgentStatus>,
    pub executor_config: Option<serde_json::Value>,
    pub labels: Option<Vec<String>>,
    pub max_concurrent_jobs: Option<i32>,
    pub current_jobs: Option<i32>,
    pub last_seen_at: Option<DateTime<Utc>>,
    pub last_used_at: Option<DateTime<Utc>>,
    pub version: Option<String>,
    pub os: Option<String>,
    pub arch: Option<String>,
}

/// Database query builder for agents
pub struct AgentQueryBuilder;

impl AgentQueryBuilder {
    pub fn select_all() -> &'static str {
        r#"
        SELECT id, uuid, name, token_hash, status, executor_type, executor_config,
               labels, max_concurrent_jobs, current_jobs, last_seen_at, last_used_at,
               version, os, arch, created_at, updated_at, deleted_at
        FROM agents
        "#
    }

    pub fn select_by_id() -> &'static str {
        r#"
        SELECT id, uuid, name, token_hash, status, executor_type, executor_config,
               labels, max_concurrent_jobs, current_jobs, last_seen_at, last_used_at,
               version, os, arch, created_at, updated_at, deleted_at
        FROM agents WHERE id = $1
        "#
    }

    pub fn select_by_token_hash() -> &'static str {
        r#"
        SELECT id, uuid, name, token_hash, status, executor_type, executor_config,
               labels, max_concurrent_jobs, current_jobs, last_seen_at, last_used_at,
               version, os, arch, created_at, updated_at, deleted_at
        FROM agents WHERE token_hash = $1
        "#
    }

    pub fn select_online() -> &'static str {
        r#"
        SELECT id, uuid, name, token_hash, status, executor_type, executor_config,
               labels, max_concurrent_jobs, current_jobs, last_seen_at, last_used_at,
               version, os, arch, created_at, updated_at, deleted_at
        FROM agents WHERE status = 'online' AND current_jobs < max_concurrent_jobs
        ORDER BY current_jobs ASC
        "#
    }

    pub fn insert() -> &'static str {
        r#"
        INSERT INTO agents (uuid, name, token_hash, status, executor_type, executor_config,
                            labels, max_concurrent_jobs, current_jobs, created_at, updated_at)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
        RETURNING id
        "#
    }

    pub fn update() -> &'static str {
        r#"
        UPDATE agents SET
            name = COALESCE($2, name),
            status = COALESCE($3, status),
            executor_config = COALESCE($4, executor_config),
            labels = COALESCE($5, labels),
            max_concurrent_jobs = COALESCE($6, max_concurrent_jobs),
            current_jobs = COALESCE($7, current_jobs),
            last_seen_at = COALESCE($8, last_seen_at),
            last_used_at = COALESCE($9, last_used_at),
            version = COALESCE($10, version),
            os = COALESCE($11, os),
            arch = COALESCE($12, arch),
            updated_at = NOW()
        WHERE id = $1
        RETURNING id, uuid, name, token_hash, status, executor_type, executor_config,
                  labels, max_concurrent_jobs, current_jobs, last_seen_at, last_used_at,
                  version, os, arch, created_at, updated_at, deleted_at
        "#
    }

    pub fn update_status() -> &'static str {
        r#"
        UPDATE agents SET
            status = $2,
            current_jobs = $3,
            last_seen_at = NOW(),
            updated_at = NOW()
        WHERE id = $1
        "#
    }

    pub fn delete() -> &'static str {
        "UPDATE agents SET deleted_at = NOW(), updated_at = NOW(), status = 'disabled' WHERE id = $1"
    }
}
