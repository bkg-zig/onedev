//! Authorization Database Models — Vanilla authorization entities.
//!
//! Every authorization references a free-form role by id (`role_id`);
//! there are no fixed project roles. Entities: `UserAuthorization`,
//! `GroupAuthorization`, `BaseAuthorization`, `AccessTokenAuthorization`,
//! `LinkAuthorization`, `IssueAuthorization` (issue + user only).

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

/// User authorization row (user + project + role)
#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct DbUserAuthorization {
    pub id: i64,
    pub uuid: Uuid,
    pub user_id: i64,
    pub project_id: i64,
    pub role_id: i64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DbCreateUserAuthorization {
    pub user_id: i64,
    pub project_id: i64,
    pub role_id: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DbUpdateUserAuthorization {
    pub role_id: Option<i64>,
}

/// Group authorization row (group + project + role)
#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct DbGroupAuthorization {
    pub id: i64,
    pub uuid: Uuid,
    pub group_id: i64,
    pub project_id: i64,
    pub role_id: i64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DbCreateGroupAuthorization {
    pub group_id: i64,
    pub project_id: i64,
    pub role_id: i64,
}

/// Base authorization row (project + role, applies to everyone)
#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct DbBaseAuthorization {
    pub id: i64,
    pub uuid: Uuid,
    pub project_id: i64,
    pub role_id: i64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DbCreateBaseAuthorization {
    pub project_id: i64,
    pub role_id: i64,
}

/// Access-token authorization row (project + token + role)
#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct DbAccessTokenAuthorization {
    pub id: i64,
    pub uuid: Uuid,
    pub project_id: i64,
    pub access_token_id: i64,
    pub role_id: i64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DbCreateAccessTokenAuthorization {
    pub project_id: i64,
    pub access_token_id: i64,
    pub role_id: i64,
}

/// Link authorization row (link name + role)
#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct DbLinkAuthorization {
    pub id: i64,
    pub uuid: Uuid,
    pub link_name: String,
    pub role_id: i64,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DbCreateLinkAuthorization {
    pub link_name: String,
    pub role_id: i64,
}

/// Issue authorization row (issue + user, no permission level in vanilla)
#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct DbIssueAuthorization {
    pub id: i64,
    pub uuid: Uuid,
    pub user_id: i64,
    pub issue_id: i64,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DbCreateIssueAuthorization {
    pub user_id: i64,
    pub issue_id: i64,
}

/// Database query builder for user authorizations
pub struct UserAuthorizationQueryBuilder;

impl UserAuthorizationQueryBuilder {
    pub fn select_all() -> &'static str {
        "SELECT id, uuid, user_id, project_id, role_id, created_at, updated_at FROM user_authorizations"
    }

    pub fn select_by_id() -> &'static str {
        "SELECT id, uuid, user_id, project_id, role_id, created_at, updated_at FROM user_authorizations WHERE id = $1"
    }

    pub fn select_by_user_id() -> &'static str {
        "SELECT id, uuid, user_id, project_id, role_id, created_at, updated_at FROM user_authorizations WHERE user_id = $1 ORDER BY project_id"
    }

    pub fn select_by_project_id() -> &'static str {
        "SELECT id, uuid, user_id, project_id, role_id, created_at, updated_at FROM user_authorizations WHERE project_id = $1 ORDER BY user_id"
    }

    pub fn select_by_user_and_project() -> &'static str {
        "SELECT id, uuid, user_id, project_id, role_id, created_at, updated_at FROM user_authorizations WHERE user_id = $1 AND project_id = $2"
    }

    pub fn insert() -> &'static str {
        r#"
        INSERT INTO user_authorizations (uuid, user_id, project_id, role_id, created_at, updated_at)
        VALUES ($1, $2, $3, $4, $5, $6)
        RETURNING id
        "#
    }

    pub fn update() -> &'static str {
        r#"
        UPDATE user_authorizations SET
            role_id = COALESCE($2, role_id),
            updated_at = NOW()
        WHERE id = $1
        RETURNING id, uuid, user_id, project_id, role_id, created_at, updated_at
        "#
    }

    pub fn delete() -> &'static str {
        "DELETE FROM user_authorizations WHERE id = $1"
    }
}

/// Database query builder for issue authorizations
pub struct IssueAuthorizationQueryBuilder;

impl IssueAuthorizationQueryBuilder {
    pub fn select_by_id() -> &'static str {
        "SELECT id, uuid, user_id, issue_id, created_at FROM issue_authorizations WHERE id = $1"
    }

    pub fn select_by_user_id() -> &'static str {
        "SELECT id, uuid, user_id, issue_id, created_at FROM issue_authorizations WHERE user_id = $1"
    }

    pub fn select_by_issue_id() -> &'static str {
        "SELECT id, uuid, user_id, issue_id, created_at FROM issue_authorizations WHERE issue_id = $1"
    }

    pub fn insert() -> &'static str {
        r#"
        INSERT INTO issue_authorizations (uuid, user_id, issue_id, created_at)
        VALUES ($1, $2, $3, $4)
        RETURNING id
        "#
    }

    pub fn delete() -> &'static str {
        "DELETE FROM issue_authorizations WHERE id = $1"
    }
}
