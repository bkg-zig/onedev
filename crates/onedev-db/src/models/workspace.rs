//! Workspace Database Model

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, Row};
use uuid::Uuid;

/// Workspace row from database
#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct DbWorkspace {
    pub id: i64,
    pub uuid: Uuid,
    pub project_id: i64,
    pub name: String,
    pub spec: serde_json::Value,
    pub status: DbWorkspaceStatus,
    pub agent_id: Option<i64>,
    pub creator_id: i64,
    pub commit_sha: Option<String>,
    pub branch: Option<String>,
    pub started_at: Option<DateTime<Utc>>,
    pub finished_at: Option<DateTime<Utc>>,
    pub error_message: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Workspace status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "workspace_status", rename_all = "lowercase")]
pub enum DbWorkspaceStatus {
    Pending,
    Provisioning,
    Running,
    Stopped,
    Failed,
    Terminated,
}

/// Workspace creation input for database
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DbCreateWorkspace {
    pub project_id: i64,
    pub name: String,
    pub spec: serde_json::Value,
    pub creator_id: i64,
    pub commit_sha: Option<String>,
    pub branch: Option<String>,
}

/// Workspace update input for database
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DbUpdateWorkspace {
    pub status: Option<DbWorkspaceStatus>,
    pub agent_id: Option<i64>,
    pub commit_sha: Option<String>,
    pub branch: Option<String>,
    pub started_at: Option<DateTime<Utc>>,
    pub finished_at: Option<DateTime<Utc>>,
    pub error_message: Option<String>,
}

/// Database query builder for workspaces
pub struct WorkspaceQueryBuilder;

impl WorkspaceQueryBuilder {
    pub fn select_all() -> &'static str {
        r#"
        SELECT id, uuid, project_id, name, spec, status, agent_id, creator_id,
               commit_sha, branch, started_at, finished_at, error_message,
               created_at, updated_at
        FROM workspaces
        "#
    }

    pub fn select_by_id() -> &'static str {
        r#"
        SELECT id, uuid, project_id, name, spec, status, agent_id, creator_id,
               commit_sha, branch, started_at, finished_at, error_message,
               created_at, updated_at
        FROM workspaces WHERE id = $1
        "#
    }

    pub fn select_by_project_id() -> &'static str {
        r#"
        SELECT id, uuid, project_id, name, spec, status, agent_id, creator_id,
               commit_sha, branch, started_at, finished_at, error_message,
               created_at, updated_at
        FROM workspaces WHERE project_id = $1
        ORDER BY created_at DESC
        LIMIT $2 OFFSET $3
        "#
    }

    pub fn select_by_creator_id() -> &'static str {
        r#"
        SELECT id, uuid, project_id, name, spec, status, agent_id, creator_id,
               commit_sha, branch, started_at, finished_at, error_message,
               created_at, updated_at
        FROM workspaces WHERE creator_id = $1
        ORDER BY created_at DESC
        LIMIT $2 OFFSET $3
        "#
    }

    pub fn select_by_agent_id() -> &'static str {
        r#"
        SELECT id, uuid, project_id, name, spec, status, agent_id, creator_id,
               commit_sha, branch, started_at, finished_at, error_message,
               created_at, updated_at
        FROM workspaces WHERE agent_id = $1
        ORDER BY created_at DESC
        "#
    }

    pub fn insert() -> &'static str {
        r#"
        INSERT INTO workspaces (uuid, project_id, name, spec, status, agent_id, creator_id,
                                commit_sha, branch, created_at, updated_at)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
        RETURNING id
        "#
    }

    pub fn update() -> &'static str {
        r#"
        UPDATE workspaces SET
            status = COALESCE($2, status),
            agent_id = COALESCE($3, agent_id),
            commit_sha = COALESCE($4, commit_sha),
            branch = COALESCE($5, branch),
            started_at = COALESCE($6, started_at),
            finished_at = COALESCE($7, finished_at),
            error_message = COALESCE($8, error_message),
            updated_at = NOW()
        WHERE id = $1
        RETURNING id, uuid, project_id, name, spec, status, agent_id, creator_id,
                  commit_sha, branch, started_at, finished_at, error_message,
                  created_at, updated_at
        "#
    }

    pub fn delete() -> &'static str {
        "DELETE FROM workspaces WHERE id = $1"
    }
}