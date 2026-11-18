//! Authentication HTTP Handlers — wired to `onedev-auth`.

use crate::AppState;
use crate::errors::AuthError;
use crate::models::users::{UserResponse, UserStatus as ApiUserStatus, UserType as ApiUserType};
use crate::utils::extractors::{CurrentSubject, OptionalSubject};
use axum::{
    extract::{Json, State},
    http::StatusCode,
};
use onedev_auth::Subject;
use onedev_auth::services::auth_service::AuthError as InnerAuthError;
use serde::{Deserialize, Serialize};

/// Login request
#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
    pub remember_me: Option<bool>,
    pub totp_code: Option<String>,
    pub backup_code: Option<String>,
}

/// Login response
#[derive(Debug, Serialize)]
pub struct LoginResponse {
    pub subject: String,
    pub token: TokenPair,
    pub user: UserResponse,
    pub requires_two_factor: bool,
}

/// Token pair
#[derive(Debug, Serialize)]
pub struct TokenPair {
    pub access_token: String,
    pub refresh_token: String,
    pub token_type: String,
    pub expires_in: i64,
}

/// 2FA verification request
#[derive(Debug, Deserialize)]
pub struct VerifyTwoFactorRequest {
    pub user_id: i64,
    pub code: String,
    pub backup_code: Option<String>,
    pub remember_me: Option<bool>,
}

/// Current user response
#[derive(Debug, Serialize)]
pub struct CurrentUserResponse {
    pub user: UserResponse,
    pub authenticated: bool,
}

fn map_auth_error(err: InnerAuthError) -> AuthError {
    match err {
        InnerAuthError::InvalidCredentials => AuthError::InvalidCredentials,
        InnerAuthError::UserNotFound => AuthError::UserNotFound,
        InnerAuthError::UserDisabled | InnerAuthError::PendingVerification => {
            AuthError::UserDisabled
        }
        InnerAuthError::Locked => AuthError::UserLocked,
        InnerAuthError::InvalidToken => AuthError::TokenInvalid,
        InnerAuthError::TokenExpired => AuthError::TokenExpired,
        InnerAuthError::TokenRevoked => AuthError::TokenRevoked,
        InnerAuthError::InvalidTwoFactorCode => AuthError::InvalidTwoFactorCode,
        InnerAuthError::TwoFactorRequired => AuthError::TwoFactorRequired,
        InnerAuthError::TwoFactorNotEnabled => AuthError::TwoFactorNotEnabled,
        InnerAuthError::InvalidBackupCode => AuthError::InvalidBackupCode,
        InnerAuthError::SessionNotFound => AuthError::SessionNotFound,
        InnerAuthError::SessionExpired => AuthError::SessionExpired,
        InnerAuthError::AccessTokenNotFound => AuthError::AccessTokenNotFound,
        InnerAuthError::InsufficientPermissions => {
            AuthError::InsufficientPermissions("insufficient permissions".to_owned())
        }
        other => AuthError::Internal(other.to_string()),
    }
}

fn user_response(user: &onedev_core::models::user::User) -> UserResponse {
    UserResponse {
        id: user.id,
        uuid: user.uuid,
        name: user.name.clone(),
        full_name: user.full_name.clone(),
        email: user.email.clone(),
        user_type: match user.user_type {
            onedev_core::models::user::UserType::Ordinary => ApiUserType::Ordinary,
            onedev_core::models::user::UserType::Service => ApiUserType::Service,
            onedev_core::models::user::UserType::Ai => ApiUserType::Ai,
        },
        status: match user.status {
            onedev_core::models::user::UserStatus::Active => ApiUserStatus::Active,
            onedev_core::models::user::UserStatus::Disabled => ApiUserStatus::Disabled,
            onedev_core::models::user::UserStatus::PendingVerification => {
                ApiUserStatus::PendingVerification
            }
            onedev_core::models::user::UserStatus::Locked => ApiUserStatus::Locked,
        },
        avatar_url: user.avatar_url.clone(),
        two_factor_enabled: user.two_factor_enabled,
        last_login_at: user.last_login_at,
        created_at: user.created_at,
        updated_at: user.updated_at,
        notify_own_events: user.notify_own_events,
        keep_emails_private: user.keep_emails_private,
        timezone: user.timezone.clone(),
        locale: user.locale.clone(),
    }
}

