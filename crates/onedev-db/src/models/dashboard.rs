//! Dashboard Database Model

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

/// Dashboard row from database
#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct DbDashboard {
    pub id: i64,
    pub uuid: Uuid,
    pub owner_id: i64,
    pub owner_type: DbDashboardOwnerType,
    pub name: String,
    pub layout: serde_json::Value,
    pub is_default: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Dashboard owner type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "dashboard_owner_type", rename_all = "lowercase")]
pub enum DbDashboardOwnerType {
    User,
    Group,
}

/// Dashboard creation input for database
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DbCreateDashboard {
    pub owner_id: i64,
    pub owner_type: DbDashboardOwnerType,
    pub name: String,
    pub layout: serde_json::Value,
    pub is_default: bool,
}

/// Dashboard update input for database
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DbUpdateDashboard {
    pub name: Option<String>,
    pub layout: Option<serde_json::Value>,
    pub is_default: Option<bool>,
}

/// Dashboard share row from database
#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct DbDashboardShare {
    pub id: i64,
    pub uuid: Uuid,
    pub dashboard_id: i64,
    pub user_id: i64,
    pub permission: DbDashboardPermission,
    pub created_at: DateTime<Utc>,
}

/// Dashboard permission
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "dashboard_permission", rename_all = "lowercase")]
pub enum DbDashboardPermission {
    View,
    Edit,
}

/// Database query builder for dashboards
pub struct DashboardQueryBuilder;

impl DashboardQueryBuilder {
    pub fn select_all() -> &'static str {
        r#"
        SELECT id, uuid, owner_id, owner_type, name, layout, is_default, created_at, updated_at
        FROM dashboards
        "#
    }

    pub fn select_by_id() -> &'static str {
        r#"
        SELECT id, uuid, owner_id, owner_type, name, layout, is_default, created_at, updated_at
        FROM dashboards WHERE id = $1
        "#
    }

    pub fn select_by_owner() -> &'static str {
        r#"
        SELECT id, uuid, owner_id, owner_type, name, layout, is_default, created_at, updated_at
        FROM dashboards WHERE owner_id = $1 AND owner_type = $2
        ORDER BY is_default DESC, name ASC
        "#
    }

    pub fn select_default_by_owner() -> &'static str {
        r#"
        SELECT id, uuid, owner_id, owner_type, name, layout, is_default, created_at, updated_at
        FROM dashboards WHERE owner_id = $1 AND owner_type = $2 AND is_default = true
        LIMIT 1
        "#
    }

    pub fn insert() -> &'static str {
        r#"
        INSERT INTO dashboards (uuid, owner_id, owner_type, name, layout, is_default, created_at, updated_at)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
        RETURNING id
        "#
    }

    pub fn update() -> &'static str {
        r#"
        UPDATE dashboards SET
            name = COALESCE($2, name),
            layout = COALESCE($3, layout),
            is_default = COALESCE($4, is_default),
            updated_at = NOW()
        WHERE id = $1
        RETURNING id, uuid, owner_id, owner_type, name, layout, is_default, created_at, updated_at
        "#
    }

    pub fn delete() -> &'static str {
        "DELETE FROM dashboards WHERE id = $1"
    }
}

/// Database query builder for dashboard shares
pub struct DashboardShareQueryBuilder;

impl DashboardShareQueryBuilder {
    pub fn select_by_id() -> &'static str {
        r#"
        SELECT id, uuid, dashboard_id, user_id, permission, created_at
        FROM dashboard_shares WHERE id = $1
        "#
    }

    pub fn select_by_dashboard_id() -> &'static str {
        r#"
        SELECT id, uuid, dashboard_id, user_id, permission, created_at
        FROM dashboard_shares WHERE dashboard_id = $1
        "#
    }

    pub fn select_by_dashboard_and_user() -> &'static str {
        r#"
        SELECT id, uuid, dashboard_id, user_id, permission, created_at
        FROM dashboard_shares WHERE dashboard_id = $1 AND user_id = $2
        "#
    }

    pub fn insert() -> &'static str {
        r#"
        INSERT INTO dashboard_shares (uuid, dashboard_id, user_id, permission, created_at)
        VALUES ($1, $2, $3, $4, $5)
        RETURNING id
        "#
    }

    pub fn update() -> &'static str {
        r#"
        UPDATE dashboard_shares SET
            permission = COALESCE($2, permission)
        WHERE id = $1
        RETURNING id, uuid, dashboard_id, user_id, permission, created_at
        "#
    }

    pub fn delete() -> &'static str {
        "DELETE FROM dashboard_shares WHERE id = $1"
    }
}
