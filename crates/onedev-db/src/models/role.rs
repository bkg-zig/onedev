//! Role Database Model

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, Row};
use uuid::Uuid;

/// Role row from database
#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct DbRole {
    pub id: i64,
    pub uuid: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub is_system: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Role permission row from database
#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct DbRolePermission {
    pub id: i64,
    pub uuid: Uuid,
    pub role_id: i64,
    pub permission: String,
    pub resource_type: Option<String>,
    pub resource_id: Option<i64>,
    pub created_at: DateTime<Utc>,
}

/// Role creation input for database
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DbCreateRole {
    pub name: String,
    pub description: Option<String>,
    pub is_system: bool,
}

/// Role update input for database
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DbUpdateRole {
    pub name: Option<String>,
    pub description: Option<String>,
}

/// Database query builder for roles
pub struct RoleQueryBuilder;

impl RoleQueryBuilder {
    pub fn select_all() -> &'static str {
        r#"
        SELECT id, uuid, name, description, is_system, created_at, updated_at
        FROM roles
        "#
    }

    pub fn select_by_id() -> &'static str {
        r#"
        SELECT id, uuid, name, description, is_system, created_at, updated_at
        FROM roles WHERE id = $1
        "#
    }

    pub fn select_by_name() -> &'static str {
        r#"
        SELECT id, uuid, name, description, is_system, created_at, updated_at
        FROM roles WHERE name = $1
        "#
    }

    pub fn select_with_permissions() -> &'static str {
        r#"
        SELECT r.id, r.uuid, r.name, r.description, r.is_system, r.created_at, r.updated_at,
               rp.permission, rp.resource_type, rp.resource_id
        FROM roles r
        LEFT JOIN role_permissions rp ON r.id = rp.role_id
        WHERE r.id = $1
        "#
    }

    pub fn insert() -> &'static str {
        r#"
        INSERT INTO roles (uuid, name, description, is_system, created_at, updated_at)
        VALUES ($1, $2, $3, $4, $5, $6)
        RETURNING id
        "#
    }

    pub fn update() -> &'static str {
        r#"
        UPDATE roles SET
            name = COALESCE($2, name),
            description = COALESCE($3, description),
            updated_at = NOW()
        WHERE id = $1
        RETURNING id, uuid, name, description, is_system, created_at, updated_at
        "#
    }

    pub fn delete() -> &'static str {
        "DELETE FROM roles WHERE id = $1 AND is_system = false"
    }
}

/// Database query builder for role permissions
pub struct RolePermissionQueryBuilder;

impl RolePermissionQueryBuilder {
    pub fn select_by_role_id() -> &'static str {
        r#"
        SELECT id, uuid, role_id, permission, resource_type, resource_id, created_at
        FROM role_permissions WHERE role_id = $1
        "#
    }

    pub fn insert() -> &'static str {
        r#"
        INSERT INTO role_permissions (uuid, role_id, permission, resource_type, resource_id, created_at)
        VALUES ($1, $2, $3, $4, $5, $6)
        RETURNING id
        "#
    }

    pub fn delete_by_role_id() -> &'static str {
        "DELETE FROM role_permissions WHERE role_id = $1"
    }

    pub fn delete_by_role_and_permission() -> &'static str {
        "DELETE FROM role_permissions WHERE role_id = $1 AND permission = $2"
    }
}