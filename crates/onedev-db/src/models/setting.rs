//! Settings Database Model

use super::*;
use crate::DbResult;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, Postgres, Row};
use uuid::Uuid;

/// System setting row from database
#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct DbSystemSetting {
    pub id: i64,
    pub uuid: Uuid,
    pub key: String,
    pub value: serde_json::Value,
    pub description: Option<String>,
    pub category: String,
    pub editable: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Project setting row from database
#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct DbProjectSetting {
    pub id: i64,
    pub uuid: Uuid,
    pub project_id: i64,
    pub key: String,
    pub value: serde_json::Value,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// User setting row from database
#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct DbUserSetting {
    pub id: i64,
    pub uuid: Uuid,
    pub user_id: i64,
    pub key: String,
    pub value: serde_json::Value,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Setting creation input for database
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DbCreateSetting {
    pub key: String,
    pub value: serde_json::Value,
    pub description: Option<String>,
    pub category: String,
    pub editable: bool,
}

/// Setting update input for database
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DbUpdateSetting {
    pub value: Option<serde_json::Value>,
    pub description: Option<String>,
    pub editable: Option<bool>,
}

/// Database query builder for system settings
pub struct SystemSettingQueryBuilder;

impl SystemSettingQueryBuilder {
    pub fn select_all() -> &'static str {
        r#"
        SELECT id, uuid, key, value, description, category, editable,
               created_at, updated_at
        FROM system_settings
        "#
    }

    pub fn select_by_id() -> &'static str {
        r#"
        SELECT id, uuid, key, value, description, category, editable,
               created_at, updated_at
        FROM system_settings WHERE id = $1
        "#
    }

    pub fn select_by_key() -> &'static str {
        r#"
        SELECT id, uuid, key, value, description, category, editable,
               created_at, updated_at
        FROM system_settings WHERE key = $1
        "#
    }

    pub fn insert() -> &'static str {
        r#"
        INSERT INTO system_settings (uuid, key, value, description, category,
                                     editable, created_at, updated_at)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
        RETURNING id
        "#
    }

    pub fn update() -> &'static str {
        r#"
        UPDATE system_settings SET
            value = COALESCE($2, value),
            description = COALESCE($3, description),
            editable = COALESCE($4, editable),
            updated_at = NOW()
        WHERE id = $1
        RETURNING id, uuid, key, value, description, category, editable,
                  created_at, updated_at
        "#
    }

    pub fn delete() -> &'static str {
        "DELETE FROM system_settings WHERE id = $1"
    }
}

/// Database query builder for project settings
pub struct ProjectSettingQueryBuilder;

impl ProjectSettingQueryBuilder {
    pub fn select_by_project_id() -> &'static str {
        r#"
        SELECT id, uuid, project_id, key, value, created_at, updated_at
        FROM project_settings WHERE project_id = $1
        "#
    }

    pub fn select_by_project_and_key() -> &'static str {
        r#"
        SELECT id, uuid, project_id, key, value, created_at, updated_at
        FROM project_settings WHERE project_id = $1 AND key = $2
        "#
    }

    pub fn upsert() -> &'static str {
        r#"
        INSERT INTO project_settings (uuid, project_id, key, value, created_at, updated_at)
        VALUES ($1, $2, $3, $4, $5, $6)
        ON CONFLICT (project_id, key) DO UPDATE SET
            value = EXCLUDED.value,
            updated_at = NOW()
        RETURNING id
        "#
    }

    pub fn delete() -> &'static str {
        "DELETE FROM project_settings WHERE project_id = $1 AND key = $2"
    }
}