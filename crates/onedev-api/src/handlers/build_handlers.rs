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

async fn to_response(state: &AppState, build: &onedev_db::models::build::DbBuild) -> BuildResponse {
    let author = state.user_service.get(build.author_id).await.map(|user| crate::models::users::UserRef {
        id: user.id,
        uuid: user.uuid,
        name: user.name,
        full_name: user.full_name,
        avatar_url: user.avatar_url,
        user_type: user.user_type,
    }).unwrap_or(crate::models::users::UserRef {
        id: build.author_id,
        uuid: uuid::Uuid::nil(),
        name: String::new(),
        full_name: None,
        avatar_url: None,
        user_type: crate::models::users::UserType::Ordinary,
    });
    BuildResponse {
        build: BuildRef {
            id: build.id,
            uuid: build.uuid,
            project_id: build.project_id,
            number: build.number,
            status: crate::services::build_service::status_api(build.status),
            ref_name: build.ref_name.clone(),
            commit_sha: build.commit_sha.clone(),
            author,
            started_at: build.started_at,
            finished_at: build.finished_at,
            created_at: build.created_at,
        },
        pipeline_id: None,
        jobs: Vec::new(),
        variables: Vec::new(),
    }
}

/// GET /api/projects/{project_id}/builds
pub async fn list_builds(
    State(state): State<crate::AppState>,
    Path(project_id): Path<i64>,
    Query(query): Query<BuildQuery>,
) -> Result<Json<PaginatedResponse<BuildRef>>, AuthError> {
    let limit = query.pagination.per_page.max(1) as i64;
    let offset = query.pagination.page as i64 * limit;
    let total = state.build_service.count(project_id).await? as u64;
    let rows = state.build_service.list(project_id, limit, offset).await?;
    let mut refs = Vec::with_capacity(rows.len());
    for row in &rows {
        refs.push(to_response(&state, row).await.build);
    }
    Ok(Json(PaginatedResponse::new(refs, total, query.pagination)))
}

/// POST /api/projects/{project_id}/builds
pub async fn trigger_build(
    State(state): State<crate::AppState>,
    crate::utils::extractors::CurrentSubject(subject): crate::utils::extractors::CurrentSubject,
    Path(project_id): Path<i64>,
    Json(request): Json<TriggerBuildRequest>,
) -> Result<Json<BuildResponse>, AuthError> {
    let variables = request
        .variables
        .unwrap_or_default()
        .into_iter()
        .map(|variable| (variable.key, variable.value))
        .collect();
    let build = state
        .build_service
        .trigger(project_id, subject.effective_user_id().unwrap_or(1), request.ref_name, variables)
        .await?;
    Ok(Json(to_response(&state, &build).await))
}

/// GET /api/projects/{project_id}/builds/{id}
pub async fn get_build(
    State(state): State<crate::AppState>,
    Path((project_id, id)): Path<(i64, i64)>,
) -> Result<Json<BuildResponse>, AuthError> {
    let build = state.build_service.get(project_id, id).await?;
    Ok(Json(to_response(&state, &build).await))
}
