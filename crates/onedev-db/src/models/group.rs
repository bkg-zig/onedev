//! Group Database Model

use super::*;
use crate::DbResult;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, Postgres, Row};
use uuid::Uuid;

/// Group row from database
#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct DbGroup {
    pub id: i64,
    pub uuid: Uuid,
    pub name: String,
    pub path: String,
    pub description: Option<String>,
    pub parent_id: Option<i64>,
    pub avatar_url: Option<String>,
    pub visibility: DbGroupVisibility,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
    // Settings
    pub require_two_factor: bool,
    pub two_factor_grace_period: i32,
    pub project_creation_level: DbProjectCreationLevel,
    pub subgroup_creation_level: DbSubgroupCreationLevel,
    pub settings: Option<serde_json::Value>,
}

/// Group visibility
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "group_visibility", rename_all = "lowercase")]
pub enum DbGroupVisibility {
    Private,
    Internal,
    Public,
}

/// Project creation level
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "project_creation_level", rename_all = "lowercase")]
pub enum DbProjectCreationLevel {
    Noone,
    Maintainer,
    Developer,
    Owner,
}

/// Subgroup creation level
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "subgroup_creation_level", rename_all = "lowercase")]
pub enum DbSubgroupCreationLevel {
    Owner,
    Maintainer,
}

impl From<DbGroupVisibility> for onedev_core::models::group::GroupVisibility {
    fn from(v: DbGroupVisibility) -> Self {
        match v {
            DbGroupVisibility::Private => Self::Private,
            DbGroupVisibility::Internal => Self::Internal,
            DbGroupVisibility::Public => Self::Public,
        }
    }
}

impl From<onedev_core::models::group::GroupVisibility> for DbGroupVisibility {
    fn from(v: onedev_core::models::group::GroupVisibility) -> Self {
        match v {
            onedev_core::models::group::GroupVisibility::Private => Self::Private,
            onedev_core::models::group::GroupVisibility::Internal => Self::Internal,
            onedev_core::models::group::GroupVisibility::Public => Self::Public,
        }
    }
}

impl From<DbGroupVisibility> for onedev_core::models::group::VisibilityLevel {
    fn from(v: DbGroupVisibility) -> Self {
        match v {
            DbGroupVisibility::Private => Self::Private,
            DbGroupVisibility::Internal => Self::Internal,
            DbGroupVisibility::Public => Self::Public,
        }
    }
}

impl From<DbProjectCreationLevel> for onedev_core::models::group::ProjectCreationLevel {
    fn from(l: DbProjectCreationLevel) -> Self {
        match l {
            DbProjectCreationLevel::Noone => Self::NoOne,
            DbProjectCreationLevel::Maintainer => Self::Maintainer,
            DbProjectCreationLevel::Developer => Self::Developer,
            DbProjectCreationLevel::Owner => Self::Owner,
        }
    }
}

impl From<onedev_core::models::group::ProjectCreationLevel> for DbProjectCreationLevel {
    fn from(l: onedev_core::models::group::ProjectCreationLevel) -> Self {
        match l {
            onedev_core::models::group::ProjectCreationLevel::NoOne => Self::Noone,
            onedev_core::models::group::ProjectCreationLevel::Maintainer => Self::Maintainer,
            onedev_core::models::group::ProjectCreationLevel::Developer => Self::Developer,
            onedev_core::models::group::ProjectCreationLevel::Owner => Self::Owner,
        }
    }
}

impl From<DbSubgroupCreationLevel> for onedev_core::models::group::SubgroupCreationLevel {
    fn from(l: DbSubgroupCreationLevel) -> Self {
        match l {
            DbSubgroupCreationLevel::Owner => Self::Owner,
            DbSubgroupCreationLevel::Maintainer => Self::Maintainer,
        }
    }
}

impl From<onedev_core::models::group::SubgroupCreationLevel> for DbSubgroupCreationLevel {
    fn from(l: onedev_core::models::group::SubgroupCreationLevel) -> Self {
        match l {
            onedev_core::models::group::SubgroupCreationLevel::Owner => Self::Owner,
            onedev_core::models::group::SubgroupCreationLevel::Maintainer => Self::Maintainer,
        }
    }
}

/// Group creation input for database
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DbCreateGroup {
    pub name: String,
    pub path: String,
    pub description: Option<String>,
    pub parent_id: Option<i64>,
    pub visibility: DbGroupVisibility,
    pub require_two_factor: bool,
    pub two_factor_grace_period: i32,
    pub project_creation_level: DbProjectCreationLevel,
    pub subgroup_creation_level: DbSubgroupCreationLevel,
}

