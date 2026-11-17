//! Two-Factor Authentication HTTP Handlers

use axum::{
    extract::{State, Json},
    http::StatusCode,
};
use serde::{Deserialize, Serialize};
use crate::services::TwoFactorService;
use crate::models::two_factor::TwoFactorSetup;
use crate::errors::AuthError;
use std::sync::Arc;

/// 2FA setup response
#[derive(Debug, Serialize)]
pub struct TwoFactorSetupResponse {
    pub secret: String,
    pub qr_code_svg: String,
    pub backup_codes: Vec<String>,
    pub issuer: String,
    pub account_name: String,
}

impl From<TwoFactorSetup> for TwoFactorSetupResponse {
    fn from(setup: TwoFactorSetup) -> Self {
        Self {
            secret: setup.secret,
            qr_code_svg: setup.qr_code_svg,
            backup_codes: setup.backup_codes,
            issuer: setup.issuer,
            account_name: setup.account_name,
        }
    }
}

/// Verify 2FA request
#[derive(Debug, Deserialize)]
pub struct VerifyTwoFactorRequest {
    pub code: String,
    pub backup_code: Option<String>,
}

/// Enable 2FA request
#[derive(Debug, Deserialize)]
pub struct EnableTwoFactorRequest {
    pub code: String,
    pub secret: String,
    pub backup_codes: Vec<String>,
}

/// Disable 2FA request
#[derive(Debug, Deserialize)]
pub struct DisableTwoFactorRequest {
    pub password: String,
}

/// Regenerate backup codes request
#[derive(Debug, Deserialize)]
pub struct RegenerateBackupCodesRequest {
    pub password: String,
}

/// Backup codes response
#[derive(Debug, Serialize)]
pub struct BackupCodesResponse {
    pub backup_codes: Vec<String>,
}

/// GET /api/auth/2fa/setup
pub async fn setup_two_factor(
    State(two_factor_service): State<Arc<TwoFactorService>>,
    subject: crate::models::subject::Subject,
) -> Result<Json<TwoFactorSetupResponse>, AuthError> {
    let user_id = match subject {
        crate::models::subject::Subject::User(p) => p.user_id,
        _ => return Err(AuthError::Unauthorized("User authentication required".to_string())),
    };

    // Get issuer from config (would be injected)
    let issuer = "OneDev";
    
    let setup = two_factor_service.create_setup(user_id, issuer).await?;
    Ok(Json(setup.into()))
}

/// POST /api/auth/2fa/enable
pub async fn enable_two_factor(
    State(two_factor_service): State<Arc<TwoFactorService>>,
    subject: crate::models::subject::Subject,
    Json(request): Json<EnableTwoFactorRequest>,
) -> Result<StatusCode, AuthError> {
    let user_id = match subject {
        crate::models::subject::Subject::User(p) => p.user_id,
        _ => return Err(AuthError::Unauthorized("User authentication required".to_string())),
    };

    two_factor_service.enable(
        user_id,
        &request.code,
        &request.secret,
        request.backup_codes,
    ).await?;

    Ok(StatusCode::NO_CONTENT)
}

/// POST /api/auth/2fa/disable
pub async fn disable_two_factor(
    State(two_factor_service): State<Arc<TwoFactorService>>,
    State(auth_service): State<Arc<crate::services::AuthService>>,
    subject: crate::models::subject::Subject,
    Json(request): Json<DisableTwoFactorRequest>,
) -> Result<StatusCode, AuthError> {
    let user_id = match subject {
        crate::models::subject::Subject::User(p) => p.user_id,
        _ => return Err(AuthError::Unauthorized("User authentication required".to_string())),
    };

    // Get user's password hash for verification
    let user = auth_service.user_repo.find_by_id(&auth_service.db_pool, user_id).await?
        .ok_or(AuthError::UserNotFound)?;

    let password_hash = user.password_hash
        .ok_or(AuthError::Internal("User has no password set".to_string()))?;

    two_factor_service.disable(user_id, &request.password, &password_hash).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// POST /api/auth/2fa/backup-codes/regenerate
pub async fn regenerate_backup_codes(
    State(two_factor_service): State<Arc<TwoFactorService>>,
    State(auth_service): State<Arc<crate::services::AuthService>>,
    subject: crate::models::subject::Subject,
    Json(request): Json<RegenerateBackupCodesRequest>,
) -> Result<Json<BackupCodesResponse>, AuthError> {
    let user_id = match subject {
        crate::models::subject::Subject::User(p) => p.user_id,
        _ => return Err(AuthError::Unauthorized("User authentication required".to_string())),
    };

    // Get user's password hash for verification
    let user = auth_service.user_repo.find_by_id(&auth_service.db_pool, user_id).await?
        .ok_or(AuthError::UserNotFound)?;

    let password_hash = user.password_hash
        .ok_or(AuthError::Internal("User has no password set".to_string()))?;

    let backup_codes = two_factor_service.regenerate_backup_codes(user_id, &request.password, &password_hash).await?;
    
    Ok(Json(BackupCodesResponse { backup_codes }))
}

/// GET /api/auth/2fa/status
#[derive(Debug, Serialize)]
pub struct TwoFactorStatusResponse {
    pub enabled: bool,
    pub backup_codes_remaining: usize,
}

pub async fn two_factor_status(
    State(auth_service): State<Arc<crate::services::AuthService>>,
    subject: crate::models::subject::Subject,
) -> Result<Json<TwoFactorStatusResponse>, AuthError> {
    let user_id = match subject {
        crate::models::subject::Subject::User(p) => p.user_id,
        _ => return Err(AuthError::Unauthorized("User authentication required".to_string())),
    };

    let user = auth_service.user_repo.find_by_id(&auth_service.db_pool, user_id).await?
        .ok_or(AuthError::UserNotFound)?;

    Ok(Json(TwoFactorStatusResponse {
        enabled: user.two_factor_enabled,
        backup_codes_remaining: user.backup_codes.len(),
    }))
}