//! Dashboard Domain Model

use super::*;

/// Dashboard entity
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Dashboard {
    pub id: i64,
    pub uuid: Uuid,
    pub owner_id: i64,
    pub owner_type: DashboardOwnerType,
    pub name: String,
    pub description: Option<String>,
    pub layout: DashboardLayout,
    pub is_default: bool,
    pub is_public: bool,
    pub widgets: Vec<Widget>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, strum::EnumString, strum::Display,
)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case")]
pub enum DashboardOwnerType {
    User,
    Group,
    Project,
    System,
}

/// Dashboard layout
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct DashboardLayout {
    pub columns: u32,
    pub row_height: u32,
    pub margin: u32,
    pub draggable: bool,
    pub resizable: bool,
}

/// Widget entity
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Widget {
    pub id: i64,
    pub uuid: Uuid,
    pub dashboard_id: i64,
    pub widget_type: WidgetType,
    pub title: String,
    pub config: serde_json::Value,
    pub position: WidgetPosition,
    pub size: WidgetSize,
    pub refresh_interval: Option<u32>, // seconds
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, strum::EnumString, strum::Display,
)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case")]
pub enum WidgetType {
    // Issue widgets
    MyIssues,
    AssignedIssues,
    ReportedIssues,
    IssuesByLabel,
    IssuesByMilestone,
    IssuesByAssignee,
    IssueStatistics,
    IssueTrend,
    // Merge Request widgets
    MyMergeRequests,
    AssignedMergeRequests,
    ReviewRequestedMergeRequests,
    MergeRequestsByStatus,
    MergeRequestStatistics,
    MergeRequestTrend,
    // Build widgets
    MyBuilds,
    ProjectBuilds,
    PipelineStatus,
    BuildStatistics,
    BuildTrend,
    BuildDuration,
    // Git widgets
    RepositoryActivity,
    CommitsByAuthor,
    CommitsByDay,
    BranchesOverview,
    TagsOverview,
    // Package widgets
    PackageStatistics,
    PackageDownloads,
    PackageVulnerabilities,
    // Generic widgets
    Markdown,
    Chart,
    Table,
    List,
    Iframe,
    Custom,
}

/// Widget position
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct WidgetPosition {
    pub x: u32,
    pub y: u32,
}

/// Widget size
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct WidgetSize {
    pub width: u32,
    pub height: u32,
}

/// Dashboard query params
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DashboardQuery {
    pub owner_id: Option<i64>,
    pub owner_type: Option<DashboardOwnerType>,
    pub is_default: Option<bool>,
    pub sort: Option<String>,
    pub order: Option<SortOrder>,
    pub page: u32,
    pub per_page: u32,
}

/// Widget query params
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct WidgetQuery {
    pub dashboard_id: Option<i64>,
    pub widget_type: Option<WidgetType>,
    pub sort: Option<String>,
    pub order: Option<SortOrder>,
    pub page: u32,
    pub per_page: u32,
}

impl Dashboard {
    pub fn is_system(&self) -> bool {
        self.owner_type == DashboardOwnerType::System
    }

    pub fn can_edit(&self, user_id: i64) -> bool {
        self.owner_type == DashboardOwnerType::User && self.owner_id == user_id
    }
}
