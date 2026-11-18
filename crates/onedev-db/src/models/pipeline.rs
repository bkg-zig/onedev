//! Pipeline Database Model

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

/// Pipeline row from database
#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct DbPipeline {
    pub id: i64,
    pub uuid: Uuid,
    pub project_id: i64,
    pub name: String,
    pub spec: serde_json::Value,
    pub is_default: bool,
    pub protected: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Pipeline creation input for database
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DbCreatePipeline {
    pub project_id: i64,
    pub name: String,
    pub spec: serde_json::Value,
    pub is_default: bool,
    pub protected: bool,
}

/// Pipeline update input for database
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DbUpdatePipeline {
    pub name: Option<String>,
    pub spec: Option<serde_json::Value>,
    pub is_default: Option<bool>,
    pub protected: Option<bool>,
}

/// Pipeline schedule row from database
#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct DbPipelineSchedule {
    pub id: i64,
    pub uuid: Uuid,
    pub pipeline_id: i64,
    pub cron_expression: String,
    pub timezone: String,
    pub ref_name: String,
    pub parameters: Option<serde_json::Value>,
    pub active: bool,
    pub next_run_at: Option<DateTime<Utc>>,
    pub last_run_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Database query builder for pipelines
pub struct PipelineQueryBuilder;

impl PipelineQueryBuilder {
    pub fn select_all() -> &'static str {
        r#"
        SELECT id, uuid, project_id, name, spec, is_default, protected, created_at, updated_at
        FROM pipelines
        "#
    }

    pub fn select_by_id() -> &'static str {
        r#"
        SELECT id, uuid, project_id, name, spec, is_default, protected, created_at, updated_at
        FROM pipelines WHERE id = $1
        "#
    }

    pub fn select_by_project_id() -> &'static str {
        r#"
        SELECT id, uuid, project_id, name, spec, is_default, protected, created_at, updated_at
        FROM pipelines WHERE project_id = $1
        ORDER BY is_default DESC, name ASC
        "#
    }

    pub fn select_default_by_project() -> &'static str {
        r#"
        SELECT id, uuid, project_id, name, spec, is_default, protected, created_at, updated_at
        FROM pipelines WHERE project_id = $1 AND is_default = true
        LIMIT 1
        "#
    }

    pub fn insert() -> &'static str {
        r#"
        INSERT INTO pipelines (uuid, project_id, name, spec, is_default, protected, created_at, updated_at)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
        RETURNING id
        "#
    }

    pub fn update() -> &'static str {
        r#"
        UPDATE pipelines SET
            name = COALESCE($2, name),
            spec = COALESCE($3, spec),
            is_default = COALESCE($4, is_default),
            protected = COALESCE($5, protected),
            updated_at = NOW()
        WHERE id = $1
        RETURNING id, uuid, project_id, name, spec, is_default, protected, created_at, updated_at
        "#
    }

    pub fn delete() -> &'static str {
        "DELETE FROM pipelines WHERE id = $1"
    }
}

/// Database query builder for pipeline schedules
pub struct PipelineScheduleQueryBuilder;

impl PipelineScheduleQueryBuilder {
    pub fn select_by_pipeline_id() -> &'static str {
        r#"
        SELECT id, uuid, pipeline_id, cron_expression, timezone, ref_name,
               parameters, active, next_run_at, last_run_at, created_at, updated_at
        FROM pipeline_schedules WHERE pipeline_id = $1
        "#
    }

    pub fn select_due() -> &'static str {
        r#"
        SELECT id, uuid, pipeline_id, cron_expression, timezone, ref_name,
               parameters, active, next_run_at, last_run_at, created_at, updated_at
        FROM pipeline_schedules WHERE active = true AND next_run_at <= NOW()
        ORDER BY next_run_at ASC
        "#
    }

    pub fn insert() -> &'static str {
        r#"
        INSERT INTO pipeline_schedules (uuid, pipeline_id, cron_expression, timezone, ref_name,
                                        parameters, active, next_run_at, created_at, updated_at)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
        RETURNING id
        "#
    }

    pub fn update() -> &'static str {
        r#"
        UPDATE pipeline_schedules SET
            cron_expression = COALESCE($2, cron_expression),
            timezone = COALESCE($3, timezone),
            ref_name = COALESCE($4, ref_name),
            parameters = COALESCE($5, parameters),
            active = COALESCE($6, active),
            next_run_at = COALESCE($7, next_run_at),
            last_run_at = COALESCE($8, last_run_at),
            updated_at = NOW()
        WHERE id = $1
        RETURNING id, uuid, pipeline_id, cron_expression, timezone, ref_name,
                  parameters, active, next_run_at, last_run_at, created_at, updated_at
        "#
    }

    pub fn delete() -> &'static str {
        "DELETE FROM pipeline_schedules WHERE id = $1"
    }
}
