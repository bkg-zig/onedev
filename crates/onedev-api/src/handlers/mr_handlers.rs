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

async fn to_response(state: &AppState, mr: &onedev_db::models::pull_request::DbPullRequest) -> MergeRequestResponse {
    let author = state.user_service.get(mr.author_id).await.map(|user| crate::models::users::UserRef {
        id: user.id,
        uuid: user.uuid,
        name: user.name,
        full_name: user.full_name,
        avatar_url: user.avatar_url,
        user_type: user.user_type,
    }).unwrap_or(crate::models::users::UserRef {
        id: mr.author_id,
        uuid: uuid::Uuid::nil(),
        name: String::new(),
        full_name: None,
        avatar_url: None,
        user_type: crate::models::users::UserType::Ordinary,
    });
    MergeRequestResponse {
        mr: MergeRequestRef {
            id: mr.id,
            uuid: mr.uuid,
            project_id: mr.project_id,
            number: mr.number,
            title: mr.title.clone(),
            state: crate::services::mr_service::MergeRequestService::state_api(mr.state),
            author,
            source_branch: mr.source_branch.clone(),
            target_branch: mr.target_branch.clone(),
            source_project_id: mr.source_project_id,
            target_project_id: mr.target_project_id,
            merge_strategy: crate::services::mr_service::MergeRequestService::strategy_api(mr.merge_strategy),
            draft: false,
            created_at: mr.created_at,
            updated_at: mr.updated_at,
        },
        description: mr.description.clone(),
        head_commit_sha: mr.head_commit_sha.clone(),
        base_commit_sha: mr.base_commit_sha.clone(),
        merge_base_sha: mr.merge_base_sha.clone(),
        mergeable: mr.mergeable,
        merge_check_status: mr.merge_check_status.as_ref().and_then(|v| v.as_str().map(str::to_owned)),
        merge_commit_sha: mr.merge_commit_sha.clone(),
        labels: Vec::new(),
        reviewers: Vec::new(),
        watchers: Vec::new(),
    }
}

/// GET /api/projects/{project_id}/merge-requests
pub async fn list_merge_requests(
    State(state): State<crate::AppState>,
    Path(project_id): Path<i64>,
    Query(query): Query<MergeRequestQuery>,
) -> Result<Json<PaginatedResponse<MergeRequestRef>>, AuthError> {
    let limit = query.pagination.per_page.max(1) as i64;
    let offset = query.pagination.page as i64 * limit;
    let total = state.mr_service.count(project_id).await? as u64;
    let rows = state.mr_service.list(project_id, limit, offset).await?;
    let mut refs = Vec::with_capacity(rows.len());
    for row in &rows {
        refs.push(to_response(&state, row).await.mr);
    }
    Ok(Json(PaginatedResponse::new(refs, total, query.pagination)))
}

/// POST /api/projects/{project_id}/merge-requests
pub async fn create_merge_request(
    State(state): State<crate::AppState>,
    crate::utils::extractors::CurrentSubject(subject): crate::utils::extractors::CurrentSubject,
    Path(project_id): Path<i64>,
    Json(request): Json<CreateMergeRequestRequest>,
) -> Result<Json<MergeRequestResponse>, AuthError> {
    let author_id = subject.effective_user_id().unwrap_or(1);
    let created = state
        .mr_service
        .create(
            project_id,
            onedev_db::models::pull_request::DbCreatePullRequest {
                project_id,
                title: request.title,
                description: request.description,
                author_id,
                source_project_id: request.source_project_id,
                source_branch: request.source_branch,
                target_project_id: request.target_project_id,
                target_branch: request.target_branch,
                merge_strategy: request
                    .merge_strategy
                    .map(crate::services::mr_service::MergeRequestService::strategy_db)
                    .unwrap_or(onedev_db::models::pull_request::DbMergeStrategy::MergeCommit),
                head_commit_sha: String::new(),
                base_commit_sha: String::new(),
                merge_base_sha: None,
                draft: request.draft.unwrap_or(false),
                label_ids: request.label_ids,
                reviewer_ids: request.reviewer_ids,
                watcher_ids: Vec::new(),
            },
        )
        .await?;
    Ok(Json(to_response(&state, &created).await))
}

/// GET /api/projects/{project_id}/merge-requests/{id}
pub async fn get_merge_request(
    State(state): State<crate::AppState>,
    Path((project_id, id)): Path<(i64, i64)>,
) -> Result<Json<MergeRequestResponse>, AuthError> {
    let mr = state.mr_service.get(project_id, id).await?;
    Ok(Json(to_response(&state, &mr).await))
}

/// PATCH /api/projects/{project_id}/merge-requests/{id}
pub async fn update_merge_request(
    State(state): State<crate::AppState>,
    Path((project_id, id)): Path<(i64, i64)>,
    Json(request): Json<UpdateMergeRequestRequest>,
) -> Result<Json<MergeRequestResponse>, AuthError> {
    let updated = state
        .mr_service
        .update(
            project_id,
            id,
            onedev_db::models::pull_request::DbUpdatePullRequest {
                title: request.title,
                description: request.description,
                state: request.state.map(crate::services::mr_service::MergeRequestService::state_db),
                merge_strategy: request
                    .merge_strategy
                    .map(crate::services::mr_service::MergeRequestService::strategy_db),
                ..Default::default()
            },
        )
        .await?;
    Ok(Json(to_response(&state, &updated).await))
}

/// DELETE /api/projects/{project_id}/merge-requests/{id}
pub async fn delete_merge_request(
    State(state): State<crate::AppState>,
    Path((project_id, id)): Path<(i64, i64)>,
) -> Result<StatusCode, AuthError> {
    state.mr_service.delete(project_id, id).await?;
    Ok(StatusCode::NO_CONTENT)
}
