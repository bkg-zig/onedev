//! Pull Request Domain Model

use super::*;
use crate::utils::*;

/// Merge request state
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, strum::EnumString, strum::Display)]
#[serde(rename_all = "lowercase")]
#[strum(serialize_all = "lowercase")]
pub enum MergeRequestState {
    Open,
    Merged,
    Closed,
    Draft,
    Locked,
}

/// Merge strategy
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, strum::EnumString, strum::Display)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case")]
pub enum MergeStrategy {
    MergeCommit,
    Squash,
    FastForward,
    RebaseMerge,
}

/// Core Merge Request entity
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MergeRequest {
    pub id: i64,
    pub uuid: Uuid,
    pub project_id: i64,
    pub number: i64,
    pub title: String,
    pub description: Option<String>,
    pub state: MergeRequestState,
    pub merge_strategy: MergeStrategy,
    pub source_project_id: i64,
    pub source_branch: String,
    pub source_commit_sha: String,
    pub target_project_id: i64,
    pub target_branch: String,
    pub target_commit_sha: String,
    pub submitter_id: i64,
    pub assignee_id: Option<i64>,
    pub reviewer_ids: Vec<i64>,
    pub milestone_id: Option<i64>,
    pub labels: Vec<String>,
    // Merge status
    pub merge_status: MergeStatus,
    pub merge_commit_sha: Option<String>,
    pub merged_at: Option<DateTime<Utc>>,
    pub merged_by_id: Option<i64>,
    pub closed_at: Option<DateTime<Utc>>,
    pub closed_by_id: Option<i64>,
    // Pipeline
    pub head_pipeline_id: Option<i64>,
    pub merge_request_diff_id: Option<i64>,
    // Timestamps
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    // Settings
    pub allow_collaboration: bool,
    pub allow_maintainer_push: bool,
    pub squash_on_merge: bool,
    pub remove_source_branch: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, strum::EnumString, strum::Display)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case")]
pub enum MergeStatus {
    CanBeMerged,
    CannotBeMerged,
    Checking,
    Conflicts,
    PipelineFailed,
    ApprovalsRequired,
    DiscussionsNotResolved,
    MergeCommitNotAllowed,
}

/// Lightweight MR reference
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MergeRequestRef {
    pub id: i64,
    pub uuid: Uuid,
    pub project_id: i64,
    pub project_path: String,
    pub number: i64,
    pub title: String,
    pub state: MergeRequestState,
    pub source_branch: String,
    pub target_branch: String,
    pub author: UserRef,
    pub assignee: Option<UserRef>,
    pub reviewers: Vec<UserRef>,
    pub labels: Vec<LabelRef>,
    pub merge_status: MergeStatus,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// MR creation input
#[derive(Debug, Clone, Serialize, Deserialize, validator::Validate)]
pub struct CreateMergeRequest {
    #[validate(length(min = 1, max = 255))]
    pub title: String,

    pub description: Option<String>,
    pub source_branch: String,
    pub target_branch: String,
    pub target_project_id: Option<i64>, // Default: same project
    pub assignee_id: Option<i64>,
    pub reviewer_ids: Vec<i64>,
    pub milestone_id: Option<i64>,
    pub labels: Vec<String>,
    pub allow_collaboration: bool,
    pub allow_maintainer_push: bool,
    pub remove_source_branch: bool,
    pub squash_on_merge: bool,
}

/// MR update input
#[derive(Debug, Clone, Serialize, Deserialize, validator::Validate, Default)]
pub struct UpdateMergeRequest {
    #[validate(length(min = 1, max = 255))]
    pub title: Option<String>,
    pub description: Option<String>,
    pub state: Option<MergeRequestState>,
    pub target_branch: Option<String>,
    pub assignee_id: Option<Option<i64>>,
    pub reviewer_ids: Option<Vec<i64>>,
    pub milestone_id: Option<Option<i64>>,
    pub labels: Option<Vec<String>>,
    pub allow_collaboration: Option<bool>,
    pub allow_maintainer_push: Option<bool>,
    pub remove_source_branch: Option<bool>,
    pub squash_on_merge: Option<bool>,
}

/// MR query params
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct MergeRequestQuery {
    pub term: Option<String>,
    pub state: Option<MergeRequestState>,
    pub author_id: Option<i64>,
    pub assignee_id: Option<i64>,
    pub reviewer_id: Option<i64>,
    pub milestone_id: Option<i64>,
    pub labels: Vec<String>,
    pub source_branch: Option<String>,
    pub target_branch: Option<String>,
    pub project_id: Option<i64>,
    pub include_forks: bool,
    pub sort: Option<String>,
    pub order: Option<SortOrder>,
    pub page: u32,
    pub per_page: u32,
}

/// MR with relations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MergeRequestWithRelations {
    #[serde(flatten)]
    pub merge_request: MergeRequestRef,
    pub author: UserRef,
    pub assignee: Option<UserRef>,
    pub reviewers: Vec<UserRef>,
    pub labels: Vec<LabelRef>,
    pub milestone: Option<MilestoneRef>,
    pub changes: MergeRequestChanges,
    pub pipeline: Option<BuildRef>,
    pub approvals: Vec<Approval>,
    pub discussions: Vec<Discussion>,
    pub commits: Vec<CommitRef>,
    pub diff_stats: DiffStats,
}

