//! Authentication & Authorization Middleware for Axum

use crate::errors::AuthError;
use crate::models::subject::{CURRENT_SUBJECT, Subject};
use crate::models::token::{Claims, JwtService};
use crate::services::AuthService;
use axum::http::request::Parts;
use axum::{
    extract::{FromRequestParts, Request, State},
    http::{HeaderMap, StatusCode, header},
    middleware::Next,
    response::Response,
};
use axum_extra::extract::CookieJar;
use onedev_core::permissions::ProjectPermissionType;
use std::sync::Arc;
use tracing::{debug, warn};

/// Extractor for the current authenticated subject
#[derive(Debug, Clone)]
pub struct CurrentSubject(pub Subject);

impl<S> FromRequestParts<S> for CurrentSubject
where
    S: Send + Sync,
{
    type Rejection = AuthError;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        // Try to get from extensions (set by auth middleware)
        if let Some(subject) = parts.extensions.get::<Subject>() {
            return Ok(CurrentSubject(subject.clone()));
        }

        // Default to anonymous
        Ok(CurrentSubject(Subject::Anonymous))
    }
}

/// Optional current subject (doesn't reject if not authenticated)
#[derive(Debug, Clone)]
pub struct OptionalSubject(pub Option<Subject>);

impl<S> FromRequestParts<S> for OptionalSubject
where
    S: Send + Sync,
{
    type Rejection = AuthError;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        if let Some(subject) = parts.extensions.get::<Subject>() {
            Ok(OptionalSubject(Some(subject.clone())))
        } else {
            Ok(OptionalSubject(None))
        }
    }
}

/// Require authenticated user (rejects anonymous)
pub async fn require_user(CurrentSubject(subject): CurrentSubject) -> Result<Subject, AuthError> {
    match subject {
        Subject::User(_) | Subject::AccessToken(_) => Ok(subject),
        Subject::Anonymous => Err(AuthError::Unauthorized(
            "Authentication required".to_string(),
        )),
    }
}

/// Require authenticated user (not access token)
pub async fn require_user_principal(
    CurrentSubject(subject): CurrentSubject,
) -> Result<crate::models::principal::UserPrincipal, AuthError> {
    match subject {
        Subject::User(p) => Ok(p),
        _ => Err(AuthError::Unauthorized(
            "User authentication required".to_string(),
        )),
    }
}

/// Require admin/root access
pub async fn require_admin(CurrentSubject(subject): CurrentSubject) -> Result<Subject, AuthError> {
    if subject.is_elevated() {
        Ok(subject)
    } else {
        Err(AuthError::Forbidden("Admin access required".to_string()))
    }
}

/// Authentication middleware - extracts and validates JWT token
pub async fn auth_middleware(
    State(auth_service): State<Arc<AuthService>>,
    headers: HeaderMap,
    cookies: CookieJar,
    mut request: Request,
    next: Next,
) -> Result<Response, AuthError> {
    let token = extract_token(&headers, &cookies);

    let subject = if let Some(token) = token {
        // Try JWT first
        if let Ok(claims) = auth_service.jwt_service().validate_access_token(&token) {
            match auth_service.subject_from_claims(&claims).await {
                Ok(s) => s,
                Err(_) => Subject::Anonymous,
            }
        } else {
            // Try as access token (PAT)
            match auth_service.authenticate_token(&token).await {
                Ok(s) => s,
                Err(_) => Subject::Anonymous,
            }
        }
    } else {
        Subject::Anonymous
    };

    // Set subject in request extensions for extractors
    request.extensions_mut().insert(subject.clone());

    // Also set in task-local for internal use
    Ok(CURRENT_SUBJECT
        .scope(subject, async { next.run(request).await })
        .await)
}

/// Extract token from Authorization header or cookie
fn extract_token(headers: &HeaderMap, cookies: &CookieJar) -> Option<String> {
    // Check Authorization header
    if let Some(auth_header) = headers.get(header::AUTHORIZATION) {
        if let Ok(auth_str) = auth_header.to_str() {
            if let Some(token) = auth_str.strip_prefix("Bearer ") {
                return Some(token.to_string());
            }
            if let Some(token) = auth_str.strip_prefix("Token ") {
                return Some(token.to_string());
            }
        }
    }

    // Check cookie
    if let Some(cookie) = cookies.get("access_token") {
        return Some(cookie.value().to_string());
    }

    // Check X-Access-Token header (for Git HTTP)
    if let Some(token_header) = headers.get("X-Access-Token") {
        if let Ok(token) = token_header.to_str() {
            return Some(token.to_string());
        }
    }

    None
}

/// Authorization middleware.
///
/// The permission check itself needs `AuthorizationService` from router state,
/// which this layer does not receive. It forwards the request.
pub fn permission_middleware(
    permission: onedev_core::permissions::ProjectPermissionType,
    project_id: i64,
) -> axum::middleware::FromFnLayer<
    impl Fn(Request, Next) -> std::pin::Pin<Box<dyn std::future::Future<Output = Response> + Send>>
    + Clone,
    (),
    (Request, Next),
> {
    let _ = (permission, project_id);
    axum::middleware::from_fn(|request: Request, next: Next| {
        Box::pin(async move { next.run(request).await })
            as std::pin::Pin<Box<dyn std::future::Future<Output = Response> + Send>>
    })
}

/// Rate limiting middleware for auth endpoints
pub async fn auth_rate_limit(
    State(_rate_limiter): State<Arc<()>>, // Placeholder for rate limiter
    request: Request,
    next: Next,
) -> Result<Response, AuthError> {
    // TODO: Implement rate limiting
    Ok(next.run(request).await)
}

/// CORS middleware for auth endpoints
pub fn cors_middleware() -> tower_http::cors::CorsLayer {
    tower_http::cors::CorsLayer::new()
        .allow_origin(tower_http::cors::Any)
        .allow_methods([
            axum::http::Method::GET,
            axum::http::Method::POST,
            axum::http::Method::OPTIONS,
        ])
        .allow_headers([header::AUTHORIZATION, header::CONTENT_TYPE, header::COOKIE])
        .allow_credentials(true)
}
