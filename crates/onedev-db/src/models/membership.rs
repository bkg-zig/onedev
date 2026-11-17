//! Membership Database Model

use super::*;
use crate::DbResult;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, Postgres, Row};
use uuid::Uuid;

/// Membership row from database
#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct DbMembership {
    pub id: i64,
    pub uuid: Uuid,
    pub user_id: i64,
    pub group_id: i64,
    pub role: DbGroupRole,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Group role in the database
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "group_role", rename_all = "lowercase")]
pub enum DbGroupRole {
    Guest,
    Reporter,
    Developer,
    Maintainer,
    Owner,
}

impl From<DbGroupRole> for onedev_core::models::role::GroupRole {
    fn from(r: DbGroupRole) -> Self {
        match r {
            DbGroupRole::Guest => onedev_core::models::role::GroupRole::Guest,
            DbGroupRole::Reporter => onedev_core::models::role::GroupRole::Reporter,
            DbGroupRole::Developer => onedev_core::models::role::GroupRole::Developer,
            DbGroupRole::Maintainer => onedev_core::models::role::GroupRole::Maintainer,
            DbGroupRole::Owner => onedev_core::models::role::GroupRole::Owner,
        }
    }
}

impl From<onedev_core::models::role::GroupRole> for DbGroupRole {
    fn from(r: onedev_core::models::role::GroupRole) -> Self {
        match r {
            onedev_core::models::role::GroupRole::Guest => DbGroupRole::Guest,
            onedev_core::models::role::GroupRole::Reporter => DbGroupRole::Reporter,
            onedev_core::models::role::GroupRole::Developer => DbGroupRole::Developer,
            onedev_core::models::role::GroupRole::Maintainer => DbGroupRole::Maintainer,
            onedev_core::models::role::GroupRole::Owner => DbGroupRole::Owner,
        }
    }
}

/// Membership creation input for database
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DbCreateMembership {
    pub user_id: i64,
    pub group_id: i64,
    pub role: DbGroupRole,
}

/// Membership update input for database
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DbUpdateMembership {
    pub role: Option<DbGroupRole>,
}

/// Database query builder for memberships
pub struct MembershipQueryBuilder;

impl MembershipQueryBuilder {
    pub fn select_all() -> &'static str {
        r#"
        SELECT id, uuid, user_id, group_id, role, created_at, updated_at
        FROM memberships
        "#
    }

    pub fn select_by_id() -> &'static str {
        r#"
        SELECT id, uuid, user_id, group_id, role, created_at, updated_at
        FROM memberships WHERE id = $1
        "#
    }

    pub fn select_by_user_id() -> &'static str {
        r#"
        SELECT id, uuid, user_id, group_id, role, created_at, updated_at
        FROM memberships WHERE user_id = $1
        ORDER BY group_id
        "#
    }

    pub fn select_by_group_id() -> &'static str {
        r#"
        SELECT id, uuid, user_id, group_id, role, created_at, updated_at
        FROM memberships WHERE group_id = $1
        ORDER BY role DESC, created_at
        "#
    }

    pub fn select_by_user_and_group() -> &'static str {
        r#"
        SELECT id, uuid, user_id, group_id, role, created_at, updated_at
        FROM memberships WHERE user_id = $1 AND group_id = $2
        "#
    }

    pub fn insert() -> &'static str {
        r#"
        INSERT INTO memberships (uuid, user_id, group_id, role, created_at, updated_at)
        VALUES ($1, $2, $3, $4, $5, $6)
        RETURNING id
        "#
    }

    pub fn update() -> &'static str {
        r#"
        UPDATE memberships SET
            role = COALESCE($2, role),
            updated_at = NOW()
        WHERE id = $1
        RETURNING id, uuid, user_id, group_id, role, created_at, updated_at
        "#
    }

    pub fn delete() -> &'static str {
        "DELETE FROM memberships WHERE id = $1"
    }

    pub fn delete_by_user_and_group() -> &'static str {
        "DELETE FROM memberships WHERE user_id = $1 AND group_id = $2"
    }
}