//! Group HTTP Handlers — wired to `GroupService`.

use axum::{
    extract::{State, Path, Query, Json},
    http::StatusCode,
};
use crate::models::groups::*;
use crate::models::pagination::PaginatedResponse;
use crate::errors::AuthError;

/// GET /api/groups
pub async fn list_groups(
    State(state): State<crate::AppState>,
    Query(query): Query<GroupQuery>,
) -> Result<Json<PaginatedResponse<GroupListItem>>, AuthError> {
    let limit = query.pagination.per_page.max(1) as i64;
    let offset = query.pagination.page as i64 * limit;
    let total = state.group_service.count().await? as u64;
    let items = state.group_service.list(&query).await?;
    let items = items.into_iter().skip(offset as usize).take(limit as usize).collect();
    Ok(Json(PaginatedResponse::new(items, total, query.pagination)))
}

/// POST /api/groups
pub async fn create_group(
    State(state): State<crate::AppState>,
    Json(request): Json<CreateGroupRequest>,
) -> Result<Json<GroupResponse>, AuthError> {
    Ok(Json(state.group_service.create(request).await?))
}

/// GET /api/groups/{id}
pub async fn get_group(
    State(state): State<crate::AppState>,
    Path(id): Path<i64>,
) -> Result<Json<GroupResponse>, AuthError> {
    Ok(Json(state.group_service.get(id).await?))
}

/// PATCH /api/groups/{id}
pub async fn update_group(
    State(state): State<crate::AppState>,
    Path(id): Path<i64>,
    Json(request): Json<UpdateGroupRequest>,
) -> Result<Json<GroupResponse>, AuthError> {
    Ok(Json(state.group_service.update(id, request).await?))
}

/// DELETE /api/groups/{id}
pub async fn delete_group(
    State(state): State<crate::AppState>,
    Path(id): Path<i64>,
) -> Result<StatusCode, AuthError> {
    state.group_service.delete(id).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// GET /api/groups/{id}/members
pub async fn list_group_members(
    State(state): State<crate::AppState>,
    Path(id): Path<i64>,
) -> Result<Json<Vec<GroupMemberResponse>>, AuthError> {
    Ok(Json(state.group_service.members(id).await?))
}

/// POST /api/groups/{id}/members
pub async fn add_group_member(
    State(state): State<crate::AppState>,
    Path(id): Path<i64>,
    Json(request): Json<AddGroupMemberRequest>,
) -> Result<Json<GroupMemberResponse>, AuthError> {
    Ok(Json(state.group_service.add_member(id, request).await?))
}

/// GET /api/groups/{id}/members/{user_id}
pub async fn get_group_member(
    State(state): State<crate::AppState>,
    Path((_group_id, _user_id)): Path<(i64, i64)>,
) -> Result<Json<GroupMemberResponse>, AuthError> {
    let members = state.group_service.members(_group_id).await?;
    members
        .into_iter()
        .find(|member| member.user.id == _user_id)
        .map(Json)
        .ok_or_else(|| AuthError::Internal("group member not found".to_owned()))
}

/// PATCH /api/groups/{id}/members/{user_id}
pub async fn update_group_member(
    State(state): State<crate::AppState>,
    Path((group_id, user_id)): Path<(i64, i64)>,
    Json(request): Json<UpdateGroupMemberRequest>,
) -> Result<Json<GroupMemberResponse>, AuthError> {
    if let Some(level) = request.access_level {
        state.group_service.remove_member(group_id, user_id).await?;
        return Ok(Json(
            state
                .group_service
                .add_member(
                    group_id,
                    AddGroupMemberRequest {
                        user_id,
                        access_level: level,
                        expires_at: None,
                    },
                )
                .await?,
        ));
    }
    get_group_member(State(state), Path((group_id, user_id))).await
}

/// DELETE /api/groups/{id}/members/{user_id}
pub async fn remove_group_member(
    State(state): State<crate::AppState>,
    Path((group_id, user_id)): Path<(i64, i64)>,
) -> Result<StatusCode, AuthError> {
    state.group_service.remove_member(group_id, user_id).await?;
    Ok(StatusCode::NO_CONTENT)
}
