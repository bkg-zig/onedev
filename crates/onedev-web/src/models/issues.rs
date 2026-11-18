//! Issue Models

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Issue {
    pub id: Uuid,
    pub number: u32,
    pub title: String,
    pub description: Option<String>,
    pub state: IssueState,
    pub author: UserRef,
    pub assignee: Option<UserRef>,
    pub labels: Vec<Label>,
    pub milestone: Option<Milestone>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub closed_at: Option<DateTime<Utc>>,
    pub comments_count: u32,
    pub confidential: bool,
    pub due_date: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum IssueState {
    Open,
    Closed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserRef {
    pub id: Uuid,
    pub username: String,
    pub full_name: String,
    pub avatar_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Label {
    pub id: Uuid,
    pub name: String,
    pub color: String,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Milestone {
    pub id: Uuid,
    pub title: String,
    pub description: Option<String>,
    pub due_date: Option<DateTime<Utc>>,
    pub state: MilestoneState,
    pub issues_count: u32,
    pub closed_issues_count: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MilestoneState {
    Active,
    Closed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IssueComment {
    pub id: Uuid,
    pub author: UserRef,
    pub content: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: Option<DateTime<Utc>>,
    pub system: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateIssueForm {
    pub title: String,
    pub description: Option<String>,
    pub assignee_id: Option<Uuid>,
    pub label_ids: Vec<Uuid>,
    pub milestone_id: Option<Uuid>,
    pub confidential: bool,
    pub due_date: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateIssueForm {
    pub title: String,
    pub description: Option<String>,
    pub state: IssueState,
    pub assignee_id: Option<Uuid>,
    pub label_ids: Vec<Uuid>,
    pub milestone_id: Option<Uuid>,
    pub confidential: bool,
    pub due_date: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct IssueListParams {
    pub state: Option<IssueState>,
    pub assignee_id: Option<Uuid>,
    pub label_ids: Vec<Uuid>,
    pub milestone_id: Option<Uuid>,
    pub search: Option<String>,
    pub page: Option<u32>,
    pub per_page: Option<u32>,
}
