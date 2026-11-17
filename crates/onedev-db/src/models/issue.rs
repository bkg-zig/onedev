//! Issue Database Model

use super::*;
use crate::DbResult;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, Postgres, Row};
use uuid::Uuid;

/// Issue state in the database
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "issue_state", rename_all = "lowercase")]
pub enum DbIssueState {
    Open,
    Closed,
}

impl From<DbIssueState> for onedev_core::models::issue::IssueState {
    fn from(s: DbIssueState) -> Self {
        match s {
            DbIssueState::Open => onedev_core::models::issue::IssueState::Open,
            DbIssueState::Closed => onedev_core::models::issue::IssueState::Closed,
        }
    }
}

impl From<onedev_core::models::issue::IssueState> for DbIssueState {
    fn from(s: onedev_core::models::issue::IssueState) -> Self {
        match s {
            onedev_core::models::issue::IssueState::Open => DbIssueState::Open,
            onedev_core::models::issue::IssueState::Closed => DbIssueState::Closed,
        }
    }
}

/// Issue row from database
#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct DbIssue {
    pub id: i64,
    pub uuid: Uuid,
    pub project_id: i64,
    pub number: i32,
    pub title: String,
    pub description: Option<String>,
    pub state: DbIssueState,
    pub submitter_id: i64,
    pub assignee_id: Option<i64>,
    pub priority: i32,
    pub milestone_id: Option<i64>,
    pub estimated_duration: Option<i32>,
    pub remaining_duration: Option<i32>,
    pub spent_duration: i32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub closed_at: Option<DateTime<Utc>>,
    pub due_date: Option<DateTime<Utc>>,
    // Additional fields
    pub label_ids: Vec<i64>,
    pub watcher_ids: Vec<i64>,
    pub voter_ids: Vec<i64>,
}

/// Issue creation input for database
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DbCreateIssue {
    pub project_id: i64,
    pub title: String,
    pub description: Option<String>,
    pub submitter_id: i64,
    pub assignee_id: Option<i64>,
    pub priority: i32,
    pub milestone_id: Option<i64>,
    pub estimated_duration: Option<i32>,
    pub due_date: Option<DateTime<Utc>>,
    pub label_ids: Vec<i64>,
}

/// Issue update input for database
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DbUpdateIssue {
    pub title: Option<String>,
    pub description: Option<String>,
    pub state: Option<DbIssueState>,
    pub assignee_id: Option<i64>,
    pub priority: Option<i32>,
    pub milestone_id: Option<i64>,
    pub estimated_duration: Option<i32>,
    pub remaining_duration: Option<i32>,
    pub spent_duration: Option<i32>,
    pub due_date: Option<DateTime<Utc>>,
    pub label_ids: Option<Vec<i64>>,
}

impl DbIssue {
    /// Convert to core domain model
    pub fn to_core(&self) -> onedev_core::models::issue::Issue {
        onedev_core::models::issue::Issue {
            id: self.id,
            uuid: self.uuid,
            project_id: self.project_id,
            number: self.number,
            title: self.title.clone(),
            description: self.description.clone(),
            state: self.state.into(),
            submitter_id: self.submitter_id,
            assignee_id: self.assignee_id,
            priority: self.priority,
            milestone_id: self.milestone_id,
            estimated_duration: self.estimated_duration,
            remaining_duration: self.remaining_duration,
            spent_duration: self.spent_duration,
            created_at: self.created_at,
            updated_at: self.updated_at,
            closed_at: self.closed_at,
            due_date: self.due_date,
            label_ids: self.label_ids.clone(),
            watcher_ids: self.watcher_ids.clone(),
            voter_ids: self.voter_ids.clone(),
        }
    }
}

/// Database query builder for issues
pub struct IssueQueryBuilder;

impl IssueQueryBuilder {
    pub fn select_all() -> &'static str {
        r#"
        SELECT id, uuid, project_id, number, title, description, state,
               submitter_id, assignee_id, priority, milestone_id,
               estimated_duration, remaining_duration, spent_duration,
               created_at, updated_at, closed_at, due_date,
               label_ids, watcher_ids, voter_ids
        FROM issues
        "#
    }

    pub fn select_by_id() -> &'static str {
        r#"
        SELECT id, uuid, project_id, number, title, description, state,
               submitter_id, assignee_id, priority, milestone_id,
               estimated_duration, remaining_duration, spent_duration,
               created_at, updated_at, closed_at, due_date,
               label_ids, watcher_ids, voter_ids
        FROM issues WHERE id = $1
        "#
    }

    pub fn select_by_project_and_number() -> &'static str {
        r#"
        SELECT id, uuid, project_id, number, title, description, state,
               submitter_id, assignee_id, priority, milestone_id,
               estimated_duration, remaining_duration, spent_duration,
               created_at, updated_at, closed_at, due_date,
               label_ids, watcher_ids, voter_ids
        FROM issues WHERE project_id = $1 AND number = $2
        "#
    }

    pub fn insert() -> &'static str {
        r#"
        INSERT INTO issues (uuid, project_id, number, title, description, state,
                            submitter_id, assignee_id, priority, milestone_id,
                            estimated_duration, remaining_duration, spent_duration,
                            created_at, updated_at, due_date, label_ids, watcher_ids, voter_ids)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18, $19)
        RETURNING id
        "#
    }

    pub fn update() -> &'static str {
        r#"
        UPDATE issues SET
            title = COALESCE($2, title),
            description = COALESCE($3, description),
            state = COALESCE($4, state),
            assignee_id = COALESCE($5, assignee_id),
            priority = COALESCE($6, priority),
            milestone_id = COALESCE($7, milestone_id),
            estimated_duration = COALESCE($8, estimated_duration),
            remaining_duration = COALESCE($9, remaining_duration),
            spent_duration = COALESCE($10, spent_duration),
            due_date = COALESCE($11, due_date),
            label_ids = COALESCE($12, label_ids),
            watcher_ids = COALESCE($13, watcher_ids),
            voter_ids = COALESCE($14, voter_ids),
            updated_at = NOW(),
            closed_at = CASE WHEN $4 = 'closed'::issue_state AND closed_at IS NULL THEN NOW() ELSE closed_at END
        WHERE id = $1
        RETURNING id, uuid, project_id, number, title, description, state,
                  submitter_id, assignee_id, priority, milestone_id,
                  estimated_duration, remaining_duration, spent_duration,
                  created_at, updated_at, closed_at, due_date,
                  label_ids, watcher_ids, voter_ids
        "#
    }

    pub fn delete() -> &'static str {
        "DELETE FROM issues WHERE id = $1"
    }
}