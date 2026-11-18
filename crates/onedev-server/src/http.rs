//! Operational endpoints (`/health`, `/ready`, `/metrics`) and the assembly
//! of the complete HTTP application.

use std::sync::Arc;
use std::time::Duration;

use axum::Router;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Json, Response};
use axum::routing::get;
use serde_json::json;

use crate::lifecycle::{HealthStatus, Lifecycle};
use crate::metrics::{MetricsEndpoint, metrics_handler};
use crate::state::AppState;

/// How long the readiness probe waits for the database.
const READY_DB_TIMEOUT: Duration = Duration::from_secs(2);

#[derive(Clone)]
struct ProbeState {
    lifecycle: Arc<Lifecycle>,
    pool: onedev_db::DbPool,
}

/// The complete application below the middleware stack:
///
/// * `/health`  liveness: the process answers (also while shutting down),
/// * `/ready`   readiness: started, not shutting down, database reachable,
/// * `/metrics` Prometheus metrics (optionally behind a bearer token),
/// * `/api/**`  the REST API.
pub fn application(state: &AppState, metrics: MetricsEndpoint) -> Router {
    let probes = Router::new()
        .route("/health", get(health))
        .route("/ready", get(ready))
        .with_state(ProbeState {
            lifecycle: state.lifecycle.clone(),
            pool: state.pool.clone(),
        });
    probes
        .route("/metrics", get(metrics_handler).with_state(metrics))
        .nest("/api", onedev_api::routes::create_router(state.api.clone()))
}

async fn health(State(state): State<ProbeState>) -> Json<serde_json::Value> {
    let status = state.lifecycle.status();
    let mut body = json!({
        "status": status.as_str(),
        "version": env!("CARGO_PKG_VERSION"),
        "uptime_seconds": state.lifecycle.uptime().as_secs(),
    });
    if let HealthStatus::Degraded(reason) = status {
        body["reason"] = json!(reason);
    }
    Json(body)
}

async fn ready(State(state): State<ProbeState>) -> Response {
    let status = state.lifecycle.status();
    if !state.lifecycle.is_ready() {
        return unready(status.as_str(), "not accepting traffic");
    }
    let database = tokio::time::timeout(
        READY_DB_TIMEOUT,
        sqlx::query_scalar::<_, i32>("SELECT 1").fetch_one(&state.pool),
    )
    .await;
    match database {
        Ok(Ok(_)) => (
            StatusCode::OK,
            Json(json!({ "ready": true, "status": status.as_str() })),
        )
            .into_response(),
        _ => unready(status.as_str(), "database unavailable"),
    }
}

fn unready(status: &str, reason: &str) -> Response {
    (
        StatusCode::SERVICE_UNAVAILABLE,
        Json(json!({ "ready": false, "status": status, "reason": reason })),
    )
        .into_response()
}