fn token_pair(pair: &onedev_auth::models::token::TokenPair) -> TokenPair {
    TokenPair {
        access_token: pair.access_token.clone(),
        refresh_token: pair.refresh_token.clone(),
        token_type: pair.token_type.clone(),
        expires_in: (pair.access_expires_at - chrono::Utc::now())
            .num_seconds()
            .max(0),
    }
}

async fn login_response(
    state: &AppState,
    result: onedev_auth::services::auth_service::AuthResult,
    identifier: &str,
) -> Result<Json<LoginResponse>, AuthError> {
    if result.requires_two_factor {
        return Err(AuthError::TwoFactorRequired);
    }
    let pair = result.token_pair.ok_or(AuthError::TokenInvalid)?;
    let user_id = result
        .subject
        .effective_user_id()
        .ok_or(AuthError::TokenInvalid)?;
    let user = state
        .auth_service
        .inner()
        .user_by_id(user_id)
        .await
        .map_err(map_auth_error)?;
    let _ = identifier;

    Ok(Json(LoginResponse {
        subject: result.subject.principal_string(),
        token: token_pair(&pair),
        user: user_response(&user),
        requires_two_factor: false,
    }))
}

/// POST /api/auth/login
pub async fn login(
    State(state): State<crate::AppState>,
    Json(request): Json<LoginRequest>,
) -> Result<Json<LoginResponse>, AuthError> {
    if request.totp_code.is_some() {
        return Err(AuthError::Unauthorized(
            "Use /verify-2fa endpoint for 2FA".to_owned(),
        ));
    }
    let result = state
        .auth_service
        .inner()
        .authenticate_password(
            &request.username,
            &request.password,
            request.remember_me.unwrap_or(false),
            None,
            None,
        )
        .await
        .map_err(map_auth_error)?;

    login_response(&state, result, &request.username).await
}

/// POST /api/auth/verify-2fa
pub async fn verify_two_factor(
    State(state): State<crate::AppState>,
    Json(request): Json<VerifyTwoFactorRequest>,
) -> Result<Json<LoginResponse>, AuthError> {
    let result = state
        .auth_service
        .inner()
        .verify_two_factor(
            request.user_id,
            &request.code,
            request.backup_code.as_deref(),
            request.remember_me.unwrap_or(false),
            None,
            None,
        )
        .await
        .map_err(map_auth_error)?;

    login_response(&state, result, "").await
}

/// POST /api/auth/logout
pub async fn logout(
    State(state): State<crate::AppState>,
    OptionalSubject(subject): OptionalSubject,
) -> Result<StatusCode, AuthError> {
    if let Some(Subject::User(principal)) = subject {
        state
            .auth_service
            .inner()
            .revoke_all_sessions(principal.user_id)
            .await
            .map_err(map_auth_error)?;
    }
    Ok(StatusCode::NO_CONTENT)
}

/// GET /api/auth/me
pub async fn current_user(
    State(state): State<crate::AppState>,
    CurrentSubject(subject): CurrentSubject,
) -> Result<Json<CurrentUserResponse>, AuthError> {
    let user_id = subject
        .effective_user_id()
        .ok_or_else(|| AuthError::Unauthorized("Not authenticated".to_owned()))?;
    let user = state
        .auth_service
        .inner()
        .user_by_id(user_id)
        .await
        .map_err(map_auth_error)?;

    Ok(Json(CurrentUserResponse {
        user: user_response(&user),
        authenticated: true,
    }))
}

/// POST /api/auth/refresh
pub async fn refresh_token(
    State(state): State<crate::AppState>,
    Json(request): Json<RefreshTokenRequest>,
) -> Result<Json<TokenPair>, AuthError> {
    let inner = state.auth_service.inner();
    let claims = inner
        .jwt_service()
        .validate_refresh_token(&request.refresh_token)
        .map_err(|_| AuthError::TokenInvalid)?;
    let user_id = claims.user_id.ok_or(AuthError::TokenInvalid)?;
    let user = inner.user_by_id(user_id).await.map_err(map_auth_error)?;
    if !matches!(user.status, onedev_core::models::user::UserStatus::Active) {
        return Err(AuthError::UserDisabled);
    }
    let pair = inner
        .jwt_service()
        .create_access_token(user_id, &user.name, vec!["api".to_owned()])
        .map_err(|_| AuthError::TokenInvalid)?;

    Ok(Json(token_pair(&pair)))
}

/// Refresh token request
#[derive(Debug, Deserialize)]
pub struct RefreshTokenRequest {
    pub refresh_token: String,
}
