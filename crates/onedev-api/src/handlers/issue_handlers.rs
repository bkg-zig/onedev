//! Issue HTTP Handlers

use axum::{
    extract::{State, Path, Query, Json},
    http::StatusCode,
};
use serde::{Deserialize, Serialize};
use crate::models::issues::*;
use crate::models::pagination::{OffsetPagination, PaginatedResponse};
use crate::errors::AuthError;
use crate::AppState;

/// Issue state
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum IssueState {
    Open,
    Closed,
    Resolved,
    Duplicate,
    WontFix,
}

/// Issue reference
#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct IssueRef {
    pub id: i64,
    pub uuid: uuid::Uuid,
    pub project_id: i64,
    pub number: i32,
    pub title: String,
    pub state: IssueState,
    pub author: crate::models::users::UserRef,
    pub assignee: Option<crate::models::users::UserRef>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

/// Full issue response
#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct IssueResponse {
    #[serde(flatten)]
    pub issue: IssueRef,
    pub description: Option<String>,
    pub labels: Vec<crate::models::labels::LabelResponse>,
    pub milestone: Option<crate::models::milestones::MilestoneResponse>,
    pub priority: IssuePriority,
    pub issue_type: IssueType,
    pub confidential: bool,
    pub due_date: Option<chrono::DateTime<chrono::Utc>>,
    pub time_estimate: Option<i32>,
    pub time_spent: Option<i32>,
}

/// Issue priority
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum IssuePriority {
    Lowest,
    Low,
    Medium,
    High,
    Highest,
    Critical,
}

/// Issue type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum IssueType {
    Issue,
    Bug,
    Task,
    Epic,
}

/// Create issue request
#[derive(Debug, Clone, Deserialize, utoipa::ToSchema, validator::Validate)]
pub struct CreateIssueRequest {
    #[validate(length(min = 1, max = 255))]
    pub title: String,
    pub description: Option<String>,
    pub assignee_id: Option<i64>,
    pub milestone_id: Option<i64>,
    pub label_ids: Vec<i64>,
    pub priority: Option<IssuePriority>,
    pub issue_type: Option<IssueType>,
    pub confidential: Option<bool>,
    pub due_date: Option<chrono::DateTime<chrono::Utc>>,
}

/// Update issue request
#[derive(Debug, Clone, Deserialize, utoipa::ToSchema, Default)]
pub struct UpdateIssueRequest {
    pub title: Option<String>,
    pub description: Option<String>,
    pub state: Option<IssueState>,
    pub assignee_id: Option<Option<i64>>,
    pub milestone_id: Option<Option<i64>>,
    pub label_ids: Option<Vec<i64>>,
    pub priority: Option<IssuePriority>,
    pub issue_type: Option<IssueType>,
    pub confidential: Option<bool>,
    pub due_date: Option<Option<chrono::DateTime<chrono::Utc>>>,
}

/// Issue query
#[derive(Debug, Clone, Deserialize, utoipa::ToSchema, Default)]
pub struct IssueQuery {
    pub state: Option<IssueState>,
    pub assignee_id: Option<i64>,
    pub author_id: Option<i64>,
    pub milestone_id: Option<i64>,
    pub label_ids: Vec<i64>,
    pub confidential: Option<bool>,
    #[serde(flatten)]
    pub pagination: OffsetPagination,
}

/// GET /api/projects/{project_id}/issues
pub async fn list_issues(
    State(_state): State<crate::AppState>,
    Path(_project_id): Path<i64>,
    Query(_query): Query<IssueQuery>,
) -> Result<Json<PaginatedResponse<IssueRef>>, AuthError> {
    Err(AuthError::Internal("Not implemented".to_string()))
}

/// POST /api/projects/{project_id}/issues
pub async fn create_issue(
    State(_state): State<crate::AppState>,
    Path(_project_id): Path<i64>,
    Json(_request): Json<CreateIssueRequest>,
) -> Result<Json<IssueResponse>, AuthError> {
    Err(AuthError::Internal("Not implemented".to_string()))
}

/// GET /api/projects/{project_id}/issues/{id}
pub async fn get_issue(
    State(_state): State<crate::AppState>,
    Path((_project_id, _id)): Path<(i64, i64)>,
) -> Result<Json<IssueResponse>, AuthError> {
    Err(AuthError::Internal("Not implemented".to_string()))
}

/// PATCH /api/projects/{project_id}/issues/{id}
pub async fn update_issue(
    State(_state): State<crate::AppState>,
    Path((_project_id, _id)): Path<(i64, i64)>,
    Json(_request): Json<UpdateIssueRequest>,
) -> Result<Json<IssueResponse>, AuthError> {
    Err(AuthError::Internal("Not implemented".to_string()))
}

/// DELETE /api/projects/{project_id}/issues/{id}
pub async fn delete_issue(
    State(_state): State<crate::AppState>,
    Path((_project_id, _id)): Path<(i64, i64)>,
) -> Result<StatusCode, AuthError> {
    Err(AuthError::Internal("Not implemented".to_string()))
}