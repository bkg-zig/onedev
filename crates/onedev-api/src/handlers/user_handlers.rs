//! User HTTP Handlers — wired to `UserService`.

use crate::errors::AuthError;
use crate::models::pagination::PaginatedResponse;
use crate::models::users::*;
use axum::{
    extract::{Json, Path, Query, State},
    http::StatusCode,
};

/// GET /api/users
pub async fn list_users(
    State(state): State<crate::AppState>,
    Query(query): Query<UserQuery>,
) -> Result<Json<PaginatedResponse<UserListItem>>, AuthError> {
    let limit = query.pagination.per_page.max(1) as i64;
    let offset = query.pagination.page as i64 * limit;
    let total = state.user_service.count().await? as u64;
    let items = state.user_service.list(limit, offset).await?;
    Ok(Json(PaginatedResponse::new(items, total, query.pagination)))
}

/// POST /api/users
pub async fn create_user(
    State(state): State<crate::AppState>,
    Json(request): Json<CreateUserRequest>,
) -> Result<Json<UserResponse>, AuthError> {
    Ok(Json(state.user_service.create(request).await?))
}

/// GET /api/users/{id}
pub async fn get_user(
    State(state): State<crate::AppState>,
    Path(id): Path<i64>,
) -> Result<Json<UserResponse>, AuthError> {
    Ok(Json(state.user_service.get(id).await?))
}

/// PATCH /api/users/{id}
pub async fn update_user(
    State(state): State<crate::AppState>,
    Path(id): Path<i64>,
    Json(request): Json<UpdateUserRequest>,
) -> Result<Json<UserResponse>, AuthError> {
    Ok(Json(state.user_service.update(id, request).await?))
}

/// DELETE /api/users/{id}
pub async fn delete_user(
    State(state): State<crate::AppState>,
    Path(id): Path<i64>,
) -> Result<StatusCode, AuthError> {
    state.user_service.delete(id).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// GET /api/users/{id}/emails
pub async fn list_emails(
    State(state): State<crate::AppState>,
    Path(id): Path<i64>,
) -> Result<Json<Vec<EmailResponse>>, AuthError> {
    Ok(Json(state.user_service.emails(id).await?))
}

/// GET /api/users/{id}/ssh-keys
pub async fn list_ssh_keys(
    State(state): State<crate::AppState>,
    Path(id): Path<i64>,
) -> Result<Json<Vec<SshKeyResponse>>, AuthError> {
    Ok(Json(state.user_service.ssh_keys(id).await?))
}

/// GET /api/users/{id}/gpg-keys
pub async fn list_gpg_keys(
    State(state): State<crate::AppState>,
    Path(id): Path<i64>,
) -> Result<Json<Vec<GpgKeyResponse>>, AuthError> {
    Ok(Json(state.user_service.gpg_keys(id).await?))
}

/// GET /api/users/{id}/tokens
pub async fn list_tokens(
    State(state): State<crate::AppState>,
    Path(id): Path<i64>,
) -> Result<Json<Vec<AccessTokenResponse>>, AuthError> {
    Ok(Json(state.user_service.tokens(id).await?))
}

/// POST /api/users/{id}/tokens
pub async fn create_token(
    State(state): State<crate::AppState>,
    Path(id): Path<i64>,
    Json(request): Json<crate::models::access_tokens::CreateAccessTokenRequest>,
) -> Result<Json<crate::models::access_tokens::AccessTokenWithRawResponse>, AuthError> {
    let _ = (state, id, request);
    Err(AuthError::Internal(
        "token creation is handled by onedev-auth token endpoints".to_owned(),
    ))
}

/// DELETE /api/users/{id}/tokens/{token_id}
pub async fn revoke_token(
    State(state): State<crate::AppState>,
    Path((_user_id, token_id)): Path<(i64, i64)>,
) -> Result<StatusCode, AuthError> {
    state
        .auth_service
        .inner()
        .revoke_access_token(token_id, _user_id)
        .await
        .map_err(|e| AuthError::Internal(e.to_string()))?;
    Ok(StatusCode::NO_CONTENT)
}
