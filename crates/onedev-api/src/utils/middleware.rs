//! HTTP Middleware

use axum::{
    extract::{State, Request},
    http::{header, HeaderMap, StatusCode},
    middleware::Next,
    response::Response,
};
use axum_extra::extract::CookieJar;
use crate::AppState;
use crate::errors::AuthError;
use onedev_auth::Subject;
use onedev_auth::models::token::JwtService;

/// Authentication middleware - extracts and validates JWT token
pub async fn auth_middleware(
    State(state): State<crate::AppState>,
    headers: HeaderMap,
    cookies: CookieJar,
    mut request: Request,
    next: Next,
) -> Result<Response, AuthError> {
    let token = extract_token(&headers, &cookies);
    
    let subject = if let Some(token) = token {
        // Try JWT first
        if let Ok(claims) = state.auth_service.inner().jwt_service().validate_access_token(&token) {
            match state.auth_service.inner().subject_from_claims(&claims).await {
                Ok(s) => s,
                Err(_) => Subject::Anonymous,
            }
        } else {
            // Try as access token (PAT)
            match state.auth_service.inner().authenticate_token(&token).await {
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
    onedev_auth::CURRENT_SUBJECT.scope(subject, async {
        next.run(request).await
    }).await
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

/// CORS middleware for API
pub fn cors_middleware() -> tower_http::cors::CorsLayer {
    tower_http::cors::CorsLayer::new()
        .allow_origin(tower_http::cors::Any)
        .allow_methods([
            axum::http::Method::GET,
            axum::http::Method::POST,
            axum::http::Method::PATCH,
            axum::http::Method::PUT,
            axum::http::Method::DELETE,
            axum::http::Method::OPTIONS,
        ])
        .allow_headers([
            header::AUTHORIZATION,
            header::CONTENT_TYPE,
            header::COOKIE,
            header::ACCEPT,
        ])
        .allow_credentials(true)
        .expose_headers([header::CONTENT_LENGTH])
}

/// Rate limiting middleware placeholder
pub async fn rate_limit_middleware(
    State(_state): State<crate::AppState>,
    request: Request,
    next: Next,
) -> Result<Response, AuthError> {
    // TODO: Implement rate limiting
    Ok(next.run(request).await)
}

/// Request ID middleware
pub async fn request_id_middleware(
    mut request: Request,
    next: Next,
) -> Response {
    let request_id = uuid::Uuid::new_v4().to_string();
    request.extensions_mut().insert(request_id.clone());
    
    let mut response = next.run(request).await;
    response.headers_mut().insert(
        "X-Request-ID",
        request_id.parse().unwrap(),
    );
    response
}