//! Password Management HTTP Handlers

use axum::{
    extract::{State, Json},
    http::StatusCode,
};
use serde::{Deserialize, Serialize};
use crate::services::AuthService;
use crate::errors::AuthError;
use std::sync::Arc;

/// Change password request
#[derive(Debug, Deserialize)]
pub struct ChangePasswordRequest {
    pub current_password: String,
    pub new_password: String,
}

/// Request password reset request
#[derive(Debug, Deserialize)]
pub struct RequestPasswordResetRequest {
    pub email: String,
}

/// Confirm password reset request
#[derive(Debug, Deserialize)]
pub struct ConfirmPasswordResetRequest {
    pub token: String,
    pub new_password: String,
}

/// Password reset response
#[derive(Debug, Serialize)]
pub struct PasswordResetResponse {
    pub message: String,
}

/// POST /api/auth/password/change
pub async fn change_password(
    State(crate::router::AuthState { auth_service, .. }): State<crate::router::AuthState>,
    subject: crate::models::subject::Subject,
    Json(request): Json<ChangePasswordRequest>,
) -> Result<StatusCode, AuthError> {
    let user_id = match subject {
        crate::models::subject::Subject::User(p) => p.user_id,
        _ => return Err(AuthError::Unauthorized("User authentication required".to_string())),
    };

    auth_service.change_password(user_id, &request.current_password, &request.new_password).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// POST /api/auth/password/reset/request
pub async fn request_password_reset(
    State(crate::router::AuthState { auth_service, .. }): State<crate::router::AuthState>,
    Json(request): Json<RequestPasswordResetRequest>,
) -> Result<Json<PasswordResetResponse>, AuthError> {
    // Always return success to prevent email enumeration
    let _token = auth_service.request_password_reset(&request.email).await?;
    
    Ok(Json(PasswordResetResponse {
        message: "If the email exists, a password reset link has been sent".to_string(),
    }))
}

/// POST /api/auth/password/reset/confirm
pub async fn confirm_password_reset(
    State(crate::router::AuthState { auth_service, .. }): State<crate::router::AuthState>,
    Json(request): Json<ConfirmPasswordResetRequest>,
) -> Result<Json<PasswordResetResponse>, AuthError> {
    auth_service.confirm_password_reset(&request.token, &request.new_password).await?;
    
    Ok(Json(PasswordResetResponse {
        message: "Password has been reset successfully".to_string(),
    }))
}

/// Validate password strength request
#[derive(Debug, Deserialize)]
pub struct ValidatePasswordRequest {
    pub password: String,
}

/// Validate password strength response
#[derive(Debug, Serialize)]
pub struct ValidatePasswordResponse {
    pub valid: bool,
    pub errors: Vec<String>,
}

/// POST /api/auth/password/validate
pub async fn validate_password(
    Json(request): Json<ValidatePasswordRequest>,
) -> Result<Json<ValidatePasswordResponse>, AuthError> {
    use crate::models::password::validate_password_strength;
    
    let mut errors = Vec::new();
    
    if request.password.len() < 8 {
        errors.push("Password must be at least 8 characters".to_string());
    }
    if request.password.len() > 128 {
        errors.push("Password must not exceed 128 characters".to_string());
    }
    
    let has_letter = request.password.chars().any(|c| c.is_alphabetic());
    let has_digit = request.password.chars().any(|c| c.is_ascii_digit());
    
    if !has_letter {
        errors.push("Password must contain at least one letter".to_string());
    }
    if !has_digit {
        errors.push("Password must contain at least one number".to_string());
    }
    
    let valid = errors.is_empty();
    
    Ok(Json(ValidatePasswordResponse { valid, errors }))
}