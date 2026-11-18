//! Group HTTP Handlers

use crate::AppState;
use crate::errors::AuthError;
use crate::models::groups::*;
use crate::models::pagination::{OffsetPagination, PaginatedResponse};
use crate::models::users::UserRef;
use axum::{
    extract::{Json, Path, Query, State},
    http::StatusCode,
};
use serde::{Deserialize, Serialize};

/// GET /api/groups
pub async fn list_groups(
    State(_state): State<crate::AppState>,
    Query(_query): Query<GroupQuery>,
) -> Result<Json<PaginatedResponse<GroupListItem>>, AuthError> {
    Err(AuthError::Internal("Not implemented".to_string()))
}

/// POST /api/groups
pub async fn create_group(
    State(_state): State<crate::AppState>,
    Json(_request): Json<CreateGroupRequest>,
) -> Result<Json<GroupResponse>, AuthError> {
    Err(AuthError::Internal("Not implemented".to_string()))
}

/// GET /api/groups/{id}
pub async fn get_group(
    State(_state): State<crate::AppState>,
    Path(_id): Path<i64>,
) -> Result<Json<GroupResponse>, AuthError> {
    Err(AuthError::Internal("Not implemented".to_string()))
}

/// PATCH /api/groups/{id}
pub async fn update_group(
    State(_state): State<crate::AppState>,
    Path(_id): Path<i64>,
    Json(_request): Json<UpdateGroupRequest>,
) -> Result<Json<GroupResponse>, AuthError> {
    Err(AuthError::Internal("Not implemented".to_string()))
}

/// DELETE /api/groups/{id}
pub async fn delete_group(
    State(_state): State<crate::AppState>,
    Path(_id): Path<i64>,
) -> Result<StatusCode, AuthError> {
    Err(AuthError::Internal("Not implemented".to_string()))
}

/// GET /api/groups/{id}/members
pub async fn list_group_members(
    State(_state): State<crate::AppState>,
    Path(_id): Path<i64>,
) -> Result<Json<Vec<GroupMemberResponse>>, AuthError> {
    Err(AuthError::Internal("Not implemented".to_string()))
}

/// POST /api/groups/{id}/members
pub async fn add_group_member(
    State(_state): State<crate::AppState>,
    Path(_id): Path<i64>,
    Json(_request): Json<AddGroupMemberRequest>,
) -> Result<Json<GroupMemberResponse>, AuthError> {
    Err(AuthError::Internal("Not implemented".to_string()))
}

/// GET /api/groups/{id}/members/{user_id}
pub async fn get_group_member(
    State(_state): State<crate::AppState>,
    Path((_group_id, _user_id)): Path<(i64, i64)>,
) -> Result<Json<GroupMemberResponse>, AuthError> {
    Err(AuthError::Internal("Not implemented".to_string()))
}

/// PATCH /api/groups/{id}/members/{user_id}
pub async fn update_group_member(
    State(_state): State<crate::AppState>,
    Path((_group_id, _user_id)): Path<(i64, i64)>,
    Json(_request): Json<UpdateGroupMemberRequest>,
) -> Result<Json<GroupMemberResponse>, AuthError> {
    Err(AuthError::Internal("Not implemented".to_string()))
}

/// DELETE /api/groups/{id}/members/{user_id}
pub async fn remove_group_member(
    State(_state): State<crate::AppState>,
    Path((_group_id, _user_id)): Path<(i64, i64)>,
) -> Result<StatusCode, AuthError> {
    Err(AuthError::Internal("Not implemented".to_string()))
}
