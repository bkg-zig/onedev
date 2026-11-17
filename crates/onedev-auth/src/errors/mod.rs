//! Error types for authentication and authorization

use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::Serialize;
use thiserror::Error;

/// Authentication error
#[derive(Debug, Error)]
pub enum AuthError {
    #[error("Unauthorized: {0}")]
    Unauthorized(String),
    #[error("Forbidden: {0}")]
    Forbidden(String),
    #[error("Invalid credentials")]
    InvalidCredentials,
    #[error("Token expired")]
    TokenExpired,
    #[error("Token invalid")]
    TokenInvalid,
    #[error("Token revoked")]
    TokenRevoked,
    #[error("Two-factor required")]
    TwoFactorRequired,
    #[error("Invalid two-factor code")]
    InvalidTwoFactorCode,
    #[error("Two-factor not enabled")]
    TwoFactorNotEnabled,
    #[error("Invalid backup code")]
    InvalidBackupCode,
    #[error("User not found")]
    UserNotFound,
    #[error("User disabled")]
    UserDisabled,
    #[error("User locked")]
    UserLocked,
    #[error("Session not found")]
    SessionNotFound,
    #[error("Session expired")]
    SessionExpired,
    #[error("Access token not found")]
    AccessTokenNotFound,
    #[error("Insufficient permissions: {0}")]
    InsufficientPermissions(String),
    #[error("Rate limited")]
    RateLimited,
    #[error("Internal error: {0}")]
    Internal(String),
}

/// Authorization error
#[derive(Debug, Error)]
pub enum AuthzError {
    #[error("Permission denied: {0}")]
    PermissionDenied(String),
    #[error("Resource not found")]
    NotFound,
    #[error("Internal error: {0}")]
    Internal(String),
}

/// RFC 7807 Problem Details
#[derive(Debug, Serialize)]
pub struct ProblemDetails {
    pub r#type: String,
    pub title: String,
    pub status: u16,
    pub detail: String,
    pub instance: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extensions: Option<serde_json::Value>,
}

impl ProblemDetails {
    pub fn new(status: StatusCode, title: &str, detail: &str) -> Self {
        Self {
            r#type: format!("https://httpstatuses.com/{}", status.as_u16()),
            title: title.to_string(),
            status: status.as_u16(),
            detail: detail.to_string(),
            instance: None,
            extensions: None,
        }
    }

    pub fn with_instance(mut self, instance: &str) -> Self {
        self.instance = Some(instance.to_string());
        self
    }

    pub fn with_extension(mut self, key: &str, value: serde_json::Value) -> Self {
        let mut map = self.extensions.unwrap_or_else(|| serde_json::json!({}));
        map[key] = value;
        self.extensions = Some(map);
        self
    }
}

