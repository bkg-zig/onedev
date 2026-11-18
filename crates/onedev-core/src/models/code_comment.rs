//! Code Comment Domain Model

use super::*;

/// Code comment entity
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CodeComment {
    pub id: i64,
    pub uuid: Uuid,
    pub project_id: i64,
    pub merge_request_id: Option<i64>,
    pub commit_sha: String,
    pub file_path: String,
    pub old_line: Option<u32>,
    pub new_line: Option<u32>,
    pub user_id: i64,
    pub body: String,
    pub body_html: Option<String>,
    pub comment_type: CodeCommentType,
    pub status: CodeCommentStatus,
    // Thread
    pub parent_id: Option<i64>,
    pub thread_id: Option<i64>,
    pub resolved: bool,
    pub resolved_by_id: Option<i64>,
    pub resolved_at: Option<DateTime<Utc>>,
    // Reactions
    pub reactions: HashMap<String, Vec<i64>>, // emoji -> user_ids
    // Timestamps
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, strum::EnumString, strum::Display,
)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case")]
pub enum CodeCommentType {
    Diff,
    Commit,
    File,
    Line,
    Suggestion,
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, strum::EnumString, strum::Display,
)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case")]
pub enum CodeCommentStatus {
    Active,
    Resolved,
    Outdated,
    Hidden,
}

/// Code comment reply
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CodeCommentReply {
    pub id: i64,
    pub uuid: Uuid,
    pub comment_id: i64,
    pub user_id: i64,
    pub body: String,
    pub body_html: Option<String>,
    pub reactions: HashMap<String, Vec<i64>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
}

/// Compare context (for MR comments)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CompareContext {
    pub merge_request_id: i64,
    pub base_commit_sha: String,
    pub head_commit_sha: String,
    pub base_branch: String,
    pub head_branch: String,
}

/// Code comment creation input
#[derive(Debug, Clone, Serialize, Deserialize, validator::Validate)]
pub struct CreateCodeComment {
    pub project_id: i64,
    pub merge_request_id: Option<i64>,
    pub commit_sha: String,
    pub file_path: String,
    pub old_line: Option<u32>,
    pub new_line: Option<u32>,
    #[validate(length(min = 1, max = 65536))]
    pub body: String,
    pub parent_id: Option<i64>,
    pub thread_id: Option<i64>,
}

/// Code comment update input
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UpdateCodeComment {
    pub body: Option<String>,
    pub resolved: Option<bool>,
}

/// Code comment query params
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CodeCommentQuery {
    pub project_id: Option<i64>,
    pub merge_request_id: Option<i64>,
    pub commit_sha: Option<String>,
    pub file_path: Option<String>,
    pub user_id: Option<i64>,
    pub status: Option<CodeCommentStatus>,
    pub resolved: Option<bool>,
    pub sort: Option<String>,
    pub order: Option<SortOrder>,
    pub page: u32,
    pub per_page: u32,
}

impl CodeComment {
    pub fn is_thread_root(&self) -> bool {
        self.parent_id.is_none()
    }

    pub fn is_resolved(&self) -> bool {
        self.resolved || self.status == CodeCommentStatus::Resolved
    }

    pub fn is_outdated(&self) -> bool {
        self.status == CodeCommentStatus::Outdated
    }

    pub fn reaction_count(&self) -> u32 {
        self.reactions.values().map(|v| v.len() as u32).sum()
    }
}
