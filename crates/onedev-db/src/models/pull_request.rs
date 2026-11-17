//! Pull Request Database Model

use super::*;
use crate::DbResult;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, Postgres, Row};
use uuid::Uuid;

/// Pull request state in the database
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "pull_request_state", rename_all = "lowercase")]
pub enum DbPullRequestState {
    Open,
    Merged,
    Closed,
    Abandoned,
}

impl From<DbPullRequestState> for onedev_core::models::pull_request::PullRequestState {
    fn from(s: DbPullRequestState) -> Self {
        match s {
            DbPullRequestState::Open => onedev_core::models::pull_request::PullRequestState::Open,
            DbPullRequestState::Merged => onedev_core::models::pull_request::PullRequestState::Merged,
            DbPullRequestState::Closed => onedev_core::models::pull_request::PullRequestState::Closed,
            DbPullRequestState::Abandoned => onedev_core::models::pull_request::PullRequestState::Abandoned,
        }
    }
}

impl From<onedev_core::models::pull_request::PullRequestState> for DbPullRequestState {
    fn from(s: onedev_core::models::pull_request::PullRequestState) -> Self {
        match s {
            onedev_core::models::pull_request::PullRequestState::Open => DbPullRequestState::Open,
            onedev_core::models::pull_request::PullRequestState::Merged => DbPullRequestState::Merged,
            onedev_core::models::pull_request::PullRequestState::Closed => DbPullRequestState::Closed,
            onedev_core::models::pull_request::PullRequestState::Abandoned => DbPullRequestState::Abandoned,
        }
    }
}

/// Merge strategy in the database
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "merge_strategy", rename_all = "lowercase")]
pub enum DbMergeStrategy {
    MergeCommit,
    Squash,
    FastForward,
    Rebase,
}

/// Pull request row from database
#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct DbPullRequest {
    pub id: i64,
    pub uuid: Uuid,
    pub project_id: i64,
    pub number: i32,
    pub title: String,
    pub description: Option<String>,
    pub state: DbPullRequestState,
    pub author_id: i64,
    pub source_project_id: i64,
    pub source_branch: String,
    pub target_project_id: i64,
    pub target_branch: String,
    pub merge_strategy: DbMergeStrategy,
    pub merge_commit_sha: Option<String>,
    pub head_commit_sha: String,
    pub base_commit_sha: String,
    pub merge_base_sha: Option<String>,
    pub mergeable: bool,
    pub merge_check_status: Option<String>, // JSON
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub closed_at: Option<DateTime<Utc>>,
    pub merged_at: Option<DateTime<Utc>>,
    // Additional fields
    pub label_ids: Vec<i64>,
    pub reviewer_ids: Vec<i64>,
    pub watcher_ids: Vec<i64>,
    pub draft: bool,
}

/// Pull request creation input for database
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DbCreatePullRequest {
    pub project_id: i64,
    pub title: String,
    pub description: Option<String>,
    pub author_id: i64,
    pub source_project_id: i64,
    pub source_branch: String,
    pub target_project_id: i64,
    pub target_branch: String,
    pub merge_strategy: DbMergeStrategy,
    pub head_commit_sha: String,
    pub base_commit_sha: String,
    pub merge_base_sha: Option<String>,
    pub draft: bool,
}

/// Pull request update input for database
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DbUpdatePullRequest {
    pub title: Option<String>,
    pub description: Option<String>,
    pub state: Option<DbPullRequestState>,
    pub merge_strategy: Option<DbMergeStrategy>,
    pub merge_commit_sha: Option<String>,
    pub head_commit_sha: Option<String>,
    pub mergeable: Option<bool>,
    pub merge_check_status: Option<serde_json::Value>,
    pub draft: Option<bool>,
    pub label_ids: Option<Vec<i64>>,
    pub reviewer_ids: Option<Vec<i64>>,
    pub watcher_ids: Option<Vec<i64>>,
}

