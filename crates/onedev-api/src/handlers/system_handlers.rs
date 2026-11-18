//! System HTTP Handlers

use crate::AppState;
use axum::{extract::State, http::StatusCode, response::Json};
use serde::Serialize;

/// Health check response
#[derive(Debug, Serialize)]
pub struct HealthResponse {
    pub status: String,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub version: String,
    pub uptime_seconds: u64,
}

/// Readiness check response
#[derive(Debug, Serialize)]
pub struct ReadyResponse {
    pub ready: bool,
    pub checks: Vec<ReadinessCheck>,
}

/// Individual readiness check
#[derive(Debug, Serialize)]
pub struct ReadinessCheck {
    pub name: String,
    pub status: String,
    pub message: Option<String>,
}

/// Version response
#[derive(Debug, Serialize)]
pub struct VersionResponse {
    pub version: String,
    pub git_commit: Option<String>,
    pub build_date: Option<String>,
    pub rust_version: String,
}

/// GET /health
pub async fn health(State(_state): State<crate::AppState>) -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "healthy".to_string(),
        timestamp: chrono::Utc::now(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        uptime_seconds: 0, // TODO: Track actual uptime
    })
}

/// GET /ready
pub async fn ready(
    State(_state): State<crate::AppState>,
) -> Result<Json<ReadyResponse>, StatusCode> {
    let checks = vec![
        ReadinessCheck {
            name: "database".to_string(),
            status: "ok".to_string(), // TODO: Check actual DB connection
            message: None,
        },
        ReadinessCheck {
            name: "cache".to_string(),
            status: "ok".to_string(),
            message: None,
        },
    ];

    let ready = checks.iter().all(|c| c.status == "ok");

    Ok(Json(ReadyResponse { ready, checks }))
}

/// GET /version
pub async fn version(State(_state): State<crate::AppState>) -> Json<VersionResponse> {
    Json(VersionResponse {
        version: env!("CARGO_PKG_VERSION").to_string(),
        git_commit: option_env!("GIT_COMMIT").map(String::from),
        build_date: option_env!("BUILD_DATE").map(String::from),
        rust_version: option_env!("RUSTC_VERSION")
            .unwrap_or("unknown")
            .to_string(),
    })
}
