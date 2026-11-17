//! Authorization Database Model

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, Row};
use uuid::Uuid;

/// User authorization row from database
#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct DbUserAuthorization {
    pub id: i64,
    pub uuid: Uuid,
    pub user_id: i64,
    pub project_id: i64,
    pub role: DbProjectRole,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Project role
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "project_role", rename_all = "lowercase")]
pub enum DbProjectRole {
    Guest,
    Reporter,
    Developer,
    Maintainer,
    Owner,
}

// onedev-core has no ProjectRole; project access uses the same levels as groups.
impl From<DbProjectRole> for onedev_core::models::group::AccessLevel {
    fn from(r: DbProjectRole) -> Self {
        match r {
            DbProjectRole::Guest => Self::Guest,
            DbProjectRole::Reporter => Self::Reporter,
            DbProjectRole::Developer => Self::Developer,
            DbProjectRole::Maintainer => Self::Maintainer,
            DbProjectRole::Owner => Self::Owner,
        }
    }
}

impl From<onedev_core::models::group::AccessLevel> for DbProjectRole {
    fn from(r: onedev_core::models::group::AccessLevel) -> Self {
        match r {
            onedev_core::models::group::AccessLevel::Guest => Self::Guest,
            onedev_core::models::group::AccessLevel::Reporter => Self::Reporter,
            onedev_core::models::group::AccessLevel::Developer => Self::Developer,
            onedev_core::models::group::AccessLevel::Maintainer => Self::Maintainer,
            onedev_core::models::group::AccessLevel::Owner => Self::Owner,
        }
    }
}

/// Issue authorization row from database
#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct DbIssueAuthorization {
    pub id: i64,
    pub uuid: Uuid,
    pub user_id: i64,
    pub issue_id: i64,
    pub permission: DbIssuePermission,
    pub created_at: DateTime<Utc>,
}

/// Issue permission
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "issue_permission", rename_all = "lowercase")]
pub enum DbIssuePermission {
    View,
    Edit,
    Admin,
}

/// User authorization creation input for database
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DbCreateUserAuthorization {
    pub user_id: i64,
    pub project_id: i64,
    pub role: DbProjectRole,
}

/// User authorization update input for database
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DbUpdateUserAuthorization {
    pub role: Option<DbProjectRole>,
}

/// Database query builder for user authorizations
pub struct UserAuthorizationQueryBuilder;

impl UserAuthorizationQueryBuilder {
    pub fn select_all() -> &'static str {
        r#"
        SELECT id, uuid, user_id, project_id, role, created_at, updated_at
        FROM user_authorizations
        "#
    }

    pub fn select_by_id() -> &'static str {
        r#"
        SELECT id, uuid, user_id, project_id, role, created_at, updated_at
        FROM user_authorizations WHERE id = $1
        "#
    }

    pub fn select_by_user_id() -> &'static str {
        r#"
        SELECT id, uuid, user_id, project_id, role, created_at, updated_at
        FROM user_authorizations WHERE user_id = $1
        ORDER BY project_id
        "#
    }

    pub fn select_by_project_id() -> &'static str {
        r#"
        SELECT id, uuid, user_id, project_id, role, created_at, updated_at
        FROM user_authorizations WHERE project_id = $1
        ORDER BY role DESC, user_id
        "#
    }

    pub fn select_by_user_and_project() -> &'static str {
        r#"
        SELECT id, uuid, user_id, project_id, role, created_at, updated_at
        FROM user_authorizations WHERE user_id = $1 AND project_id = $2
        "#
    }

    pub fn insert() -> &'static str {
        r#"
        INSERT INTO user_authorizations (uuid, user_id, project_id, role, created_at, updated_at)
        VALUES ($1, $2, $3, $4, $5, $6)
        RETURNING id
        "#
    }

    pub fn update() -> &'static str {
        r#"
        UPDATE user_authorizations SET
            role = COALESCE($2, role),
            updated_at = NOW()
        WHERE id = $1
        RETURNING id, uuid, user_id, project_id, role, created_at, updated_at
        "#
    }

    pub fn delete() -> &'static str {
        "DELETE FROM user_authorizations WHERE id = $1"
    }
}

/// Database query builder for issue authorizations
pub struct IssueAuthorizationQueryBuilder;

impl IssueAuthorizationQueryBuilder {
    pub fn select_by_user_id() -> &'static str {
        r#"
        SELECT id, uuid, user_id, issue_id, permission, created_at
        FROM issue_authorizations WHERE user_id = $1
        "#
    }

    pub fn select_by_issue_id() -> &'static str {
        r#"
        SELECT id, uuid, user_id, issue_id, permission, created_at
        FROM issue_authorizations WHERE issue_id = $1
        "#
    }

    pub fn insert() -> &'static str {
        r#"
        INSERT INTO issue_authorizations (uuid, user_id, issue_id, permission, created_at)
        VALUES ($1, $2, $3, $4, $5)
        RETURNING id
        "#
    }

    pub fn delete() -> &'static str {
        "DELETE FROM issue_authorizations WHERE id = $1"
    }
}