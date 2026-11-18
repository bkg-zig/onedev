//! Dashboard Models

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DashboardStats {
    pub projects_count: u32,
    pub issues_assigned: u32,
    pub issues_created: u32,
    pub merge_requests_open: u32,
    pub builds_running: u32,
    pub builds_pending: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActivityItem {
    pub id: Uuid,
    pub activity_type: ActivityType,
    pub title: String,
    pub project: String,
    pub author: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ActivityType {
    IssueCreated,
    IssueUpdated,
    IssueClosed,
    MrCreated,
    MrUpdated,
    MrMerged,
    MrClosed,
    BuildStarted,
    BuildCompleted,
    BuildFailed,
    CommitPushed,
    ReleasePublished,
    CommentAdded,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserProject {
    pub id: Uuid,
    pub name: String,
    pub path: String,
    pub description: Option<String>,
    pub visibility: String,
    pub stars: u32,
    pub forks: u32,
    pub last_activity: DateTime<Utc>,
    pub owner: ProjectOwner,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectOwner {
    pub username: String,
    pub avatar_url: Option<String>,
}
