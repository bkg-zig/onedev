//! Issue Database Model

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, Row};
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
            onedev_core::models::issue::IssueState::Closed
            | onedev_core::models::issue::IssueState::Resolved
            | onedev_core::models::issue::IssueState::Duplicate
            | onedev_core::models::issue::IssueState::WontFix => DbIssueState::Closed,
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
    pub watcher_ids: Option<Vec<i64>>,
    pub voter_ids: Option<Vec<i64>>,
}

impl DbIssue {
    /// Map the numeric DB priority (0 = lowest .. 5 = critical) to the core enum.
    /// Out-of-range values are clamped.
    pub fn priority_from_i32(value: i32) -> onedev_core::models::issue::IssuePriority {
        use onedev_core::models::issue::IssuePriority;
        match value {
            i32::MIN..=0 => IssuePriority::Lowest,
            1 => IssuePriority::Low,
            2 => IssuePriority::Medium,
            3 => IssuePriority::High,
            4 => IssuePriority::Highest,
            _ => IssuePriority::Critical,
        }
    }

    /// Map the core priority enum to the numeric DB priority (inverse of `priority_from_i32`)
    pub fn priority_to_i32(priority: onedev_core::models::issue::IssuePriority) -> i32 {
        use onedev_core::models::issue::IssuePriority;
        match priority {
            IssuePriority::Lowest => 0,
            IssuePriority::Low => 1,
            IssuePriority::Medium => 2,
            IssuePriority::High => 3,
            IssuePriority::Highest => 4,
            IssuePriority::Critical => 5,
        }
    }

    /// Convert to core domain model.
    ///
    /// The core `Issue` carries fields the schema does not persist (issue type,
    /// confidentiality, custom field values, git refs, start/completion
    /// timestamps, weight, parent/epic links); they get neutral defaults.
    /// DB-only data (`remaining_duration`, `label_ids`, `watcher_ids`,
    /// `voter_ids`) is not part of the core model and is dropped here.
    pub fn to_core(&self) -> onedev_core::models::issue::Issue {
        use onedev_core::models::issue::{ConfidentialityLevel, IssueType};

        onedev_core::models::issue::Issue {
            id: self.id,
            uuid: self.uuid,
            project_id: self.project_id,
            number: i64::from(self.number),
            title: self.title.clone(),
            description: self.description.clone(),
            state: self.state.into(),
            issue_type: IssueType::Task,
            priority: Self::priority_from_i32(self.priority),
            confidentiality: ConfidentialityLevel::Public,
            submitter_id: self.submitter_id,
            assignee_id: self.assignee_id,
            milestone_id: self.milestone_id,
            field_values: serde_json::Value::Object(serde_json::Map::new()),
            branch_name: None,
            commit_sha: None,
            created_at: self.created_at,
            updated_at: self.updated_at,
            closed_at: self.closed_at,
            due_date: self.due_date,
            started_at: None,
            completed_at: None,
            time_estimate: self.estimated_duration.map(i64::from),
            time_spent: Some(i64::from(self.spent_duration)),
            weight: None,
            parent_issue_id: None,
            epic_id: None,
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