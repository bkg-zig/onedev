//! Project HTTP Handlers — wired to `ProjectService`.

use crate::errors::AuthError;
use crate::models::pagination::PaginatedResponse;
use crate::models::projects::*;
use crate::utils::extractors::CurrentSubject;
use axum::{
    extract::{Json, Path, Query, State},
    http::StatusCode,
};

/// Owner id for create/fork: the current user, falling back to root.
fn owner_id(subject: &onedev_auth::Subject) -> i64 {
    subject.effective_user_id().unwrap_or(1)
}

/// GET /api/projects
pub async fn list_projects(
    State(state): State<crate::AppState>,
    Query(query): Query<ProjectQuery>,
) -> Result<Json<PaginatedResponse<ProjectListItem>>, AuthError> {
    let limit = query.pagination.per_page.max(1) as i64;
    let offset = query.pagination.page as i64 * limit;
    let total = state.project_service.count().await? as u64;
    let items = state.project_service.list(limit, offset).await?;
    Ok(Json(PaginatedResponse::new(items, total, query.pagination)))
}

/// POST /api/projects
pub async fn create_project(
    State(state): State<crate::AppState>,
    CurrentSubject(subject): CurrentSubject,
    Json(request): Json<CreateProjectRequest>,
) -> Result<Json<ProjectResponse>, AuthError> {
    Ok(Json(
        state
            .project_service
            .create(owner_id(&subject), request)
            .await?,
    ))
}

/// GET /api/projects/{id}
pub async fn get_project(
    State(state): State<crate::AppState>,
    Path(id): Path<i64>,
) -> Result<Json<ProjectResponse>, AuthError> {
    Ok(Json(state.project_service.get(id).await?))
}

/// PATCH /api/projects/{id}
pub async fn update_project(
    State(state): State<crate::AppState>,
    Path(id): Path<i64>,
    Json(request): Json<UpdateProjectRequest>,
) -> Result<Json<ProjectResponse>, AuthError> {
    Ok(Json(state.project_service.update(id, request).await?))
}

/// DELETE /api/projects/{id}
pub async fn delete_project(
    State(state): State<crate::AppState>,
    Path(id): Path<i64>,
) -> Result<StatusCode, AuthError> {
    state.project_service.delete(id).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// POST /api/projects/{id}/fork
pub async fn fork_project(
    State(state): State<crate::AppState>,
    CurrentSubject(subject): CurrentSubject,
    Path(id): Path<i64>,
    Json(request): Json<ForkProjectRequest>,
) -> Result<Json<ProjectResponse>, AuthError> {
    Ok(Json(
        state
            .project_service
            .fork(owner_id(&subject), id, request)
            .await?,
    ))
}

/// POST /api/projects/{id}/transfer
pub async fn transfer_project(
    State(state): State<crate::AppState>,
    Path(id): Path<i64>,
    Json(request): Json<TransferProjectRequest>,
) -> Result<Json<ProjectResponse>, AuthError> {
    Ok(Json(state.project_service.transfer(id, request).await?))
}

/// POST /api/projects/{id}/star
pub async fn star_project(
    State(state): State<crate::AppState>,
    Path(id): Path<i64>,
) -> Result<Json<ProjectStarResponse>, AuthError> {
    let project = state.project_service.get(id).await?;
    Ok(Json(ProjectStarResponse {
        starred: true,
        star_count: project.star_count + 1,
    }))
}

/// POST /api/projects/{id}/unstar
pub async fn unstar_project(
    State(state): State<crate::AppState>,
    Path(id): Path<i64>,
) -> Result<Json<ProjectStarResponse>, AuthError> {
    let project = state.project_service.get(id).await?;
    Ok(Json(ProjectStarResponse {
        starred: false,
        star_count: project.star_count.saturating_sub(1),
    }))
}

/// GET /api/projects/{id}/members
pub async fn list_members(
    State(state): State<crate::AppState>,
    Path(id): Path<i64>,
) -> Result<Json<Vec<ProjectMemberResponse>>, AuthError> {
    Ok(Json(state.project_service.members(id).await?))
}

/// POST /api/projects/{id}/members
pub async fn add_member(
    State(state): State<crate::AppState>,
    Path(id): Path<i64>,
    Json(request): Json<AddProjectMemberRequest>,
) -> Result<Json<ProjectMemberResponse>, AuthError> {
    Ok(Json(
        state
            .project_service
            .add_member(id, request.user_id, request.access_level)
            .await?,
    ))
}

/// GET /api/projects/{id}/members/{user_id}
pub async fn get_member(
    State(state): State<crate::AppState>,
    Path((project_id, user_id)): Path<(i64, i64)>,
) -> Result<Json<ProjectMemberResponse>, AuthError> {
    Ok(Json(
        state.project_service.member(project_id, user_id).await?,
    ))
}

/// PATCH /api/projects/{id}/members/{user_id}
pub async fn update_member(
    State(state): State<crate::AppState>,
    Path((project_id, user_id)): Path<(i64, i64)>,
    Json(request): Json<UpdateProjectMemberRequest>,
) -> Result<Json<ProjectMemberResponse>, AuthError> {
    if let Some(level) = request.access_level {
        state
            .project_service
            .remove_member(project_id, user_id)
            .await?;
        return Ok(Json(
            state
                .project_service
                .add_member(project_id, user_id, level)
                .await?,
        ));
    }
    Ok(Json(
        state.project_service.member(project_id, user_id).await?,
    ))
}

/// DELETE /api/projects/{id}/members/{user_id}
pub async fn remove_member(
    State(state): State<crate::AppState>,
    Path((project_id, user_id)): Path<(i64, i64)>,
) -> Result<StatusCode, AuthError> {
    state
        .project_service
        .remove_member(project_id, user_id)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}
