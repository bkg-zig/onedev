//! Merge Request Models

use serde::{Deserialize, Serialize};
use uuid::Uuid;
use chrono::{DateTime, Utc};
use crate::models::projects::UserRef;
use crate::models::git::CommitRef;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MergeRequest {
    pub id: Uuid,
    pub number: u32,
    pub title: String,
    pub description: Option<String>,
    pub state: MrState,
    pub author: UserRef,
    pub assignee: Option<UserRef>,
    pub reviewers: Vec<UserRef>,
    pub source_branch: String,
    pub target_branch: String,
    pub draft: bool,
    pub merge_status: MergeStatus,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub merged_at: Option<DateTime<Utc>>,
    pub merged_by: Option<UserRef>,
    pub commits_count: u32,
    pub changes_count: u32,
    pub head_commit: Option<CommitRef>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MrState {
    Open,
    Merged,
    Closed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MergeStatus {
    CanBeMerged,
    CannotBeMerged,
    Checking,
    Conflicts,
    Draft,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MrCommit {
    pub sha: String,
    pub short_sha: String,
    pub message: String,
    pub author: UserRef,
    pub authored_at: DateTime<Utc>,
    pub verified: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MrChange {
    pub old_path: String,
    pub new_path: String,
    pub status: ChangeStatus,
    pub lines_added: u32,
    pub lines_removed: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ChangeStatus {
    Added,
    Modified,
    Deleted,
    Renamed,
    Copied,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MrComment {
    pub id: Uuid,
    pub author: UserRef,
    pub content: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: Option<DateTime<Utc>>,
    pub comment_type: CommentType,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CommentType {
    Comment,
    Review,
    System,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateMrForm {
    pub title: String,
    pub description: Option<String>,
    pub source_branch: String,
    pub target_branch: String,
    pub assignee_id: Option<Uuid>,
    pub reviewer_ids: Vec<Uuid>,
    pub draft: bool,
}