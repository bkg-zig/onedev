//! Router for Authentication & Authorization API

use crate::handlers::*;
use crate::services::{AuthService, SessionService, TokenService, TwoFactorService};
use axum::{
    Router,
    routing::{delete, get, patch, post},
};
use std::sync::Arc;

/// Application state for auth routes
#[derive(Clone)]
pub struct AuthState {
    pub auth_service: Arc<AuthService>,
    pub session_service: Arc<SessionService>,
    pub token_service: Arc<TokenService>,
    pub two_factor_service: Arc<TwoFactorService>,
}

/// Create the auth router
pub fn create_auth_router(state: AuthState) -> Router {
    Router::new()
        // Auth endpoints
        .route("/login", post(auth_handlers::login))
        .route("/verify-2fa", post(auth_handlers::verify_two_factor))
        .route("/logout", post(auth_handlers::logout))
        .route("/me", get(auth_handlers::current_user))
        .route("/refresh", post(auth_handlers::refresh_token))
        // Session endpoints
        .route("/sessions", get(session_handlers::list_sessions))
        .route("/sessions", delete(session_handlers::revoke_all_sessions))
        .route("/sessions/:id", get(session_handlers::get_session))
        .route("/sessions/:id", delete(session_handlers::revoke_session))
        // Access token endpoints
        .route("/tokens", get(token_handlers::list_tokens))
        .route("/tokens", post(token_handlers::create_token))
        .route("/tokens/:id", get(token_handlers::get_token))
        .route("/tokens/:id", patch(token_handlers::update_token))
        .route("/tokens/:id", delete(token_handlers::revoke_token))
        // 2FA endpoints
        .route("/2fa/setup", get(two_factor_handlers::setup_two_factor))
        .route("/2fa/enable", post(two_factor_handlers::enable_two_factor))
        .route(
            "/2fa/disable",
            post(two_factor_handlers::disable_two_factor),
        )
        .route("/2fa/status", get(two_factor_handlers::two_factor_status))
        .route(
            "/2fa/backup-codes/regenerate",
            post(two_factor_handlers::regenerate_backup_codes),
        )
        // Password endpoints
        .route("/password/change", post(password_handlers::change_password))
        .route(
            "/password/reset/request",
            post(password_handlers::request_password_reset),
        )
        .route(
            "/password/reset/confirm",
            post(password_handlers::confirm_password_reset),
        )
        .route(
            "/password/validate",
            post(password_handlers::validate_password),
        )
        .with_state(state)
}

/// Create the public auth router (no auth required)
pub fn create_public_auth_router(state: AuthState) -> Router {
    Router::new()
        .route("/login", post(auth_handlers::login))
        .route("/verify-2fa", post(auth_handlers::verify_two_factor))
        .route("/refresh", post(auth_handlers::refresh_token))
        .route(
            "/password/reset/request",
            post(password_handlers::request_password_reset),
        )
        .route(
            "/password/reset/confirm",
            post(password_handlers::confirm_password_reset),
        )
        .route(
            "/password/validate",
            post(password_handlers::validate_password),
        )
        .with_state(state)
}

/// Create the protected auth router (auth required)
pub fn create_protected_auth_router(state: AuthState) -> Router {
    Router::new()
        .route("/logout", post(auth_handlers::logout))
        .route("/me", get(auth_handlers::current_user))
        .route("/sessions", get(session_handlers::list_sessions))
        .route("/sessions", delete(session_handlers::revoke_all_sessions))
        .route("/sessions/:id", get(session_handlers::get_session))
        .route("/sessions/:id", delete(session_handlers::revoke_session))
        .route("/tokens", get(token_handlers::list_tokens))
        .route("/tokens", post(token_handlers::create_token))
        .route("/tokens/:id", get(token_handlers::get_token))
        .route("/tokens/:id", patch(token_handlers::update_token))
        .route("/tokens/:id", delete(token_handlers::revoke_token))
        .route("/2fa/setup", get(two_factor_handlers::setup_two_factor))
        .route("/2fa/enable", post(two_factor_handlers::enable_two_factor))
        .route(
            "/2fa/disable",
            post(two_factor_handlers::disable_two_factor),
        )
        .route("/2fa/status", get(two_factor_handlers::two_factor_status))
        .route(
            "/2fa/backup-codes/regenerate",
            post(two_factor_handlers::regenerate_backup_codes),
        )
        .route("/password/change", post(password_handlers::change_password))
        .with_state(state)
}
