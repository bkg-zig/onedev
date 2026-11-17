//! Group API models

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use chrono::{DateTime, Utc};
use uuid::Uuid;
use validator::Validate;

/// Group visibility
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum GroupVisibility {
    Private,
    Internal,
    Public,
}

/// Group reference (lightweight)
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct GroupRef {
    pub id: i64,
    pub uuid: Uuid,
    pub name: String,
    pub path: String,
    pub full_path: String,
    pub description: Option<String>,
    pub visibility: GroupVisibility,
    pub avatar_url: Option<String>,
    pub project_count: u32,
    pub subgroup_count: u32,
}

/// Full group response
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct GroupResponse {
    pub id: i64,
    pub uuid: Uuid,
    pub name: String,
    pub path: String,
    pub full_path: String,
    pub description: Option<String>,
    pub visibility: GroupVisibility,
    pub parent_id: Option<i64>,
    pub owner_id: i64,
    pub avatar_url: Option<String>,
    pub project_count: u32,
    pub subgroup_count: u32,
    pub require_two_factor: bool,
    pub two_factor_grace_period: u32,
    pub project_creation_level: ProjectCreationLevel,
    pub subgroup_creation_level: SubgroupCreationLevel,
    pub visibility_level: VisibilityLevel,
    pub shared_runners_minutes: Option<u64>,
    pub extra_shared_runners_minutes: Option<u64>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Project creation level
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ProjectCreationLevel {
    NoOne,
    Maintainer,
    Owner,
    Developer,
}

/// Subgroup creation level
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum SubgroupCreationLevel {
    Owner,
    Maintainer,
}

/// Visibility level
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum VisibilityLevel {
    Private,
    Internal,
    Public,
}

/// Group list item
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct GroupListItem {
    #[serde(flatten)]
    pub group: GroupRef,
    pub owner: super::users::UserRef,
}

/// Create group request
#[derive(Debug, Clone, Deserialize, Validate, ToSchema)]
pub struct CreateGroupRequest {
    #[validate(length(min = 1, max = 100))]
    #[schema(min_length = 1, max_length = 100)]
    pub path: String,

    #[validate(length(min = 1, max = 200))]
    #[schema(min_length = 1, max_length = 200)]
    pub name: String,

    #[validate(length(max = 5000))]
    #[schema(max_length = 5000)]
    pub description: Option<String>,

    #[schema(default = "private")]
    pub visibility: GroupVisibility,

    pub parent_id: Option<i64>,
}

/// Update group request
#[derive(Debug, Clone, Deserialize, Validate, ToSchema, Default)]
pub struct UpdateGroupRequest {
    #[validate(length(min = 1, max = 200))]
    #[schema(min_length = 1, max_length = 200)]
    pub name: Option<String>,

    #[validate(length(max = 5000))]
    #[schema(max_length = 5000)]
    pub description: Option<String>,

    pub visibility: Option<GroupVisibility>,
    pub avatar_url: Option<String>,
    pub require_two_factor: Option<bool>,
    pub two_factor_grace_period: Option<u32>,
    pub project_creation_level: Option<ProjectCreationLevel>,
    pub subgroup_creation_level: Option<SubgroupCreationLevel>,
}

/// Group query parameters
#[derive(Debug, Clone, Deserialize, Serialize, ToSchema, Default)]
pub struct GroupQuery {
    pub term: Option<String>,
    pub visibility: Option<GroupVisibility>,
    pub parent_id: Option<i64>,
    pub include_subgroups: Option<bool>,
    #[serde(flatten)]
    pub pagination: super::pagination::OffsetPagination,
    pub sort: Option<String>,
    pub order: Option<super::pagination::SortOrder>,
}

/// Group member response
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct GroupMemberResponse {
    pub user: super::users::UserRef,
    pub access_level: AccessLevel,
    pub expires_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

/// Access level
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum AccessLevel {
    Guest = 10,
    Reporter = 20,
    Developer = 30,
    Maintainer = 40,
    Owner = 50,
}

/// Add group member request
#[derive(Debug, Clone, Deserialize, Validate, ToSchema)]
pub struct AddGroupMemberRequest {
    pub user_id: i64,
    pub access_level: AccessLevel,
    pub expires_at: Option<DateTime<Utc>>,
}

/// Update group member request
#[derive(Debug, Clone, Deserialize, Validate, ToSchema, Default)]
pub struct UpdateGroupMemberRequest {
    pub access_level: Option<AccessLevel>,
    pub expires_at: Option<Option<DateTime<Utc>>>,
}