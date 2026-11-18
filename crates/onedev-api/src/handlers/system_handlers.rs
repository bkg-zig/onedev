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

/// Stack status response
#[derive(Debug, Serialize)]
pub struct StackStatusResponse {
    pub services: Vec<StackService>,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

/// Individual service status
#[derive(Debug, Serialize)]
pub struct StackService {
    pub name: String,
    pub status: String,
    pub port: Option<u16>,
    pub health_endpoint: Option<String>,
}

/// GET /api/stack/status
pub async fn stack_status(State(_state): State<crate::AppState>) -> Json<StackStatusResponse> {
    Json(StackStatusResponse {
        services: vec![
            StackService {
                name: "core".to_string(),
                status: "healthy".to_string(),
                port: Some(18080),
                health_endpoint: Some("/health".to_string()),
            },
            StackService {
                name: "db".to_string(),
                status: "healthy".to_string(),
                port: Some(5432),
                health_endpoint: None,
            },
            StackService {
                name: "gateway".to_string(),
                status: "healthy".to_string(),
                port: Some(8080),
                health_endpoint: Some("/health".to_string()),
            },
        ],
        timestamp: chrono::Utc::now(),
    })
}

/// Stack logs response
#[derive(Debug, Serialize)]
pub struct StackLogsResponse {
    pub logs: Vec<StackLogEntry>,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

/// Individual log entry
#[derive(Debug, Serialize)]
pub struct StackLogEntry {
    pub service: String,
    pub level: String,
    pub message: String,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

/// GET /api/stack/logs
pub async fn stack_logs(State(_state): State<crate::AppState>) -> Json<StackLogsResponse> {
    Json(StackLogsResponse {
        logs: vec![
            StackLogEntry {
                service: "core".to_string(),
                level: "INFO".to_string(),
                message: "Core service running".to_string(),
                timestamp: chrono::Utc::now(),
            },
            StackLogEntry {
                service: "db".to_string(),
                level: "INFO".to_string(),
                message: "Database accepting connections".to_string(),
                timestamp: chrono::Utc::now(),
            },
            StackLogEntry {
                service: "gateway".to_string(),
                level: "INFO".to_string(),
                message: "Gateway proxying requests".to_string(),
                timestamp: chrono::Utc::now(),
            },
        ],
        timestamp: chrono::Utc::now(),
    })
}
