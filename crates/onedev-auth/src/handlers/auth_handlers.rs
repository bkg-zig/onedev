//! Authentication HTTP Handlers

use axum::{
    extract::{State, Json},
    http::StatusCode,
    response::Response,
};
use serde::{Deserialize, Serialize};
use crate::services::AuthService;
use crate::models::subject::Subject;
use crate::models::token::TokenPair;
use crate::models::two_factor::TwoFactorSetup;
use crate::errors::{AuthError, ProblemDetails};
use std::sync::Arc;

/// Login request
#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    pub username: String, // Can be username or email
    pub password: String,
    pub remember_me: Option<bool>,
    pub totp_code: Option<String>,
    pub backup_code: Option<String>,
}

/// Login response
#[derive(Debug, Serialize)]
pub struct LoginResponse {
    pub subject: String,
    pub session_id: Option<String>,
    pub token_pair: Option<TokenPair>,
    pub requires_two_factor: bool,
    pub two_factor_setup: Option<TwoFactorSetup>,
}

/// 2FA verification request
#[derive(Debug, Deserialize)]
pub struct VerifyTwoFactorRequest {
    pub user_id: i64,
    pub code: String,
    pub backup_code: Option<String>,
    pub remember_me: Option<bool>,
}

/// Logout request
#[derive(Debug, Deserialize)]
pub struct LogoutRequest {
    pub session_id: Option<String>,
    pub all_sessions: Option<bool>,
}

/// Current user response
#[derive(Debug, Serialize)]
pub struct CurrentUserResponse {
    pub subject: String,
    pub authenticated: bool,
    pub user_id: Option<i64>,
    pub username: Option<String>,
    pub user_type: Option<String>,
    pub two_factor_enabled: bool,
}

/// POST /api/auth/login
pub async fn login(
    State(crate::router::AuthState { auth_service, .. }): State<crate::router::AuthState>,
    Json(request): Json<LoginRequest>,
) -> Result<Json<LoginResponse>, AuthError> {
    let remember_me = request.remember_me.unwrap_or(false);
    
    // If TOTP code provided, verify 2FA
    if let Some(code) = request.totp_code {
        // This would be called after initial login returns requires_two_factor
        // For now, we'll handle it in the verify_2fa endpoint
        return Err(AuthError::Unauthorized("Use /verify-2fa endpoint for 2FA".to_string()));
    }

    let result = auth_service.authenticate_password(
        &request.username,
        &request.password,
        remember_me,
        None, // user_agent
        None, // ip_address
    ).await?;

    let response = LoginResponse {
        subject: result.subject.principal_string(),
        session_id: result.session.map(|s| s.id.to_string()),
        token_pair: result.token_pair,
        requires_two_factor: result.requires_two_factor,
        two_factor_setup: result.two_factor_setup,
    };

    Ok(Json(response))
}

/// POST /api/auth/verify-2fa
pub async fn verify_two_factor(
    State(crate::router::AuthState { auth_service, .. }): State<crate::router::AuthState>,
    Json(request): Json<VerifyTwoFactorRequest>,
) -> Result<Json<LoginResponse>, AuthError> {
    let remember_me = request.remember_me.unwrap_or(false);
    
    let result = auth_service.verify_two_factor(
        request.user_id,
        &request.code,
        request.backup_code.as_deref(),
        remember_me,
        None,
        None,
    ).await?;

    let response = LoginResponse {
        subject: result.subject.principal_string(),
        session_id: result.session.map(|s| s.id.to_string()),
        token_pair: result.token_pair,
        requires_two_factor: result.requires_two_factor,
        two_factor_setup: result.two_factor_setup,
    };

    Ok(Json(response))
}

/// POST /api/auth/logout
pub async fn logout(
    State(crate::router::AuthState { auth_service, .. }): State<crate::router::AuthState>,
    subject: Subject,
    Json(request): Json<LogoutRequest>,
) -> Result<StatusCode, AuthError> {
    match subject {
        Subject::User(principal) => {
            if request.all_sessions.unwrap_or(false) {
                auth_service.revoke_all_sessions(principal.user_id).await?;
            } else if let Some(session_id) = request.session_id {
                let session_uuid = session_id.parse()
                    .map_err(|_| AuthError::Internal("Invalid session ID".to_string()))?;
                auth_service.revoke_session(session_uuid).await?;
            }
        }
        Subject::AccessToken(_) => {
            // Access tokens don't have sessions in the same way
            // Could implement token revocation here
        }
        Subject::Anonymous => {
            return Err(AuthError::Unauthorized("Not authenticated".to_string()));
        }
    }
    
    Ok(StatusCode::NO_CONTENT)
}

/// GET /api/auth/me
pub async fn current_user(
    subject: Subject,
) -> Result<Json<CurrentUserResponse>, AuthError> {
    let response = match subject {
        Subject::User(principal) => CurrentUserResponse {
            subject: principal.principal_string(),
            authenticated: true,
            user_id: Some(principal.user_id),
            username: Some(principal.username),
            user_type: Some(format!("{:?}", principal.user_type)),
            two_factor_enabled: true, // TODO: Get from user
        },
        Subject::AccessToken(principal) => CurrentUserResponse {
            subject: principal.principal_string(),
            authenticated: true,
            user_id: Some(principal.effective_user_id()),
            username: Some(principal.owner_username),
            user_type: Some(format!("{:?}", principal.token_type)),
            two_factor_enabled: false,
        },
        Subject::Anonymous => CurrentUserResponse {
            subject: "anonymous".to_string(),
            authenticated: false,
            user_id: None,
            username: None,
            user_type: None,
            two_factor_enabled: false,
        },
    };
    
    Ok(Json(response))
}

/// POST /api/auth/refresh
pub async fn refresh_token(
    State(crate::router::AuthState { auth_service, .. }): State<crate::router::AuthState>,
    Json(request): Json<RefreshTokenRequest>,
) -> Result<Json<TokenPair>, AuthError> {
    let claims = auth_service.jwt_service().validate_refresh_token(&request.refresh_token)?;
    
    // Verify it's a refresh token
    if claims.typ != crate::models::token::TokenType::Refresh {
        return Err(AuthError::TokenInvalid);
    }
    
    // Get user ID from claims
    let user_id = claims.user_id.ok_or(AuthError::TokenInvalid)?;
    
    // Get user to verify still active
    let user = auth_service.user_repo.find_by_id(&auth_service.db_pool, user_id).await?
        .ok_or(AuthError::UserNotFound)?;
    
    if !matches!(user.status, onedev_core::models::user::UserStatus::Active) {
        return Err(AuthError::UserDisabled);
    }
    
    // Generate new token pair
    let scopes = vec!["api".to_string(), "read_repository".to_string()];
    let token_pair = auth_service.jwt_service().create_access_token(user_id, &user.name, scopes)?;
    
    Ok(Json(token_pair))
}

/// Refresh token request
#[derive(Debug, Deserialize)]
pub struct RefreshTokenRequest {
    pub refresh_token: String,
}