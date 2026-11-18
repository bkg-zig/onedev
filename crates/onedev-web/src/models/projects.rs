//! Project Models

use serde::{Deserialize, Serialize};
use uuid::Uuid;
use chrono::{DateTime, Utc};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Project {
    pub id: Uuid,
    pub name: String,
    pub path: String,
    pub description: Option<String>,
    pub visibility: ProjectVisibility,
    pub owner: UserRef,
    pub stars: u32,
    pub forks: u32,
    pub open_issues: u32,
    pub open_mrs: u32,
    pub default_branch: String,
    pub merge_requests_enabled: bool,
    pub issues_enabled: bool,
    pub wiki_enabled: bool,
    pub builds_enabled: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub last_activity_at: Option<DateTime<Utc>>,
    pub archived: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ProjectVisibility {
    Private,
    Internal,
    Public,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserRef {
    pub id: Uuid,
    pub username: String,
    pub full_name: String,
    pub avatar_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectMember {
    pub user: UserRef,
    pub role: ProjectRole,
    pub created_at: DateTime<Utc>,
    pub expires_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ProjectRole {
    Guest,
    Reporter,
    Developer,
    Maintainer,
    Owner,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectSettings {
    pub merge_requests_enabled: bool,
    pub issues_enabled: bool,
    pub wiki_enabled: bool,
    pub builds_enabled: bool,
    pub default_branch: String,
    pub visibility: ProjectVisibility,
    pub only_allow_merge_if_pipeline_succeeds: bool,
    pub only_allow_merge_if_all_discussions_are_resolved: bool,
    pub delete_source_branch_on_merge: bool,
    pub auto_cancel_pending_pipelines: bool,
}