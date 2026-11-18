//! Label Database Model

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

/// Label row from database
#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct DbLabel {
    pub id: i64,
    pub uuid: Uuid,
    pub project_id: i64,
    pub name: String,
    pub color: String,
    pub description: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Label creation input for database
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DbCreateLabel {
    pub project_id: i64,
    pub name: String,
    pub color: String,
    pub description: Option<String>,
}

/// Label update input for database
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DbUpdateLabel {
    pub name: Option<String>,
    pub color: Option<String>,
    pub description: Option<String>,
}

/// Database query builder for labels
pub struct LabelQueryBuilder;

impl LabelQueryBuilder {
    pub fn select_all() -> &'static str {
        r#"
        SELECT id, uuid, project_id, name, color, description, created_at, updated_at
        FROM labels
        "#
    }

    pub fn select_by_id() -> &'static str {
        r#"
        SELECT id, uuid, project_id, name, color, description, created_at, updated_at
        FROM labels WHERE id = $1
        "#
    }

    pub fn select_by_project_id() -> &'static str {
        r#"
        SELECT id, uuid, project_id, name, color, description, created_at, updated_at
        FROM labels WHERE project_id = $1
        ORDER BY name ASC
        "#
    }

    pub fn select_by_project_and_name() -> &'static str {
        r#"
        SELECT id, uuid, project_id, name, color, description, created_at, updated_at
        FROM labels WHERE project_id = $1 AND name = $2
        "#
    }

    pub fn insert() -> &'static str {
        r#"
        INSERT INTO labels (uuid, project_id, name, color, description, created_at, updated_at)
        VALUES ($1, $2, $3, $4, $5, $6, $7)
        RETURNING id
        "#
    }

    pub fn update() -> &'static str {
        r#"
        UPDATE labels SET
            name = COALESCE($2, name),
            color = COALESCE($3, color),
            description = COALESCE($4, description),
            updated_at = NOW()
        WHERE id = $1
        RETURNING id, uuid, project_id, name, color, description, created_at, updated_at
        "#
    }

    pub fn delete() -> &'static str {
        "DELETE FROM labels WHERE id = $1"
    }
}
