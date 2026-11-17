//! User HTTP Handlers

use axum::{
    extract::{State, Path, Query, Json},
    http::StatusCode,
};
use serde::{Deserialize, Serialize};
use crate::models::access_tokens::{AccessTokenWithRawResponse, CreateAccessTokenRequest};
use crate::models::users::*;
use crate::models::pagination::{OffsetPagination, PaginatedResponse};
use crate::errors::AuthError;
use crate::AppState;

/// GET /api/users
pub async fn list_users(
    State(_state): State<crate::AppState>,
    Query(_query): Query<UserQuery>,
) -> Result<Json<PaginatedResponse<UserListItem>>, AuthError> {
    Err(AuthError::Internal("Not implemented".to_string()))
}

/// POST /api/users
pub async fn create_user(
    State(_state): State<crate::AppState>,
    Json(_request): Json<CreateUserRequest>,
) -> Result<Json<UserResponse>, AuthError> {
    Err(AuthError::Internal("Not implemented".to_string()))
}

/// GET /api/users/{id}
pub async fn get_user(
    State(_state): State<crate::AppState>,
    Path(_id): Path<i64>,
) -> Result<Json<UserResponse>, AuthError> {
    Err(AuthError::Internal("Not implemented".to_string()))
}

/// PATCH /api/users/{id}
pub async fn update_user(
    State(_state): State<crate::AppState>,
    Path(_id): Path<i64>,
    Json(_request): Json<UpdateUserRequest>,
) -> Result<Json<UserResponse>, AuthError> {
    Err(AuthError::Internal("Not implemented".to_string()))
}

/// DELETE /api/users/{id}
pub async fn delete_user(
    State(_state): State<crate::AppState>,
    Path(_id): Path<i64>,
) -> Result<StatusCode, AuthError> {
    Err(AuthError::Internal("Not implemented".to_string()))
}

/// GET /api/users/{id}/emails
pub async fn list_emails(
    State(_state): State<crate::AppState>,
    Path(_id): Path<i64>,
) -> Result<Json<Vec<EmailResponse>>, AuthError> {
    Err(AuthError::Internal("Not implemented".to_string()))
}

/// GET /api/users/{id}/ssh-keys
pub async fn list_ssh_keys(
    State(_state): State<crate::AppState>,
    Path(_id): Path<i64>,
) -> Result<Json<Vec<SshKeyResponse>>, AuthError> {
    Err(AuthError::Internal("Not implemented".to_string()))
}

/// GET /api/users/{id}/gpg-keys
pub async fn list_gpg_keys(
    State(_state): State<crate::AppState>,
    Path(_id): Path<i64>,
) -> Result<Json<Vec<GpgKeyResponse>>, AuthError> {
    Err(AuthError::Internal("Not implemented".to_string()))
}

/// GET /api/users/{id}/tokens
pub async fn list_tokens(
    State(_state): State<crate::AppState>,
    Path(_id): Path<i64>,
) -> Result<Json<Vec<AccessTokenResponse>>, AuthError> {
    Err(AuthError::Internal("Not implemented".to_string()))
}

/// POST /api/users/{id}/tokens
pub async fn create_token(
    State(_state): State<crate::AppState>,
    Path(_id): Path<i64>,
    Json(_request): Json<CreateAccessTokenRequest>,
) -> Result<Json<AccessTokenWithRawResponse>, AuthError> {
    Err(AuthError::Internal("Not implemented".to_string()))
}

/// DELETE /api/users/{id}/tokens/{token_id}
pub async fn revoke_token(
    State(_state): State<crate::AppState>,
    Path((_user_id, _token_id)): Path<(i64, i64)>,
) -> Result<StatusCode, AuthError> {
    Err(AuthError::Internal("Not implemented".to_string()))
}