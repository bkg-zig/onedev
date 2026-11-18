//! Issue Domain Model

use super::*;

/// Issue state
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, strum::EnumString, strum::Display,
)]
#[serde(rename_all = "lowercase")]
#[strum(serialize_all = "lowercase")]
pub enum IssueState {
    Open,
    Closed,
    Resolved,
    Duplicate,
    WontFix,
}

/// Issue priority
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Serialize,
    Deserialize,
    strum::EnumString,
    strum::Display,
)]
#[serde(rename_all = "lowercase")]
#[strum(serialize_all = "lowercase")]
pub enum IssuePriority {
    Lowest,
    Low,
    Medium,
    High,
    Highest,
    Critical,
}

/// Issue type
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, strum::EnumString, strum::Display,
)]
#[serde(rename_all = "lowercase")]
#[strum(serialize_all = "lowercase")]
pub enum IssueType {
    Task,
    Bug,
    Feature,
    Epic,
    Story,
    Incident,
    Question,
}

/// Core Issue entity
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Issue {
    pub id: i64,
    pub uuid: Uuid,
    pub project_id: i64,
    pub number: i64, // Project-scoped sequential number
    pub title: String,
    pub description: Option<String>,
    pub state: IssueState,
    pub issue_type: IssueType,
    pub priority: IssuePriority,
    pub confidentiality: ConfidentialityLevel,
    pub submitter_id: i64,
    pub assignee_id: Option<i64>,
    pub milestone_id: Option<i64>,
    // Custom fields stored as JSON
    pub field_values: serde_json::Value,
    // Git references
    pub branch_name: Option<String>,
    pub commit_sha: Option<String>,
    // Timestamps
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub closed_at: Option<DateTime<Utc>>,
    pub due_date: Option<DateTime<Utc>>,
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
    // Metrics
    pub time_estimate: Option<i64>, // seconds
    pub time_spent: Option<i64>,    // seconds
    pub weight: Option<i32>,
    // Relations
    pub parent_issue_id: Option<i64>,
    pub epic_id: Option<i64>,
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, strum::EnumString, strum::Display,
)]
#[serde(rename_all = "lowercase")]
#[strum(serialize_all = "lowercase")]
pub enum ConfidentialityLevel {
    Public,
    Confidential,
    Internal,
}

/// Lightweight issue reference
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct IssueRef {
    pub id: i64,
    pub uuid: Uuid,
    pub project_id: i64,
    pub project_path: String,
    pub number: i64,
    pub title: String,
    pub state: IssueState,
    pub issue_type: IssueType,
    pub priority: IssuePriority,
    pub assignee: Option<UserRef>,
    pub labels: Vec<LabelRef>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Issue creation input
#[derive(Debug, Clone, Serialize, Deserialize, validator::Validate)]
pub struct CreateIssue {
    #[validate(length(min = 1, max = 255))]
    pub title: String,

    pub description: Option<String>,
    pub issue_type: Option<IssueType>,
    pub priority: Option<IssuePriority>,
    pub confidentiality: Option<ConfidentialityLevel>,
    pub assignee_id: Option<i64>,
    pub milestone_id: Option<i64>,
    pub labels: Vec<String>,
    pub field_values: Option<serde_json::Value>,
    pub due_date: Option<DateTime<Utc>>,
    pub time_estimate: Option<i64>,
    pub weight: Option<i32>,
    pub parent_issue_id: Option<i64>,
    pub epic_id: Option<i64>,
}

/// Issue update input
#[derive(Debug, Clone, Serialize, Deserialize, validator::Validate, Default)]
pub struct UpdateIssue {
    #[validate(length(min = 1, max = 255))]
    pub title: Option<String>,
    pub description: Option<String>,
    pub state: Option<IssueState>,
    pub issue_type: Option<IssueType>,
    pub priority: Option<IssuePriority>,
    pub confidentiality: Option<ConfidentialityLevel>,
    pub assignee_id: Option<Option<i64>>,
    pub milestone_id: Option<Option<i64>>,
    pub labels: Option<Vec<String>>,
    pub field_values: Option<serde_json::Value>,
    pub due_date: Option<Option<DateTime<Utc>>>,
    pub time_estimate: Option<Option<i64>>,
    pub weight: Option<Option<i32>>,
    pub parent_issue_id: Option<Option<i64>>,
    pub epic_id: Option<Option<i64>>,
}

/// Issue query params
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct IssueQuery {
    pub term: Option<String>,
    pub state: Option<IssueState>,
    pub issue_type: Option<IssueType>,
    pub priority: Option<IssuePriority>,
    pub confidentiality: Option<ConfidentialityLevel>,
    pub assignee_id: Option<i64>,
    pub submitter_id: Option<i64>,
    pub milestone_id: Option<i64>,
    pub labels: Vec<String>,
    pub project_id: Option<i64>,
    pub include_subprojects: bool,
    pub sort: Option<String>,
    pub order: Option<SortOrder>,
    pub page: u32,
    pub per_page: u32,
}

/// Issue with relations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IssueWithRelations {
    #[serde(flatten)]
    pub issue: IssueRef,
    pub submitter: UserRef,
    pub assignee: Option<UserRef>,
    pub labels: Vec<LabelRef>,
    pub milestone: Option<MilestoneRef>,
    pub comments_count: u32,
    pub reactions: HashMap<String, u32>,
    pub linked_issues: Vec<IssueLink>,
    pub related_merge_requests: Vec<MergeRequestRef>,
}

