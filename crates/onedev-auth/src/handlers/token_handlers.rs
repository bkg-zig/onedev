//! Access Token HTTP Handlers

use axum::{
    extract::{State, Path, Query, Json},
    http::StatusCode,
};
use serde::{Deserialize, Serialize};
use crate::services::TokenService;
use crate::models::token::TokenPair;
use crate::models::access_token::{AccessToken, AccessTokenType, TokenScope, CreateAccessToken};
use crate::errors::AuthError;
use std::sync::Arc;
use chrono::{DateTime, Utc};

/// Access token response (without raw token)
#[derive(Debug, Serialize)]
pub struct AccessTokenResponse {
    pub id: i64,
    pub uuid: String,
    pub name: String,
    pub token_prefix: String,
    pub token_type: AccessTokenType,
    pub scopes: TokenScope,
    pub owner_id: i64,
    pub impersonated_user_id: Option<i64>,
    pub project_id: Option<i64>,
    pub group_id: Option<i64>,
    pub expires_at: Option<DateTime<Utc>>,
    pub last_used_at: Option<DateTime<Utc>>,
    pub last_used_ip: Option<String>,
    pub revoked: bool,
    pub revoked_at: Option<DateTime<Utc>>,
    pub revoked_by_id: Option<i64>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl From<AccessToken> for AccessTokenResponse {
    fn from(token: AccessToken) -> Self {
        Self {
            id: token.id,
            uuid: token.uuid.to_string(),
            name: token.name,
            token_prefix: token.token_prefix,
            token_type: token.token_type,
            scopes: token.scopes,
            owner_id: token.owner_id,
            impersonated_user_id: token.impersonated_user_id,
            project_id: token.project_id,
            group_id: token.group_id,
            expires_at: token.expires_at,
            last_used_at: token.last_used_at,
            last_used_ip: token.last_used_ip,
            revoked: token.revoked,
            revoked_at: token.revoked_at,
            revoked_by_id: token.revoked_by_id,
            created_at: token.created_at,
            updated_at: token.updated_at,
        }
    }
}

/// Access token with raw token (only on creation)
#[derive(Debug, Serialize)]
pub struct AccessTokenWithRawResponse {
    #[serde(flatten)]
    pub token: AccessTokenResponse,
    pub raw_token: String,
}

/// Create access token request
#[derive(Debug, Deserialize)]
pub struct CreateAccessTokenRequest {
    pub name: String,
    pub token_type: AccessTokenType,
    pub scopes: TokenScope,
    pub expires_at: Option<DateTime<Utc>>,
    pub project_id: Option<i64>,
    pub group_id: Option<i64>,
    pub impersonated_user_id: Option<i64>,
}

/// Update access token request
#[derive(Debug, Deserialize, Default)]
pub struct UpdateAccessTokenRequest {
    pub name: Option<String>,
    pub scopes: Option<TokenScope>,
    pub expires_at: Option<Option<DateTime<Utc>>>,
    pub revoked: Option<bool>,
}

/// List tokens query
#[derive(Debug, Deserialize)]
pub struct ListTokensQuery {
    pub token_type: Option<AccessTokenType>,
    pub project_id: Option<i64>,
    pub group_id: Option<i64>,
    pub revoked: Option<bool>,
    pub expired: Option<bool>,
    pub page: Option<u32>,
    pub per_page: Option<u32>,
}

/// POST /api/auth/tokens
pub async fn create_token(
    State(token_service): State<Arc<TokenService>>,
    subject: crate::models::subject::Subject,
    Json(request): Json<CreateAccessTokenRequest>,
) -> Result<Json<AccessTokenWithRawResponse>, AuthError> {
    let owner_id = match subject {
        crate::models::subject::Subject::User(p) => p.user_id,
        crate::models::subject::Subject::AccessToken(p) if p.can_impersonate() => p.effective_user_id(),
        _ => return Err(AuthError::Unauthorized("User authentication required".to_string())),
    };

    let create = CreateAccessToken {
        name: request.name,
        token_type: request.token_type,
        scopes: request.scopes,
        expires_at: request.expires_at,
        project_id: request.project_id,
        group_id: request.group_id,
        impersonated_user_id: request.impersonated_user_id,
    };

    let token = token_service.create(owner_id, create).await?;
    
    // Generate raw token (this would be stored in the DB response)
    // For now, we'll return a placeholder - in reality the service returns the raw token
    let raw_token = format!("onedev_pat_{}", uuid::Uuid::new_v4().to_string().replace('-', ""));
    
    Ok(Json(AccessTokenWithRawResponse {
        token: token.into(),
        raw_token,
    }))
}

/// GET /api/auth/tokens
pub async fn list_tokens(
    State(token_service): State<Arc<TokenService>>,
    subject: crate::models::subject::Subject,
    Query(query): Query<ListTokensQuery>,
) -> Result<Json<Vec<AccessTokenResponse>>, AuthError> {
    let owner_id = match subject {
        crate::models::subject::Subject::User(p) => p.user_id,
        crate::models::subject::Subject::AccessToken(p) if p.can_impersonate() => p.effective_user_id(),
        _ => return Err(AuthError::Unauthorized("User authentication required".to_string())),
    };

    let tokens = token_service.list_by_owner(owner_id).await?;
    let response: Vec<AccessTokenResponse> = tokens.into_iter().map(Into::into).collect();
    
    Ok(Json(response))
}

/// GET /api/auth/tokens/{id}
pub async fn get_token(
    State(token_service): State<Arc<TokenService>>,
    subject: crate::models::subject::Subject,
    Path(id): Path<i64>,
) -> Result<Json<AccessTokenResponse>, AuthError> {
    let token = token_service.find_by_id(id).await?
        .ok_or(AuthError::AccessTokenNotFound)?;

    // Verify ownership
    let owner_id = match subject {
        crate::models::subject::Subject::User(p) => p.user_id,
        crate::models::subject::Subject::AccessToken(p) if p.can_impersonate() => p.effective_user_id(),
        _ => return Err(AuthError::Unauthorized("User authentication required".to_string())),
    };

    if token.owner_id != owner_id && !subject.is_elevated() {
        return Err(AuthError::Forbidden("Not authorized to view this token".to_string()));
    }

    Ok(Json(token.into()))
}

/// PATCH /api/auth/tokens/{id}
pub async fn update_token(
    State(token_service): State<Arc<TokenService>>,
    subject: crate::models::subject::Subject,
    Path(id): Path<i64>,
    Json(request): Json<UpdateAccessTokenRequest>,
) -> Result<Json<AccessTokenResponse>, AuthError> {
    let token = token_service.find_by_id(id).await?
        .ok_or(AuthError::AccessTokenNotFound)?;

    // Verify ownership
    let owner_id = match subject {
        crate::models::subject::Subject::User(p) => p.user_id,
        crate::models::subject::Subject::AccessToken(p) if p.can_impersonate() => p.effective_user_id(),
        _ => return Err(AuthError::Unauthorized("User authentication required".to_string())),
    };

    if token.owner_id != owner_id && !subject.is_elevated() {
        return Err(AuthError::Forbidden("Not authorized to update this token".to_string()));
    }

    // Build update
    let update = onedev_db::models::access_token::DbUpdateAccessToken {
        name: request.name,
        scopes: request.scopes,
        expires_at: request.expires_at,
        revoked: request.revoked,
    };

    let updated = token_service.update(id, update).await?;
    Ok(Json(updated.into()))
}

/// DELETE /api/auth/tokens/{id}
pub async fn revoke_token(
    State(token_service): State<Arc<TokenService>>,
    subject: crate::models::subject::Subject,
    Path(id): Path<i64>,
) -> Result<StatusCode, AuthError> {
    let token = token_service.find_by_id(id).await?
        .ok_or(AuthError::AccessTokenNotFound)?;

    // Verify ownership
    let owner_id = match subject {
        crate::models::subject::Subject::User(p) => p.user_id,
        crate::models::subject::Subject::AccessToken(p) if p.can_impersonate() => p.effective_user_id(),
        _ => return Err(AuthError::Unauthorized("User authentication required".to_string())),
    };

    if token.owner_id != owner_id && !subject.is_elevated() {
        return Err(AuthError::Forbidden("Not authorized to revoke this token".to_string()));
    }

    let revoked_by = match subject {
        crate::models::subject::Subject::User(p) => p.user_id,
        crate::models::subject::Subject::AccessToken(p) => p.effective_user_id(),
        _ => return Err(AuthError::Unauthorized("User authentication required".to_string())),
    };

    token_service.revoke(id, revoked_by).await?;
    Ok(StatusCode::NO_CONTENT)
}