//! Build Database Model

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

/// Build row from database
#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct DbBuild {
    pub id: i64,
    pub uuid: Uuid,
    pub project_id: i64,
    pub number: i32,
    pub ref_name: String,
    pub commit_sha: String,
    pub commit_message: Option<String>,
    pub author_id: i64,
    pub status: DbBuildStatus,
    pub trigger: DbBuildTrigger,
    pub trigger_data: Option<serde_json::Value>,
    pub agent_id: Option<i64>,
    pub started_at: Option<DateTime<Utc>>,
    pub finished_at: Option<DateTime<Utc>>,
    pub duration: Option<i32>,
    pub error_message: Option<String>,
    pub spec: serde_json::Value,
    pub parameters: Option<serde_json::Value>,
    pub artifacts: Option<serde_json::Value>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Build status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "build_status", rename_all = "lowercase")]
pub enum DbBuildStatus {
    Pending,
    Running,
    Success,
    Failed,
    Cancelled,
    Skipped,
}

/// Build trigger
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "build_trigger", rename_all = "snake_case")]
pub enum DbBuildTrigger {
    Push,
    PullRequest,
    Tag,
    Schedule,
    Manual,
    Api,
    Webhook,
}

/// Build creation input for database
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DbCreateBuild {
    pub project_id: i64,
    pub number: i32,
    pub ref_name: String,
    pub commit_sha: String,
    pub commit_message: Option<String>,
    pub author_id: i64,
    pub trigger: DbBuildTrigger,
    pub trigger_data: Option<serde_json::Value>,
    pub spec: serde_json::Value,
    pub parameters: Option<serde_json::Value>,
}

/// Build update input for database
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DbUpdateBuild {
    pub status: Option<DbBuildStatus>,
    pub agent_id: Option<i64>,
    pub started_at: Option<DateTime<Utc>>,
    pub finished_at: Option<DateTime<Utc>>,
    pub duration: Option<i32>,
    pub error_message: Option<String>,
    pub artifacts: Option<serde_json::Value>,
}

/// Build job row from database
#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct DbBuildJob {
    pub id: i64,
    pub uuid: Uuid,
    pub build_id: i64,
    pub name: String,
    pub status: DbBuildStatus,
    pub agent_id: Option<i64>,
    pub started_at: Option<DateTime<Utc>>,
    pub finished_at: Option<DateTime<Utc>>,
    pub duration: Option<i32>,
    pub error_message: Option<String>,
    pub spec: serde_json::Value,
    pub dependencies: Vec<i64>,
    #[sqlx(rename = "order_num")]
    pub order: i32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Build step row from database
#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct DbBuildStep {
    pub id: i64,
    pub uuid: Uuid,
    pub job_id: i64,
    pub name: String,
    pub status: DbBuildStatus,
    pub command: Option<String>,
    pub image: Option<String>,
    pub started_at: Option<DateTime<Utc>>,
    pub finished_at: Option<DateTime<Utc>>,
    pub duration: Option<i32>,
    pub error_message: Option<String>,
    pub logs: Option<String>,
    #[sqlx(rename = "order_num")]
    pub order: i32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Database query builder for builds
pub struct BuildQueryBuilder;

impl BuildQueryBuilder {
    pub fn select_all() -> &'static str {
        r#"
        SELECT id, uuid, project_id, number, ref_name, commit_sha, commit_message,
               author_id, status, trigger, trigger_data, agent_id, started_at,
               finished_at, duration, error_message, spec, parameters, artifacts,
               created_at, updated_at
        FROM builds
        "#
    }

    pub fn select_by_id() -> &'static str {
        r#"
        SELECT id, uuid, project_id, number, ref_name, commit_sha, commit_message,
               author_id, status, trigger, trigger_data, agent_id, started_at,
               finished_at, duration, error_message, spec, parameters, artifacts,
               created_at, updated_at
        FROM builds WHERE id = $1
        "#
    }

    pub fn select_by_project_id() -> &'static str {
        r#"
        SELECT id, uuid, project_id, number, ref_name, commit_sha, commit_message,
               author_id, status, trigger, trigger_data, agent_id, started_at,
               finished_at, duration, error_message, spec, parameters, artifacts,
               created_at, updated_at
        FROM builds WHERE project_id = $1
        ORDER BY number DESC
        LIMIT $2 OFFSET $3
        "#
    }

    pub fn select_by_project_and_number() -> &'static str {
        r#"
        SELECT id, uuid, project_id, number, ref_name, commit_sha, commit_message,
               author_id, status, trigger, trigger_data, agent_id, started_at,
               finished_at, duration, error_message, spec, parameters, artifacts,
               created_at, updated_at
        FROM builds WHERE project_id = $1 AND number = $2
        "#
    }

    pub fn select_running() -> &'static str {
        r#"
        SELECT id, uuid, project_id, number, ref_name, commit_sha, commit_message,
               author_id, status, trigger, trigger_data, agent_id, started_at,
               finished_at, duration, error_message, spec, parameters, artifacts,
               created_at, updated_at
        FROM builds WHERE status IN ('pending', 'running')
        ORDER BY created_at ASC
        "#
    }

    pub fn insert() -> &'static str {
        r#"
        INSERT INTO builds (uuid, project_id, number, ref_name, commit_sha, commit_message,
                            author_id, status, trigger, trigger_data, spec, parameters,
                            created_at, updated_at)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14)
        RETURNING id
        "#
    }

    pub fn update() -> &'static str {
        r#"
        UPDATE builds SET
            status = COALESCE($2, status),
            agent_id = COALESCE($3, agent_id),
            started_at = COALESCE($4, started_at),
            finished_at = COALESCE($5, finished_at),
            duration = COALESCE($6, duration),
            error_message = COALESCE($7, error_message),
            artifacts = COALESCE($8, artifacts),
            updated_at = NOW()
        WHERE id = $1
        RETURNING id, uuid, project_id, number, ref_name, commit_sha, commit_message,
                  author_id, status, trigger, trigger_data, agent_id, started_at,
                  finished_at, duration, error_message, spec, parameters, artifacts,
                  created_at, updated_at
        "#
    }
}