/// Issue link
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct IssueLink {
    pub source_issue_id: i64,
    pub target_issue_id: i64,
    pub link_type: IssueLinkType,
    pub created_at: DateTime<Utc>,
    pub created_by: i64,
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, strum::EnumString, strum::Display,
)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case")]
pub enum IssueLinkType {
    RelatesTo,
    Blocks,
    IsBlockedBy,
    Duplicates,
    IsDuplicatedBy,
    ParentOf,
    ChildOf,
    PartOf,
    HasPart,
}

/// Milestone reference
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MilestoneRef {
    pub id: i64,
    pub uuid: Uuid,
    pub title: String,
    pub description: Option<String>,
    pub state: MilestoneState,
    pub due_date: Option<DateTime<Utc>>,
    pub start_date: Option<DateTime<Utc>>,
    pub project_id: i64,
    pub issues_count: u32,
    pub open_issues_count: u32,
    pub closed_issues_count: u32,
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, strum::EnumString, strum::Display,
)]
#[serde(rename_all = "lowercase")]
#[strum(serialize_all = "lowercase")]
pub enum MilestoneState {
    Active,
    Closed,
}

/// Label reference
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LabelRef {
    pub id: i64,
    pub uuid: Uuid,
    pub name: String,
    pub color: String, // Hex color
    pub description: Option<String>,
    pub project_id: i64,
    pub is_scoped: bool,
}

// `MergeRequestRef` is defined once in `pull_request.rs`.

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, strum::EnumString, strum::Display,
)]
#[serde(rename_all = "lowercase")]
#[strum(serialize_all = "lowercase")]
pub enum MergeRequestState {
    Open,
    Merged,
    Closed,
    Draft,
}

impl Issue {
    pub fn reference(&self) -> String {
        format!("#{}", self.number)
    }

    pub fn full_reference(&self, project_path: &str) -> String {
        format!("{}#{}", project_path, self.number)
    }

    pub fn is_open(&self) -> bool {
        matches!(self.state, IssueState::Open)
    }

    pub fn is_closed(&self) -> bool {
        matches!(
            self.state,
            IssueState::Closed | IssueState::Resolved | IssueState::Duplicate | IssueState::WontFix
        )
    }

    pub fn is_confidential(&self) -> bool {
        matches!(self.confidentiality, ConfidentialityLevel::Confidential)
    }
}
