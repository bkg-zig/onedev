//! Issue HTTP Handlers

use crate::AppState;
use crate::errors::AuthError;
use crate::models::pagination::{OffsetPagination, PaginatedResponse};
use axum::{
    extract::{Json, Path, Query, State},
    http::StatusCode,
};
use serde::{Deserialize, Serialize};

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

async fn to_response(state: &AppState, issue: &onedev_db::models::issue::DbIssue) -> IssueResponse {
    let author = state.issue_service.author_ref(issue.submitter_id).await;
    let assignee = match issue.assignee_id {
        Some(id) => Some(state.issue_service.author_ref(id).await),
        None => None,
    };
    IssueResponse {
        issue: IssueRef {
            id: issue.id,
            uuid: issue.uuid,
            project_id: issue.project_id,
            number: issue.number,
            title: issue.title.clone(),
            state: crate::services::issue_service::issue_state_api(issue.state),
            author,
            assignee,
            created_at: issue.created_at,
            updated_at: issue.updated_at,
        },
        description: issue.description.clone(),
        labels: Vec::new(),
        milestone: None,
        priority: match onedev_db::models::issue::DbIssue::priority_from_i32(issue.priority) {
            onedev_core::models::issue::IssuePriority::Lowest => IssuePriority::Lowest,
            onedev_core::models::issue::IssuePriority::Low => IssuePriority::Low,
            onedev_core::models::issue::IssuePriority::Medium => IssuePriority::Medium,
            onedev_core::models::issue::IssuePriority::High => IssuePriority::High,
            onedev_core::models::issue::IssuePriority::Highest => IssuePriority::Highest,
            onedev_core::models::issue::IssuePriority::Critical => IssuePriority::Critical,
        },
        issue_type: IssueType::Issue,
        confidential: false,
        due_date: issue.due_date,
        time_estimate: issue.estimated_duration,
        time_spent: Some(issue.spent_duration),
    }
}

fn to_ref(issue: &IssueResponse) -> IssueRef {
    issue.issue.clone()
}

/// GET /api/projects/{project_id}/issues
pub async fn list_issues(
    State(state): State<crate::AppState>,
    Path(project_id): Path<i64>,
    Query(query): Query<IssueQuery>,
) -> Result<Json<PaginatedResponse<IssueRef>>, AuthError> {
    let limit = query.pagination.per_page.max(1) as i64;
    let offset = query.pagination.page as i64 * limit;
    let total = state.issue_service.count(project_id).await? as u64;
    let rows = state.issue_service.list(project_id, limit, offset).await?;
    let mut refs = Vec::with_capacity(rows.len());
    for row in &rows {
        refs.push(to_ref(&to_response(&state, row).await));
    }
    Ok(Json(PaginatedResponse::new(refs, total, query.pagination)))
}

/// POST /api/projects/{project_id}/issues
pub async fn create_issue(
    State(state): State<crate::AppState>,
    Path(project_id): Path<i64>,
    Json(request): Json<CreateIssueRequest>,
) -> Result<Json<IssueResponse>, AuthError> {
    let created = state
        .issue_service
        .create(project_id, 1, request.title, request.description)
        .await?;
    Ok(Json(to_response(&state, &created).await))
}

/// GET /api/projects/{project_id}/issues/{id}
pub async fn get_issue(
    State(state): State<crate::AppState>,
    Path((project_id, id)): Path<(i64, i64)>,
) -> Result<Json<IssueResponse>, AuthError> {
    let issue = state.issue_service.get(project_id, id).await?;
    Ok(Json(to_response(&state, &issue).await))
}

/// PATCH /api/projects/{project_id}/issues/{id}
pub async fn update_issue(
    State(state): State<crate::AppState>,
    Path((project_id, id)): Path<(i64, i64)>,
    Json(request): Json<UpdateIssueRequest>,
) -> Result<Json<IssueResponse>, AuthError> {
    let updated = state
        .issue_service
        .update(
            project_id,
            id,
            onedev_db::models::issue::DbUpdateIssue {
                title: request.title,
                description: request.description,
                state: request.state.map(crate::services::issue_service::issue_state_db),
                // DbUpdateIssue uses COALESCE semantics (None keeps the
                // old value), so clearing a field is not possible yet.
                assignee_id: request.assignee_id.unwrap_or(None),
                milestone_id: request.milestone_id.unwrap_or(None),
                due_date: request.due_date.unwrap_or(None),
                ..Default::default()
            },
        )
        .await?;
    Ok(Json(to_response(&state, &updated).await))
}

/// DELETE /api/projects/{project_id}/issues/{id}
pub async fn delete_issue(
    State(state): State<crate::AppState>,
    Path((project_id, id)): Path<(i64, i64)>,
) -> Result<StatusCode, AuthError> {
    state.issue_service.delete(project_id, id).await?;
    Ok(StatusCode::NO_CONTENT)
}