impl IntoResponse for AuthError {
    fn into_response(self) -> Response {
        let (status, title, detail) = match &self {
            AuthError::Unauthorized(msg) => (StatusCode::UNAUTHORIZED, "Unauthorized", msg.as_str()),
            AuthError::Forbidden(msg) => (StatusCode::FORBIDDEN, "Forbidden", msg.as_str()),
            AuthError::InvalidCredentials => (StatusCode::UNAUTHORIZED, "Invalid Credentials", "Invalid username or password"),
            AuthError::TokenExpired => (StatusCode::UNAUTHORIZED, "Token Expired", "Authentication token has expired"),
            AuthError::TokenInvalid => (StatusCode::UNAUTHORIZED, "Invalid Token", "Authentication token is invalid"),
            AuthError::TokenRevoked => (StatusCode::UNAUTHORIZED, "Token Revoked", "Authentication token has been revoked"),
            AuthError::TwoFactorRequired => (StatusCode::UNAUTHORIZED, "Two-Factor Required", "Two-factor authentication is required"),
            AuthError::InvalidTwoFactorCode => (StatusCode::UNAUTHORIZED, "Invalid 2FA Code", "Invalid two-factor authentication code"),
            AuthError::TwoFactorNotEnabled => (StatusCode::BAD_REQUEST, "2FA Not Enabled", "Two-factor authentication is not enabled"),
            AuthError::InvalidBackupCode => (StatusCode::UNAUTHORIZED, "Invalid Backup Code", "Invalid backup code"),
            AuthError::UserNotFound => (StatusCode::NOT_FOUND, "User Not Found", "User not found"),
            AuthError::UserDisabled => (StatusCode::FORBIDDEN, "User Disabled", "User account is disabled"),
            AuthError::UserLocked => (StatusCode::FORBIDDEN, "User Locked", "User account is locked"),
            AuthError::SessionNotFound => (StatusCode::UNAUTHORIZED, "Session Not Found", "Session not found"),
            AuthError::SessionExpired => (StatusCode::UNAUTHORIZED, "Session Expired", "Session has expired"),
            AuthError::AccessTokenNotFound => (StatusCode::UNAUTHORIZED, "Access Token Not Found", "Access token not found"),
            AuthError::InsufficientPermissions(msg) => (StatusCode::FORBIDDEN, "Insufficient Permissions", msg.as_str()),
            AuthError::RateLimited => (StatusCode::TOO_MANY_REQUESTS, "Rate Limited", "Too many requests"),
            AuthError::Internal(msg) => (StatusCode::INTERNAL_SERVER_ERROR, "Internal Server Error", msg.as_str()),
        };

        let problem = ProblemDetails::new(status, title, detail);
        (status, Json(problem)).into_response()
    }
}

impl IntoResponse for AuthzError {
    fn into_response(self) -> Response {
        let (status, title, detail) = match &self {
            AuthzError::PermissionDenied(msg) => (StatusCode::FORBIDDEN, "Permission Denied", msg.as_str()),
            AuthzError::NotFound => (StatusCode::NOT_FOUND, "Not Found", "Resource not found"),
            AuthzError::Internal(msg) => (StatusCode::INTERNAL_SERVER_ERROR, "Internal Server Error", msg.as_str()),
        };

        let problem = ProblemDetails::new(status, title, detail);
        (status, Json(problem)).into_response()
    }
}

/// Convert anyhow::Error to AuthError
impl From<anyhow::Error> for AuthError {
    fn from(err: anyhow::Error) -> Self {
        AuthError::Internal(err.to_string())
    }
}

/// Convert onedev_db::DbError to AuthError
impl From<onedev_db::DbError> for AuthError {
    fn from(err: onedev_db::DbError) -> Self {
        AuthError::Internal(err.to_string())
    }
}

/// Convert onedev_db::DbError to AuthzError
impl From<onedev_db::DbError> for AuthzError {
    fn from(err: onedev_db::DbError) -> Self {
        AuthzError::Internal(err.to_string())
    }
}

impl From<crate::services::auth_service::AuthError> for AuthError {
    fn from(err: crate::services::auth_service::AuthError) -> Self {
        use crate::services::auth_service::AuthError as ServiceError;
        match err {
            ServiceError::InvalidCredentials => AuthError::InvalidCredentials,
            ServiceError::UserNotFound => AuthError::UserNotFound,
            ServiceError::UserDisabled | ServiceError::PendingVerification => AuthError::UserDisabled,
            ServiceError::Locked => AuthError::UserLocked,
            ServiceError::InvalidToken => AuthError::TokenInvalid,
            ServiceError::TokenExpired => AuthError::TokenExpired,
            other => AuthError::Internal(other.to_string()),
        }
    }
}

impl From<String> for AuthError {
    fn from(err: String) -> Self {
        AuthError::Internal(err)
    }
}

impl From<crate::models::token::JwtError> for AuthError {
    fn from(err: crate::models::token::JwtError) -> Self {
        use crate::models::token::JwtError;
        match err {
            JwtError::Expired => AuthError::TokenExpired,
            other => AuthError::TokenInvalid,
        }
    }
}