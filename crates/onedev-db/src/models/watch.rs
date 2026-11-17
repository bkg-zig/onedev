//! Watch/Subscription Database Model

use super::*;
use crate::DbResult;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, Postgres, Row};
use uuid::Uuid;

/// Watch row from database
#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct DbWatch {
    pub id: i64,
    pub uuid: Uuid,
    pub user_id: i64,
    pub watchable_type: DbWatchableType,
    pub watchable_id: i64,
    pub created_at: DateTime<Utc>,
}

/// Watchable type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "watchable_type", rename_all = "lowercase")]
pub enum DbWatchableType {
    Project,
    Issue,
    PullRequest,
    Build,
    Pack,
    Workspace,
}

/// Watch creation input for database
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DbCreateWatch {
    pub user_id: i64,
    pub watchable_type: DbWatchableType,
    pub watchable_id: i64,
}

/// Database query builder for watches
pub struct WatchQueryBuilder;

impl WatchQueryBuilder {
    pub fn select_all() -> &'static str {
        r#"
        SELECT id, uuid, user_id, watchable_type, watchable_id, created_at
        FROM watches
        "#
    }

    pub fn select_by_id() -> &'static str {
        r#"
        SELECT id, uuid, user_id, watchable_type, watchable_id, created_at
        FROM watches WHERE id = $1
        "#
    }

    pub fn select_by_user_id() -> &'static str {
        r#"
        SELECT id, uuid, user_id, watchable_type, watchable_id, created_at
        FROM watches WHERE user_id = $1
        ORDER BY created_at DESC
        "#
    }

    pub fn select_by_watchable() -> &'static str {
        r#"
        SELECT id, uuid, user_id, watchable_type, watchable_id, created_at
        FROM watches WHERE watchable_type = $1 AND watchable_id = $2
        "#
    }

    pub fn select_by_user_and_watchable() -> &'static str {
        r#"
        SELECT id, uuid, user_id, watchable_type, watchable_id, created_at
        FROM watches WHERE user_id = $1 AND watchable_type = $2 AND watchable_id = $3
        "#
    }

    pub fn insert() -> &'static str {
        r#"
        INSERT INTO watches (uuid, user_id, watchable_type, watchable_id, created_at)
        VALUES ($1, $2, $3, $4, $5)
        RETURNING id
        "#
    }

    pub fn delete() -> &'static str {
        "DELETE FROM watches WHERE id = $1"
    }

    pub fn delete_by_user_and_watchable() -> &'static str {
        "DELETE FROM watches WHERE user_id = $1 AND watchable_type = $2 AND watchable_id = $3"
    }
}

/// Query subscription row from database
#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct DbQuerySubscription {
    pub id: i64,
    pub uuid: Uuid,
    pub user_id: i64,
    pub query_name: String,
    pub query_type: DbQueryType,
    pub project_id: Option<i64>,
    pub created_at: DateTime<Utc>,
}

/// Query type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "query_type", rename_all = "lowercase")]
pub enum DbQueryType {
    Issue,
    PullRequest,
    Build,
    Pack,
    Workspace,
    Commit,
    CodeComment,
}

/// Database query builder for query subscriptions
pub struct QuerySubscriptionQueryBuilder;

impl QuerySubscriptionQueryBuilder {
    pub fn select_by_user_id() -> &'static str {
        r#"
        SELECT id, uuid, user_id, query_name, query_type, project_id, created_at
        FROM query_subscriptions WHERE user_id = $1
        "#
    }

    pub fn select_by_user_and_query() -> &'static str {
        r#"
        SELECT id, uuid, user_id, query_name, query_type, project_id, created_at
        FROM query_subscriptions WHERE user_id = $1 AND query_name = $2 AND query_type = $3
        "#
    }

    pub fn insert() -> &'static str {
        r#"
        INSERT INTO query_subscriptions (uuid, user_id, query_name, query_type, project_id, created_at)
        VALUES ($1, $2, $3, $4, $5, $6)
        RETURNING id
        "#
    }

    pub fn delete() -> &'static str {
        "DELETE FROM query_subscriptions WHERE id = $1"
    }
}