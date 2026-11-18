//! Build HTTP Handlers

use crate::AppState;
use crate::errors::AuthError;
use crate::models::pagination::{OffsetPagination, PaginatedResponse};
use axum::{
    extract::{Json, Path, Query, State},
    http::StatusCode,
};
use serde::{Deserialize, Serialize};

/// Build status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum BuildStatus {
    Pending,
    Running,
    Success,
    Failed,
    Canceled,
    Skipped,
}

/// Build reference
#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct BuildRef {
    pub id: i64,
    pub uuid: uuid::Uuid,
    pub project_id: i64,
    pub number: i32,
    pub status: BuildStatus,
    pub ref_name: String,
    pub commit_sha: String,
    pub author: crate::models::users::UserRef,
    pub started_at: Option<chrono::DateTime<chrono::Utc>>,
    pub finished_at: Option<chrono::DateTime<chrono::Utc>>,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

/// Full build response
#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct BuildResponse {
    #[serde(flatten)]
    pub build: BuildRef,
    pub pipeline_id: Option<i64>,
    pub jobs: Vec<BuildJobResponse>,
    pub variables: Vec<BuildVariableResponse>,
}

/// Build job response
#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct BuildJobResponse {
    pub id: i64,
    pub uuid: uuid::Uuid,
    pub build_id: i64,
    pub name: String,
    pub stage: String,
    pub status: BuildStatus,
    pub runner_id: Option<i64>,
    pub started_at: Option<chrono::DateTime<chrono::Utc>>,
    pub finished_at: Option<chrono::DateTime<chrono::Utc>>,
    pub duration: Option<i64>,
    pub allow_failure: bool,
}

/// Build variable response
#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct BuildVariableResponse {
    pub key: String,
    pub value: String,
    pub variable_type: String,
    pub protected: bool,
    pub masked: bool,
}

/// Trigger build request
#[derive(Debug, Clone, Deserialize, utoipa::ToSchema)]
pub struct TriggerBuildRequest {
    pub ref_name: String,
    pub variables: Option<Vec<BuildVariableRequest>>,
    pub trigger_token: Option<String>,
}

/// Build variable request
#[derive(Debug, Clone, Deserialize, utoipa::ToSchema)]
pub struct BuildVariableRequest {
    pub key: String,
    pub value: String,
    pub variable_type: Option<String>,
    pub protected: Option<bool>,
    pub masked: Option<bool>,
}

/// Build query
#[derive(Debug, Clone, Deserialize, utoipa::ToSchema, Default)]
pub struct BuildQuery {
    pub status: Option<BuildStatus>,
    pub ref_name: Option<String>,
    pub author_id: Option<i64>,
    #[serde(flatten)]
    pub pagination: OffsetPagination,
}

/// GET /api/projects/{project_id}/builds
pub async fn list_builds(
    State(_state): State<crate::AppState>,
    Path(_project_id): Path<i64>,
    Query(_query): Query<BuildQuery>,
) -> Result<Json<PaginatedResponse<BuildRef>>, AuthError> {
    Err(AuthError::Internal("Not implemented".to_string()))
}

/// POST /api/projects/{project_id}/builds
pub async fn trigger_build(
    State(_state): State<crate::AppState>,
    Path(_project_id): Path<i64>,
    Json(_request): Json<TriggerBuildRequest>,
) -> Result<Json<BuildResponse>, AuthError> {
    Err(AuthError::Internal("Not implemented".to_string()))
}

/// GET /api/projects/{project_id}/builds/{id}
pub async fn get_build(
    State(_state): State<crate::AppState>,
    Path((_project_id, _id)): Path<(i64, i64)>,
) -> Result<Json<BuildResponse>, AuthError> {
    Err(AuthError::Internal("Not implemented".to_string()))
}
