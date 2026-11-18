//! Merge Request HTTP Handlers

use crate::AppState;
use crate::errors::AuthError;
use crate::models::pagination::{OffsetPagination, PaginatedResponse};
use axum::{
    extract::{Json, Path, Query, State},
    http::StatusCode,
};
use serde::{Deserialize, Serialize};

/// Merge request state
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum MergeRequestState {
    Open,
    Merged,
    Closed,
    Draft,
    Locked,
}

/// Merge strategy
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum MergeStrategy {
    MergeCommit,
    Squash,
    FastForward,
    RebaseMerge,
}

/// Merge request reference
#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct MergeRequestRef {
    pub id: i64,
    pub uuid: uuid::Uuid,
    pub project_id: i64,
    pub number: i32,
    pub title: String,
    pub state: MergeRequestState,
    pub author: crate::models::users::UserRef,
    pub source_branch: String,
    pub target_branch: String,
    pub source_project_id: i64,
    pub target_project_id: i64,
    pub merge_strategy: MergeStrategy,
    pub draft: bool,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

/// Full merge request response
#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct MergeRequestResponse {
    #[serde(flatten)]
    pub mr: MergeRequestRef,
    pub description: Option<String>,
    pub head_commit_sha: String,
    pub base_commit_sha: String,
    pub merge_base_sha: Option<String>,
    pub mergeable: bool,
    pub merge_check_status: Option<String>,
    pub merge_commit_sha: Option<String>,
    pub labels: Vec<crate::models::labels::LabelResponse>,
    pub reviewers: Vec<crate::models::users::UserRef>,
    pub watchers: Vec<crate::models::users::UserRef>,
}

/// Create merge request request
#[derive(Debug, Clone, Deserialize, utoipa::ToSchema, validator::Validate)]
pub struct CreateMergeRequestRequest {
    #[validate(length(min = 1, max = 255))]
    pub title: String,
    pub description: Option<String>,
    pub source_branch: String,
    pub target_branch: String,
    pub source_project_id: i64,
    pub target_project_id: i64,
    pub merge_strategy: Option<MergeStrategy>,
    pub draft: Option<bool>,
    pub label_ids: Vec<i64>,
    pub reviewer_ids: Vec<i64>,
}

/// Update merge request request
#[derive(Debug, Clone, Deserialize, utoipa::ToSchema, Default)]
pub struct UpdateMergeRequestRequest {
    pub title: Option<String>,
    pub description: Option<String>,
    pub state: Option<MergeRequestState>,
    pub target_branch: Option<String>,
    pub merge_strategy: Option<MergeStrategy>,
    pub draft: Option<bool>,
    pub label_ids: Option<Vec<i64>>,
    pub reviewer_ids: Option<Vec<i64>>,
}

/// Merge request query
#[derive(Debug, Clone, Deserialize, utoipa::ToSchema, Default)]
pub struct MergeRequestQuery {
    pub state: Option<MergeRequestState>,
    pub author_id: Option<i64>,
    pub reviewer_id: Option<i64>,
    pub label_ids: Vec<i64>,
    pub draft: Option<bool>,
    #[serde(flatten)]
    pub pagination: OffsetPagination,
}

/// GET /api/projects/{project_id}/merge-requests
pub async fn list_merge_requests(
    State(_state): State<crate::AppState>,
    Path(_project_id): Path<i64>,
    Query(_query): Query<MergeRequestQuery>,
) -> Result<Json<PaginatedResponse<MergeRequestRef>>, AuthError> {
    Err(AuthError::Internal("Not implemented".to_string()))
}

/// POST /api/projects/{project_id}/merge-requests
pub async fn create_merge_request(
    State(_state): State<crate::AppState>,
    Path(_project_id): Path<i64>,
    Json(_request): Json<CreateMergeRequestRequest>,
) -> Result<Json<MergeRequestResponse>, AuthError> {
    Err(AuthError::Internal("Not implemented".to_string()))
}

/// GET /api/projects/{project_id}/merge-requests/{id}
pub async fn get_merge_request(
    State(_state): State<crate::AppState>,
    Path((_project_id, _id)): Path<(i64, i64)>,
) -> Result<Json<MergeRequestResponse>, AuthError> {
    Err(AuthError::Internal("Not implemented".to_string()))
}

/// PATCH /api/projects/{project_id}/merge-requests/{id}
pub async fn update_merge_request(
    State(_state): State<crate::AppState>,
    Path((_project_id, _id)): Path<(i64, i64)>,
    Json(_request): Json<UpdateMergeRequestRequest>,
) -> Result<Json<MergeRequestResponse>, AuthError> {
    Err(AuthError::Internal("Not implemented".to_string()))
}

/// DELETE /api/projects/{project_id}/merge-requests/{id}
pub async fn delete_merge_request(
    State(_state): State<crate::AppState>,
    Path((_project_id, _id)): Path<(i64, i64)>,
) -> Result<StatusCode, AuthError> {
    Err(AuthError::Internal("Not implemented".to_string()))
}
