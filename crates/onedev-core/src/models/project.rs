//! Project Domain Model

use super::*;
use crate::utils::*;

/// Project visibility
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, strum::EnumString, strum::Display)]
#[serde(rename_all = "lowercase")]
#[strum(serialize_all = "lowercase")]
pub enum ProjectVisibility {
    Private,
    Internal,
    Public,
}

/// Project status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, strum::EnumString, strum::Display)]
#[serde(rename_all = "lowercase")]
#[strum(serialize_all = "lowercase")]
pub enum ProjectStatus {
    Active,
    Archived,
    Maintenance,
    Deleted,
}

/// Core Project entity
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Project {
    pub id: i64,
    pub uuid: Uuid,
    pub name: String,
    pub path: String, // URL-friendly path (lowercase, hyphens)
    pub description: Option<String>,
    pub readme: Option<String>,
    pub visibility: ProjectVisibility,
    pub status: ProjectStatus,
    pub owner_id: i64, // User or Group ID
    pub owner_type: OwnerType,
    pub parent_id: Option<i64>, // For nested projects (forks, subgroups)
    pub forked_from_id: Option<i64>,
    pub default_branch: String,
    pub avatar_url: Option<String>,
    // Features
    pub issues_enabled: bool,
    pub merge_requests_enabled: bool,
    pub builds_enabled: bool,
    pub wiki_enabled: bool,
    pub snippets_enabled: bool,
    pub packs_enabled: bool,
    pub pages_enabled: bool,
    pub container_registry_enabled: bool,
    // Settings
    pub only_allow_merge_if_pipeline_succeeds: bool,
    pub only_allow_merge_if_all_discussions_resolved: bool,
    pub remove_source_branch_after_merge: bool,
    pub enable_merge_commit: bool,
    pub enable_squash_merge: bool,
    pub enable_fast_forward_merge: bool,
    pub squash_merge_template: Option<String>,
    // CI/CD
    pub build_timeout: u32, // minutes
    pub build_coverage_regex: Option<String>,
    // Timestamps
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
    pub last_activity_at: Option<DateTime<Utc>>,
}

/// Owner type for project ownership
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, strum::EnumString, strum::Display)]
#[serde(rename_all = "lowercase")]
#[strum(serialize_all = "lowercase")]
pub enum OwnerType {
    User,
    Group,
}

/// Lightweight project reference
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProjectRef {
    pub id: i64,
    pub uuid: Uuid,
    pub name: String,
    pub path: String,
    pub full_path: String, // Includes parent paths
    pub description: Option<String>,
    pub visibility: ProjectVisibility,
    pub avatar_url: Option<String>,
    pub default_branch: String,
}

/// Project creation input
#[derive(Debug, Clone, Serialize, Deserialize, validator::Validate)]
pub struct CreateProject {
    #[validate(length(min = 1, max = 100))]
    #[validate(regex(path = *crate::utils::validation::PROJECT_PATH_REGEX))]
    pub path: String,

    #[validate(length(min = 1, max = 200))]
    pub name: String,

    pub description: Option<String>,
    pub visibility: ProjectVisibility,
    pub initialize_with_readme: bool,
    pub template_name: Option<String>,
    pub import_url: Option<String>,
    pub mirror: bool,
}

/// Project update input
#[derive(Debug, Clone, Serialize, Deserialize, validator::Validate, Default)]
pub struct UpdateProject {
    #[validate(length(min = 1, max = 200))]
    pub name: Option<String>,

    pub description: Option<String>,
    pub readme: Option<String>,
    pub visibility: Option<ProjectVisibility>,
    pub status: Option<ProjectStatus>,
    pub default_branch: Option<String>,
    pub avatar_url: Option<String>,
    pub issues_enabled: Option<bool>,
    pub merge_requests_enabled: Option<bool>,
    pub builds_enabled: Option<bool>,
    pub wiki_enabled: Option<bool>,
    pub snippets_enabled: Option<bool>,
    pub packs_enabled: Option<bool>,
    pub pages_enabled: Option<bool>,
    pub container_registry_enabled: Option<bool>,
}

/// Project query params
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ProjectQuery {
    pub term: Option<String>,
    pub visibility: Option<ProjectVisibility>,
    pub status: Option<ProjectStatus>,
    pub owner_id: Option<i64>,
    pub include_subgroups: bool,
    pub sort: Option<String>,
    pub order: Option<SortOrder>,
    pub page: u32,
    pub per_page: u32,
}

/// Project with statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectWithStats {
    #[serde(flatten)]
    pub project: ProjectRef,
    pub stats: ProjectStats,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ProjectStats {
    pub issues_count: u64,
    pub open_issues_count: u64,
    pub merge_requests_count: u64,
    pub open_merge_requests_count: u64,
    pub builds_count: u64,
    pub storage_size: u64,
    pub repository_size: u64,
    pub wiki_size: u64,
    pub packages_size: u64,
    pub forks_count: u64,
    pub stars_count: u64,
}

impl Project {
    pub fn full_path(&self) -> String {
        if let Some(parent_id) = self.parent_id {
            // Would need parent lookup - simplified here
            format!("parent/{}", self.path)
        } else {
            self.path.clone()
        }
    }

    pub fn is_fork(&self) -> bool {
        self.forked_from_id.is_some()
    }

    pub fn is_archived(&self) -> bool {
        matches!(self.status, ProjectStatus::Archived)
    }

    pub fn is_public(&self) -> bool {
        matches!(self.visibility, ProjectVisibility::Public)
    }

    pub fn web_url(&self, base_url: &str) -> String {
        format!("{}/{}", base_url.trim_end_matches('/'), self.full_path())
    }

    pub fn git_http_url(&self, base_url: &str) -> String {
        format!("{}/{}.git", base_url.trim_end_matches('/'), self.full_path())
    }

    pub fn git_ssh_url(&self, ssh_host: &str, ssh_port: u16) -> String {
        if ssh_port == 22 {
            format!("git@{}:{}.git", ssh_host, self.full_path())
        } else {
            format!("ssh://git@{}:{}/{}.git", ssh_host, ssh_port, self.full_path())
        }
    }
}