impl DbPullRequest {
    /// Convert to core domain model
    pub fn to_core(&self) -> onedev_core::models::pull_request::PullRequest {
        onedev_core::models::pull_request::PullRequest {
            id: self.id,
            uuid: self.uuid,
            project_id: self.project_id,
            number: self.number,
            title: self.title.clone(),
            description: self.description.clone(),
            state: self.state.into(),
            author_id: self.author_id,
            source_project_id: self.source_project_id,
            source_branch: self.source_branch.clone(),
            target_project_id: self.target_project_id,
            target_branch: self.target_branch.clone(),
            merge_strategy: match self.merge_strategy {
                DbMergeStrategy::MergeCommit => onedev_core::models::pull_request::MergeStrategy::MergeCommit,
                DbMergeStrategy::Squash => onedev_core::models::pull_request::MergeStrategy::Squash,
                DbMergeStrategy::FastForward => onedev_core::models::pull_request::MergeStrategy::FastForward,
                DbMergeStrategy::Rebase => onedev_core::models::pull_request::MergeStrategy::Rebase,
            },
            merge_commit_sha: self.merge_commit_sha.clone(),
            head_commit_sha: self.head_commit_sha.clone(),
            base_commit_sha: self.base_commit_sha.clone(),
            merge_base_sha: self.merge_base_sha.clone(),
            mergeable: self.mergeable,
            merge_check_status: self.merge_check_status.clone(),
            created_at: self.created_at,
            updated_at: self.updated_at,
            closed_at: self.closed_at,
            merged_at: self.merged_at,
            label_ids: self.label_ids.clone(),
            reviewer_ids: self.reviewer_ids.clone(),
            watcher_ids: self.watcher_ids.clone(),
            draft: self.draft,
        }
    }
}

/// Database query builder for pull requests
pub struct PullRequestQueryBuilder;

impl PullRequestQueryBuilder {
    pub fn select_all() -> &'static str {
        r#"
        SELECT id, uuid, project_id, number, title, description, state,
               author_id, source_project_id, source_branch, target_project_id,
               target_branch, merge_strategy, merge_commit_sha, head_commit_sha,
               base_commit_sha, merge_base_sha, mergeable, merge_check_status,
               created_at, updated_at, closed_at, merged_at,
               label_ids, reviewer_ids, watcher_ids, draft
        FROM pull_requests
        "#
    }

    pub fn select_by_id() -> &'static str {
        r#"
        SELECT id, uuid, project_id, number, title, description, state,
               author_id, source_project_id, source_branch, target_project_id,
               target_branch, merge_strategy, merge_commit_sha, head_commit_sha,
               base_commit_sha, merge_base_sha, mergeable, merge_check_status,
               created_at, updated_at, closed_at, merged_at,
               label_ids, reviewer_ids, watcher_ids, draft
        FROM pull_requests WHERE id = $1
        "#
    }

    pub fn select_by_project_and_number() -> &'static str {
        r#"
        SELECT id, uuid, project_id, number, title, description, state,
               author_id, source_project_id, source_branch, target_project_id,
               target_branch, merge_strategy, merge_commit_sha, head_commit_sha,
               base_commit_sha, merge_base_sha, mergeable, merge_check_status,
               created_at, updated_at, closed_at, merged_at,
               label_ids, reviewer_ids, watcher_ids, draft
        FROM pull_requests WHERE project_id = $1 AND number = $2
        "#
    }

    pub fn insert() -> &'static str {
        r#"
        INSERT INTO pull_requests (uuid, project_id, number, title, description, state,
                                   author_id, source_project_id, source_branch, target_project_id,
                                   target_branch, merge_strategy, head_commit_sha, base_commit_sha,
                                   merge_base_sha, mergeable, merge_check_status,
                                   created_at, updated_at, label_ids, reviewer_ids,
                                   watcher_ids, draft)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18, $19, $20, $21, $22, $23)
        RETURNING id
        "#
    }

    pub fn update() -> &'static str {
        r#"
        UPDATE pull_requests SET
            title = COALESCE($2, title),
            description = COALESCE($3, description),
            state = COALESCE($4, state),
            merge_strategy = COALESCE($5, merge_strategy),
            merge_commit_sha = COALESCE($6, merge_commit_sha),
            head_commit_sha = COALESCE($7, head_commit_sha),
            mergeable = COALESCE($8, mergeable),
            merge_check_status = COALESCE($9, merge_check_status),
            draft = COALESCE($10, draft),
            label_ids = COALESCE($11, label_ids),
            reviewer_ids = COALESCE($12, reviewer_ids),
            watcher_ids = COALESCE($13, watcher_ids),
            updated_at = NOW(),
            closed_at = CASE WHEN $4 IN ('closed','merged','abandoned')::pull_request_state AND closed_at IS NULL THEN NOW() ELSE closed_at END,
            merged_at = CASE WHEN $4 = 'merged'::pull_request_state AND merged_at IS NULL THEN NOW() ELSE merged_at END
        WHERE id = $1
        RETURNING id, uuid, project_id, number, title, description, state,
                  author_id, source_project_id, source_branch, target_project_id,
                  target_branch, merge_strategy, merge_commit_sha, head_commit_sha,
                  base_commit_sha, merge_base_sha, mergeable, merge_check_status,
                  created_at, updated_at, closed_at, merged_at,
                  label_ids, reviewer_ids, watcher_ids, draft
        "#
    }

    pub fn delete() -> &'static str {
        "DELETE FROM pull_requests WHERE id = $1"
    }
}