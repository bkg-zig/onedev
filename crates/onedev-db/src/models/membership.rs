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

// onedev-core models a group role as `group::AccessLevel` (there is no `GroupRole` type)
impl From<DbGroupRole> for onedev_core::models::group::AccessLevel {
    fn from(r: DbGroupRole) -> Self {
        match r {
            DbGroupRole::Guest => Self::Guest,
            DbGroupRole::Reporter => Self::Reporter,
            DbGroupRole::Developer => Self::Developer,
            DbGroupRole::Maintainer => Self::Maintainer,
            DbGroupRole::Owner => Self::Owner,
        }
    }
}

impl From<onedev_core::models::group::AccessLevel> for DbGroupRole {
    fn from(l: onedev_core::models::group::AccessLevel) -> Self {
        match l {
            onedev_core::models::group::AccessLevel::Guest => Self::Guest,
            onedev_core::models::group::AccessLevel::Reporter => Self::Reporter,
            onedev_core::models::group::AccessLevel::Developer => Self::Developer,
            onedev_core::models::group::AccessLevel::Maintainer => Self::Maintainer,
            onedev_core::models::group::AccessLevel::Owner => Self::Owner,
        }
    }
}

impl DbMembership {
    /// Convert to core domain model
    ///
    /// The `memberships` table stores neither an expiry nor the creator, so
    /// `expires_at` is `None`, `created_by_id` is `UNKNOWN_USER_ID` and the
    /// `user` relation is not loaded.
    pub fn to_core(&self) -> onedev_core::models::group::GroupMember {
        onedev_core::models::group::GroupMember {
            id: self.id,
            uuid: self.uuid,
            group_id: self.group_id,
            user_id: self.user_id,
            access_level: self.role.into(),
            expires_at: None,
            created_at: self.created_at,
            created_by_id: onedev_core::models::user::UNKNOWN_USER_ID,
            user: None,
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