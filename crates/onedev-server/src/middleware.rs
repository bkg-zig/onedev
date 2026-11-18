//! HTTP Middleware

use axum::{
    body::Body,
    extract::{MatchedPath, Request, State},
    http::{HeaderMap, HeaderValue, StatusCode},
    middleware::Next,
    response::Response,
};
use governor::{Quota, RateLimiter};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;
use tracing::{debug, warn};
use uuid::Uuid;

use crate::config::AppConfig;

/// Rate limiter state
pub type RateLimiterState = Arc<Mutex<RateLimiter<String, governor::state::InMemoryState, governor::clock::DefaultClock>>>;

/// Create rate limiter from config
pub fn create_rate_limiter(config: &AppConfig) -> RateLimiterState {
    let quota = Quota::per_minute(config.security.rate_limit.requests_per_minute)
        .allow_burst(config.security.rate_limit.burst);

    let limiter = RateLimiter::direct(quota);
    Arc::new(Mutex::new(limiter))
}

/// Rate limiting middleware
pub async fn rate_limit_middleware(
    State(limiter): State<RateLimiterState>,
    req: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    // Extract client identifier (IP or user ID)
    let client_id = extract_client_id(&req);

    let mut limiter = limiter.lock().await;
    match limiter.check_key(&client_id) {
        Ok(_) => {
            drop(limiter);
            Ok(next.run(req).await)
        }
        Err(_) => {
            warn!("Rate limit exceeded for client: {}", client_id);
            Err(StatusCode::TOO_MANY_REQUESTS)
        }
    }
}

/// Extract client identifier from request
fn extract_client_id(req: &Request) -> String {
    // Try to get user ID from auth header
    if let Some(auth_header) = req.headers().get("authorization") {
        if let Ok(auth_str) = auth_header.to_str() {
            if auth_str.starts_with("Bearer ") {
                // Could extract user ID from JWT, for now use token prefix
                return format!("token:{}", &auth_str[7..min(20, auth_str.len())]);
            }
        }
    }

    // Fall back to IP
    if let Some(forwarded) = req.headers().get("x-forwarded-for") {
        if let Ok(ip) = forwarded.to_str() {
            return format!("ip:{}", ip.split(',').next().unwrap_or("unknown").trim());
        }
    }

    if let Some(real_ip) = req.headers().get("x-real-ip") {
        if let Ok(ip) = real_ip.to_str() {
            return format!("ip:{}", ip);
        }
    }

    // Fallback to a generic identifier
    "unknown".to_string()
}

fn min(a: usize, b: usize) -> usize {
    if a < b { a } else { b }
}

/// Request ID middleware
pub async fn request_id_middleware(
    mut req: Request,
    next: Next,
) -> Response {
    let request_id = req.headers()
        .get("x-request-id")
        .and_then(|h| h.to_str().ok())
        .map(|s| s.to_string())
        .unwrap_or_else(|| Uuid::new_v4().to_string());

    // Add to request extensions for handlers to access
    req.extensions_mut().insert(RequestId(request_id.clone()));

    let mut response = next.run(req).await;
    response.headers_mut().insert(
        "x-request-id",
        HeaderValue::from_str(&request_id).unwrap(),
    );

    response
}

/// Request ID extractor
#[derive(Clone, Debug)]
pub struct RequestId(pub String);

/// Tracing middleware
pub async fn tracing_middleware(
    req: Request,
    next: Next,
) -> Response {
    let method = req.method().clone();
    let path = req.uri().path().to_string();
    let request_id = req.extensions().get::<RequestId>().map(|r| r.0.clone());

    let span = tracing::info_span!(
        "http_request",
        method = %method,
        path = %path,
        request_id = request_id.as_deref().unwrap_or("unknown"),
    );

    let start = std::time::Instant::now();
    let response = next.run(req).await;
    let duration = start.elapsed();
    let status = response.status();

    tracing::info!(
        status = %status,
        duration_ms = duration.as_millis(),
        "Request completed"
    );

    response
}

/// Security headers middleware
pub async fn security_headers_middleware(
    req: Request,
    next: Next,
) -> Response {
    let mut response = next.run(req).await;

    let headers = response.headers_mut();

    // HSTS
    headers.insert(
        "strict-transport-security",
        HeaderValue::from_static("max-age=31536000; includeSubDomains"),
    );

    // CSP
    headers.insert(
        "content-security-policy",
        HeaderValue::from_static(
            "default-src 'self'; script-src 'self' 'unsafe-inline' 'unsafe-eval'; style-src 'self' 'unsafe-inline'; img-src 'self' data: https:; font-src 'self' data:; connect-src 'self' wss: https:;"
        ),
    );

    // X-Frame-Options
    headers.insert(
        "x-frame-options",
        HeaderValue::from_static("DENY"),
    );

    // X-Content-Type-Options
    headers.insert(
        "x-content-type-options",
        HeaderValue::from_static("nosniff"),
    );

    // Referrer-Policy
    headers.insert(
        "referrer-policy",
        HeaderValue::from_static("strict-origin-when-cross-origin"),
    );

    // Permissions-Policy
    headers.insert(
        "permissions-policy",
        HeaderValue::from_static("geolocation=(), microphone=(), camera=()"),
    );

    // X-XSS-Protection (legacy but still useful)
    headers.insert(
        "x-xss-protection",
        HeaderValue::from_static("1; mode=block"),
    );

    response
}

/// CORS middleware (simplified, use tower-http in production)
pub async fn cors_middleware(
    req: Request,
    next: Next,
) -> Response {
    let mut response = next.run(req).await;

    let headers = response.headers_mut();
    headers.insert(
        "access-control-allow-origin",
        HeaderValue::from_static("*"),
    );
    headers.insert(
        "access-control-allow-methods",
        HeaderValue::from_static("GET, POST, PUT, PATCH, DELETE, OPTIONS"),
    );
    headers.insert(
        "access-control-allow-headers",
        HeaderValue::from_static("Authorization, Content-Type, X-Requested-With"),
    );
    headers.insert(
        "access-control-max-age",
        HeaderValue::from_static("86400"),
    );

    response
}

/// Compression middleware (handled by tower-http in router)
pub async fn compression_middleware(
    req: Request,
    next: Next,
) -> Response {
    next.run(req).await
}

/// Request size limit middleware
pub async fn request_size_limit_middleware(
    req: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    const MAX_SIZE: usize = 100 * 1024 * 1024; // 100MB

    if let Some(content_length) = req.headers().get("content-length") {
        if let Ok(len_str) = content_length.to_str() {
            if let Ok(len) = len_str.parse::<usize>() {
                if len > MAX_SIZE {
                    warn!("Request size {} exceeds limit {}", len, MAX_SIZE);
                    return Err(StatusCode::PAYLOAD_TOO_LARGE);
                }
            }
        }
    }

    Ok(next.run(req).await)
}

/// Timeout middleware
pub async fn timeout_middleware(
    req: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    let timeout = Duration::from_secs(60);

    match tokio::time::timeout(timeout, next.run(req)).await {
        Ok(response) => Ok(response),
        Err(_) => {
            warn!("Request timed out after {:?}", timeout);
            Err(StatusCode::REQUEST_TIMEOUT)
        }
    }
}