//! Group Domain Model

use super::*;
use crate::utils::*;

/// Group visibility
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, strum::EnumString, strum::Display)]
#[serde(rename_all = "lowercase")]
#[strum(serialize_all = "lowercase")]
pub enum GroupVisibility {
    Private,
    Internal,
    Public,
}

/// Core Group entity
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Group {
    pub id: i64,
    pub uuid: Uuid,
    pub name: String,
    pub path: String, // URL-friendly path
    pub description: Option<String>,
    pub visibility: GroupVisibility,
    pub parent_id: Option<i64>, // For nested groups
    pub owner_id: i64, // User who created the group
    pub avatar_url: Option<String>,
    // Settings
    pub require_two_factor: bool,
    pub two_factor_grace_period: u32, // hours
    pub project_creation_level: ProjectCreationLevel,
    pub subgroup_creation_level: SubgroupCreationLevel,
    pub visibility_level: VisibilityLevel,
    pub shared_runners_minutes: Option<u64>,
    pub extra_shared_runners_minutes: Option<u64>,
    // LDAP/SSO
    pub ldap_cn: Option<String>,
    pub ldap_access: Option<LdapAccess>,
    // Timestamps
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, strum::EnumString, strum::Display)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case")]
pub enum ProjectCreationLevel {
    NoOne,
    Maintainer,
    Owner,
    Developer,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, strum::EnumString, strum::Display)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case")]
pub enum SubgroupCreationLevel {
    Owner,
    Maintainer,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, strum::EnumString, strum::Display)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case")]
pub enum VisibilityLevel {
    Private,
    Internal,
    Public,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, strum::EnumString, strum::Display)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case")]
pub enum LdapAccess {
    NotAllowed,
    Guest,
    Reporter,
    Developer,
    Maintainer,
    Owner,
}

/// Group reference
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GroupRef {
    pub id: i64,
    pub uuid: Uuid,
    pub name: String,
    pub path: String,
    pub full_path: String,
    pub description: Option<String>,
    pub visibility: GroupVisibility,
    pub avatar_url: Option<String>,
}

/// Group creation input
#[derive(Debug, Clone, Serialize, Deserialize, validator::Validate)]
pub struct CreateGroup {
    #[validate(length(min = 1, max = 100))]
    #[validate(regex(path = *crate::utils::validation::GROUP_PATH_REGEX))]
    pub path: String,

    #[validate(length(min = 1, max = 200))]
    pub name: String,

    pub description: Option<String>,
    pub visibility: GroupVisibility,
    pub parent_id: Option<i64>,
}

/// Group update input
#[derive(Debug, Clone, Serialize, Deserialize, validator::Validate, Default)]
pub struct UpdateGroup {
    #[validate(length(min = 1, max = 200))]
    pub name: Option<String>,
    pub description: Option<String>,
    pub visibility: Option<GroupVisibility>,
    pub avatar_url: Option<String>,
    pub require_two_factor: Option<bool>,
    pub two_factor_grace_period: Option<u32>,
    pub project_creation_level: Option<ProjectCreationLevel>,
    pub subgroup_creation_level: Option<SubgroupCreationLevel>,
}

/// Group query params
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GroupQuery {
    pub term: Option<String>,
    pub visibility: Option<GroupVisibility>,
    pub parent_id: Option<i64>,
    pub include_subgroups: bool,
    pub sort: Option<String>,
    pub order: Option<SortOrder>,
    pub page: u32,
    pub per_page: u32,
}

/// Group membership
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GroupMember {
    pub id: i64,
    pub uuid: Uuid,
    pub group_id: i64,
    pub user_id: i64,
    pub access_level: AccessLevel,
    pub expires_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub created_by_id: i64,
    pub user: Option<UserRef>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, strum::EnumString, strum::Display)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case")]
pub enum AccessLevel {
    Guest = 10,
    Reporter = 20,
    Developer = 30,
    Maintainer = 40,
    Owner = 50,
}

impl AccessLevel {
    pub fn is_admin(&self) -> bool {
        *self >= AccessLevel::Maintainer
    }

    pub fn can_manage_members(&self) -> bool {
        *self >= AccessLevel::Owner
    }
}

/// Group with projects
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroupWithProjects {
    #[serde(flatten)]
    pub group: GroupRef,
    pub projects: Vec<ProjectRef>,
    pub subgroups: Vec<GroupRef>,
    pub shared_projects: Vec<ProjectRef>,
}

impl Group {
    pub fn full_path(&self) -> String {
        if let Some(parent_id) = self.parent_id {
            // Would need parent lookup
            format!("parent/{}", self.path)
        } else {
            self.path.clone()
        }
    }

    pub fn web_url(&self, base_url: &str) -> String {
        format!("{}/groups/{}", base_url.trim_end_matches('/'), self.full_path())
    }

    pub fn is_public(&self) -> bool {
        matches!(self.visibility, GroupVisibility::Public)
    }
}