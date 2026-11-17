//! Project API models

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use chrono::{DateTime, Utc};
use uuid::Uuid;
use validator::Validate;

/// Project visibility
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum ProjectVisibility {
    Private,
    Internal,
    Public,
}

/// Project status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum ProjectStatus {
    Active,
    Archived,
    PendingDelete,
}

/// Project reference (lightweight)
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ProjectRef {
    pub id: i64,
    pub uuid: Uuid,
    pub name: String,
    pub path: String,
    pub full_path: String,
    pub description: Option<String>,
    pub visibility: ProjectVisibility,
    pub status: ProjectStatus,
    pub avatar_url: Option<String>,
    pub star_count: u32,
    pub fork_count: u32,
}

/// Full project response
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ProjectResponse {
    pub id: i64,
    pub uuid: Uuid,
    pub name: String,
    pub path: String,
    pub full_path: String,
    pub description: Option<String>,
    pub visibility: ProjectVisibility,
    pub status: ProjectStatus,
    pub owner_id: i64,
    pub owner_type: super::users::UserType,
    pub parent_id: Option<i64>,
    pub avatar_url: Option<String>,
    pub star_count: u32,
    pub fork_count: u32,
    pub open_issues_count: u32,
    pub open_merge_requests_count: u32,
    pub last_activity_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub default_branch: String,
    pub features: ProjectFeatures,
    pub settings: ProjectSettings,
}

/// Project features
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, Default)]
pub struct ProjectFeatures {
    pub issues: bool,
    pub merge_requests: bool,
    pub wiki: bool,
    pub snippets: bool,
    pub builds: bool,
    pub container_registry: bool,
    pub packages: bool,
    pub pages: bool,
    pub analytics: bool,
}

/// Project settings
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, Default)]
pub struct ProjectSettings {
    pub merge_method: Option<String>,
    pub auto_close_referenced_issues: bool,
    pub only_allow_merge_if_pipeline_succeeds: bool,
    pub only_allow_merge_if_all_discussions_are_resolved: bool,
    pub remove_source_branch_after_merge: bool,
    pub printing_merge_request_link_enabled: bool,
    pub default_branch_protection: DefaultBranchProtection,
}

/// Default branch protection
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, Default)]
pub struct DefaultBranchProtection {
    pub enabled: bool,
    pub push_access_level: Option<String>,
    pub merge_access_level: Option<String>,
    pub unprotect_access_level: Option<String>,
}

/// Project list item
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ProjectListItem {
    #[serde(flatten)]
    pub project: ProjectRef,
    pub owner: super::users::UserRef,
    pub namespace: Option<super::groups::GroupRef>,
    pub last_activity_at: Option<DateTime<Utc>>,
}

/// Create project request
#[derive(Debug, Clone, Deserialize, Validate, ToSchema)]
pub struct CreateProjectRequest {
    #[validate(length(min = 1, max = 100))]
    #[schema(min_length = 1, max_length = 100)]
    pub name: String,

    #[validate(length(min = 1, max = 100))]
    #[schema(min_length = 1, max_length = 100)]
    pub path: String,

    #[validate(length(max = 5000))]
    #[schema(max_length = 5000)]
    pub description: Option<String>,

    #[schema(default = "private")]
    pub visibility: ProjectVisibility,

    pub initialize_with_readme: Option<bool>,
    pub default_branch: Option<String>,
    pub group_id: Option<i64>,
    pub template_project_id: Option<i64>,
}

/// Update project request
#[derive(Debug, Clone, Deserialize, Validate, ToSchema, Default)]
pub struct UpdateProjectRequest {
    #[validate(length(min = 1, max = 100))]
    #[schema(min_length = 1, max_length = 100)]
    pub name: Option<String>,

    #[validate(length(max = 5000))]
    #[schema(max_length = 5000)]
    pub description: Option<String>,

    pub visibility: Option<ProjectVisibility>,
    pub status: Option<ProjectStatus>,
    pub avatar_url: Option<String>,
    pub default_branch: Option<String>,
    pub features: Option<ProjectFeatures>,
    pub settings: Option<ProjectSettings>,
}

/// Project query parameters
#[derive(Debug, Clone, Deserialize, Serialize, ToSchema, Default)]
pub struct ProjectQuery {
    pub term: Option<String>,
    pub visibility: Option<ProjectVisibility>,
    pub status: Option<ProjectStatus>,
    pub group_id: Option<i64>,
    pub owned: Option<bool>,
    pub starred: Option<bool>,
    pub member: Option<bool>,
    #[serde(flatten)]
    pub pagination: super::pagination::OffsetPagination,
    pub sort: Option<String>,
    pub order: Option<super::pagination::SortOrder>,
}

/// Sort order
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema, Default)]
#[serde(rename_all = "lowercase")]
pub enum SortOrder {
    #[default]
    Asc,
    Desc,
}

/// Fork project request
#[derive(Debug, Clone, Deserialize, Validate, ToSchema)]
pub struct ForkProjectRequest {
    pub name: Option<String>,
    pub path: Option<String>,
    pub namespace_id: Option<i64>,
    pub description: Option<String>,
}

/// Transfer project request
#[derive(Debug, Clone, Deserialize, Validate, ToSchema)]
pub struct TransferProjectRequest {
    pub namespace_id: i64,
}

/// Project star response
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ProjectStarResponse {
    pub starred: bool,
    pub star_count: u32,
}

/// Project member response
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ProjectMemberResponse {
    pub user: super::users::UserRef,
    pub role: super::groups::AccessLevel,
    pub expires_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

/// Add project member request
#[derive(Debug, Clone, Deserialize, Validate, ToSchema)]
pub struct AddProjectMemberRequest {
    pub user_id: i64,
    pub access_level: super::groups::AccessLevel,
    pub expires_at: Option<DateTime<Utc>>,
}

/// Update project member request
#[derive(Debug, Clone, Deserialize, Validate, ToSchema, Default)]
pub struct UpdateProjectMemberRequest {
    pub access_level: Option<super::groups::AccessLevel>,
    pub expires_at: Option<Option<DateTime<Utc>>>,
}