/// Group update input for database
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DbUpdateGroup {
    pub name: Option<String>,
    pub description: Option<String>,
    pub avatar_url: Option<String>,
    pub visibility: Option<DbGroupVisibility>,
    pub require_two_factor: Option<bool>,
    pub two_factor_grace_period: Option<i32>,
    pub project_creation_level: Option<DbProjectCreationLevel>,
    pub subgroup_creation_level: Option<DbSubgroupCreationLevel>,
    pub settings: Option<serde_json::Value>,
}

impl DbGroup {
    /// Convert to core domain model
    ///
    /// The `groups` table stores no owner, LDAP mapping or shared-runner quota,
    /// so those core fields get neutral defaults: `owner_id` is
    /// `UNKNOWN_USER_ID`, the optional ones are `None`. `visibility_level`
    /// mirrors `visibility`.
    pub fn to_core(&self) -> onedev_core::models::group::Group {
        onedev_core::models::group::Group {
            id: self.id,
            uuid: self.uuid,
            name: self.name.clone(),
            path: self.path.clone(),
            description: self.description.clone(),
            visibility: self.visibility.into(),
            parent_id: self.parent_id,
            owner_id: onedev_core::models::user::UNKNOWN_USER_ID,
            avatar_url: self.avatar_url.clone(),
            require_two_factor: self.require_two_factor,
            two_factor_grace_period: u32::try_from(self.two_factor_grace_period).unwrap_or(0),
            project_creation_level: self.project_creation_level.into(),
            subgroup_creation_level: self.subgroup_creation_level.into(),
            visibility_level: self.visibility.into(),
            shared_runners_minutes: None,
            extra_shared_runners_minutes: None,
            ldap_cn: None,
            ldap_access: None,
            created_at: self.created_at,
            updated_at: self.updated_at,
            deleted_at: self.deleted_at,
        }
    }
}

/// Database query builder for groups
pub struct GroupQueryBuilder;

impl GroupQueryBuilder {
    pub fn select_all() -> &'static str {
        r#"
        SELECT id, uuid, name, path, description, parent_id, avatar_url,
               visibility, created_at, updated_at, deleted_at,
               require_two_factor, two_factor_grace_period,
               project_creation_level, subgroup_creation_level, settings
        FROM groups
        "#
    }

    pub fn select_by_id() -> &'static str {
        r#"
        SELECT id, uuid, name, path, description, parent_id, avatar_url,
               visibility, created_at, updated_at, deleted_at,
               require_two_factor, two_factor_grace_period,
               project_creation_level, subgroup_creation_level, settings
        FROM groups WHERE id = $1
        "#
    }

    pub fn select_by_path() -> &'static str {
        r#"
        SELECT id, uuid, name, path, description, parent_id, avatar_url,
               visibility, created_at, updated_at, deleted_at,
               require_two_factor, two_factor_grace_period,
               project_creation_level, subgroup_creation_level, settings
        FROM groups WHERE path = $1 AND parent_id IS NOT DISTINCT FROM $2
        "#
    }

    pub fn select_by_uuid() -> &'static str {
        r#"
        SELECT id, uuid, name, path, description, parent_id, avatar_url,
               visibility, created_at, updated_at, deleted_at,
               require_two_factor, two_factor_grace_period,
               project_creation_level, subgroup_creation_level, settings
        FROM groups WHERE uuid = $1
        "#
    }

    pub fn insert() -> &'static str {
        r#"
        INSERT INTO groups (uuid, name, path, description, parent_id, avatar_url,
                            visibility, created_at, updated_at,
                            require_two_factor, two_factor_grace_period,
                            project_creation_level, subgroup_creation_level, settings)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14)
        RETURNING id
        "#
    }

    pub fn update() -> &'static str {
        r#"
        UPDATE groups SET
            name = COALESCE($2, name),
            description = COALESCE($3, description),
            avatar_url = COALESCE($4, avatar_url),
            visibility = COALESCE($5, visibility),
            require_two_factor = COALESCE($6, require_two_factor),
            two_factor_grace_period = COALESCE($7, two_factor_grace_period),
            project_creation_level = COALESCE($8, project_creation_level),
            subgroup_creation_level = COALESCE($9, subgroup_creation_level),
            settings = COALESCE($10, settings),
            updated_at = NOW()
        WHERE id = $1
        RETURNING id, uuid, name, path, description, parent_id, avatar_url,
                  visibility, created_at, updated_at, deleted_at,
                  require_two_factor, two_factor_grace_period,
                  project_creation_level, subgroup_creation_level, settings
        "#
    }

    pub fn delete() -> &'static str {
        "UPDATE groups SET deleted_at = NOW(), updated_at = NOW() WHERE id = $1"
    }
}