/// MR Changes (Files)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MergeRequestChanges {
    pub files: Vec<DiffFile>,
    pub commits: Vec<CommitRef>,
    pub diff_stats: DiffStats,
}

/// Diff file
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DiffFile {
    pub old_path: String,
    pub new_path: String,
    pub old_mode: Option<String>,
    pub new_mode: Option<String>,
    pub is_new: bool,
    pub is_deleted: bool,
    pub is_renamed: bool,
    pub is_binary: bool,
    pub lines_added: u32,
    pub lines_removed: u32,
    pub diff: Option<String>, // Unified diff
}

/// Diff statistics
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct DiffStats {
    pub files_changed: u32,
    pub lines_added: u32,
    pub lines_removed: u32,
}

/// Commit reference
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CommitRef {
    pub id: String, // SHA
    pub short_id: String,
    pub title: String,
    pub message: Option<String>,
    pub author_name: String,
    pub author_email: String,
    pub author_date: DateTime<Utc>,
    pub committer_name: String,
    pub committer_email: String,
    pub committer_date: DateTime<Utc>,
    pub parent_ids: Vec<String>,
}

// `BuildRef` is defined once in `build.rs`.

/// Approval
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Approval {
    pub user_id: i64,
    pub user: UserRef,
    pub state: ApprovalState,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, strum::EnumString, strum::Display)]
#[serde(rename_all = "lowercase")]
#[strum(serialize_all = "lowercase")]
pub enum ApprovalState {
    Pending,
    Approved,
    Rejected,
}

/// Discussion (thread of comments)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Discussion {
    pub id: i64,
    pub uuid: Uuid,
    pub merge_request_id: i64,
    pub is_resolved: bool,
    pub resolved_by_id: Option<i64>,
    pub resolved_at: Option<DateTime<Utc>>,
    pub notes: Vec<DiscussionNote>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Discussion note (comment)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DiscussionNote {
    pub id: i64,
    pub uuid: Uuid,
    pub discussion_id: i64,
    pub author_id: i64,
    pub author: UserRef,
    pub body: String,
    pub system: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl MergeRequest {
    pub fn reference(&self) -> String {
        format!("!{}", self.number)
    }

    pub fn full_reference(&self, project_path: &str) -> String {
        format!("{}!{}", project_path, self.number)
    }

    pub fn is_open(&self) -> bool {
        matches!(self.state, MergeRequestState::Open | MergeRequestState::Draft)
    }

    pub fn is_merged(&self) -> bool {
        matches!(self.state, MergeRequestState::Merged)
    }

    pub fn is_closed(&self) -> bool {
        matches!(self.state, MergeRequestState::Closed | MergeRequestState::Merged)
    }

    pub fn can_merge(&self) -> bool {
        matches!(self.merge_status, MergeStatus::CanBeMerged)
    }

    pub fn requires_approval(&self) -> bool {
        matches!(self.merge_status, MergeStatus::ApprovalsRequired)
    }
}