//! Prometheus metrics: recorder installation, metric descriptions, the
//! `/metrics` endpoint and the periodic collector (DB pool gauges, upkeep).

use std::sync::{Mutex, OnceLock};
use std::time::Duration;

use anyhow::Context as _;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{IntoResponse, Response};
use metrics::{Unit, describe_counter, describe_gauge, describe_histogram, gauge};
use metrics_exporter_prometheus::{Matcher, PrometheusBuilder, PrometheusHandle};
use sha2::{Digest, Sha256};

/// Latency buckets (seconds) for request-like operations.
const LATENCY_BUCKETS: &[f64] = &[
    0.001, 0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0, 30.0, 60.0, 300.0,
    1800.0, 3600.0,
];

static HANDLE: OnceLock<PrometheusHandle> = OnceLock::new();
static INSTALL_LOCK: Mutex<()> = Mutex::new(());

/// Install the global recorder (idempotent: later calls return the same handle).
pub fn install() -> anyhow::Result<PrometheusHandle> {
    let _guard = INSTALL_LOCK.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    if let Some(handle) = HANDLE.get() {
        return Ok(handle.clone());
    }
    let handle = PrometheusBuilder::new()
        .set_buckets_for_metric(Matcher::Suffix("_duration_seconds".to_string()), LATENCY_BUCKETS)
        .context("invalid histogram buckets")?
        .install_recorder()
        .context("failed to install the Prometheus recorder (another recorder is active)")?;
    describe_metrics();
    let _ = HANDLE.set(handle.clone());
    Ok(handle)
}

fn describe_metrics() {
    describe_counter!("http_requests_total", "HTTP requests by method, route and status");
    describe_histogram!(
        "http_request_duration_seconds",
        Unit::Seconds,
        "HTTP request latency by method and route"
    );
    describe_gauge!("http_requests_in_flight", "HTTP requests currently being processed");
    describe_counter!(
        "http_requests_rejected_total",
        "Requests rejected before reaching a handler (rate_limited, overloaded, body_too_large, timeout)"
    );
    describe_counter!("http_panics_total", "Handler panics converted to HTTP 500");
    describe_gauge!("db_pool_connections", "Database pool connections by state (idle, active, max)");
    describe_gauge!("build_queue_depth", "Pending builds");
    describe_gauge!("builds_running", "Builds currently executing");
    describe_counter!("builds_total", "Finished builds by final status");
    describe_histogram!("build_duration_seconds", Unit::Seconds, "Build wall-clock duration");
    describe_counter!("build_scheduler_errors_total", "Build scheduler iterations that failed");
    describe_counter!("webhook_deliveries_total", "Webhook delivery attempts by outcome");
    describe_histogram!(
        "webhook_delivery_duration_seconds",
        Unit::Seconds,
        "Webhook HTTP request duration"
    );
    describe_gauge!("webhook_queue_depth", "Webhook deliveries waiting to be sent");
    describe_counter!("search_documents_indexed_total", "Documents written to the search index by kind");
    describe_histogram!(
        "search_index_duration_seconds",
        Unit::Seconds,
        "Duration of an indexing cycle by kind"
    );
    describe_counter!("git_gc_runs_total", "Repository maintenance runs by outcome");
    describe_histogram!("git_gc_duration_seconds", Unit::Seconds, "Duration of a repository maintenance run");
    describe_counter!("cleanup_rows_deleted_total", "Rows removed by retention cleanup by table");
    describe_counter!("cleanup_runs_total", "Retention cleanup cycles by outcome");
    describe_counter!("otel_spans_exported_total", "Spans delivered to the OTLP collector");
    describe_counter!("otel_spans_dropped_total", "Spans dropped because the export queue was full");
    describe_counter!("otel_export_failures_total", "Failed OTLP export requests");
    describe_counter!("background_task_restarts_total", "Background service iterations that failed and were retried");
    describe_gauge!("onedev_build_info", "Build information (always 1)");
    describe_gauge!("onedev_uptime_seconds", "Seconds since the server started");
}

/// State of the `/metrics` endpoint.
#[derive(Clone)]
pub struct MetricsEndpoint {
    pub handle: PrometheusHandle,
    pub enabled: bool,
    /// SHA-256 of the configured bearer token (None = endpoint is open).
    token_digest: Option<[u8; 32]>,
}

impl MetricsEndpoint {
    pub fn new(handle: PrometheusHandle, enabled: bool, token: Option<&str>) -> Self {
        Self {
            handle,
            enabled,
            token_digest: token.filter(|t| !t.is_empty()).map(digest),
        }
    }

    fn authorized(&self, headers: &HeaderMap) -> bool {
        let Some(expected) = self.token_digest else {
            return true;
        };
        let presented = headers
            .get(header::AUTHORIZATION)
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.strip_prefix("Bearer "))
            .map(digest);
        match presented {
            Some(actual) => constant_time_eq(&actual, &expected),
            None => false,
        }
    }
}

fn digest(value: &str) -> [u8; 32] {
    Sha256::digest(value.as_bytes()).into()
}

fn constant_time_eq(a: &[u8; 32], b: &[u8; 32]) -> bool {
    a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

/// `GET /metrics`
pub async fn metrics_handler(State(endpoint): State<MetricsEndpoint>, headers: HeaderMap) -> Response {
    if !endpoint.enabled {
        return StatusCode::NOT_FOUND.into_response();
    }
    if !endpoint.authorized(&headers) {
        return (
            StatusCode::UNAUTHORIZED,
            [(header::WWW_AUTHENTICATE, "Bearer realm=\"metrics\"")],
            "unauthorized",
        )
            .into_response();
    }
    (
        [(header::CONTENT_TYPE, "text/plain; version=0.0.4; charset=utf-8")],
        endpoint.handle.render(),
    )
        .into_response()
}

/// Periodically refresh pool gauges and run the recorder's upkeep.
pub async fn run_collector(
    pool: onedev_db::DbPool,
    handle: PrometheusHandle,
    interval: Duration,
    started: std::time::Instant,
    mut shutdown: crate::lifecycle::ShutdownSignal,
) {
    gauge!("onedev_build_info", "version" => env!("CARGO_PKG_VERSION")).set(1.0);
    let mut ticker = tokio::time::interval(interval.max(Duration::from_secs(1)));
    ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    loop {
        tokio::select! {
            _ = ticker.tick() => {
                let size = pool.size() as f64;
                let idle = pool.num_idle() as f64;
                gauge!("db_pool_connections", "state" => "idle").set(idle);
                gauge!("db_pool_connections", "state" => "active").set((size - idle).max(0.0));
                gauge!("db_pool_connections", "state" => "max")
                    .set(f64::from(pool.options().get_max_connections()));
                gauge!("onedev_uptime_seconds").set(started.elapsed().as_secs_f64());
                handle.run_upkeep();
            }
            _ = shutdown.wait() => break,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::to_bytes;

    #[test]
    fn install_is_idempotent_and_renders_recorded_metrics() {
        let first = install().expect("first install");
        let second = install().expect("second install returns the shared handle");
        metrics::counter!("test_metrics_counter_total", "kind" => "unit").increment(3);
        metrics::histogram!("test_metrics_latency_duration_seconds").record(0.2);
        let rendered = second.render();
        assert!(rendered.contains("test_metrics_counter_total{kind=\"unit\"} 3"), "{rendered}");
        assert!(rendered.contains("test_metrics_latency_duration_seconds_bucket"), "{rendered}");
        assert!(first.render().contains("test_metrics_counter_total"));
    }

    #[tokio::test]
    async fn endpoint_enforces_the_bearer_token() {
        let handle = install().unwrap();
        let endpoint = MetricsEndpoint::new(handle, true, Some("s3cret"));

        let denied = metrics_handler(State(endpoint.clone()), HeaderMap::new()).await;
        assert_eq!(denied.status(), StatusCode::UNAUTHORIZED);

        let mut wrong = HeaderMap::new();
        wrong.insert(header::AUTHORIZATION, "Bearer nope".parse().unwrap());
        assert_eq!(
            metrics_handler(State(endpoint.clone()), wrong).await.status(),
            StatusCode::UNAUTHORIZED
        );

        let mut ok = HeaderMap::new();
        ok.insert(header::AUTHORIZATION, "Bearer s3cret".parse().unwrap());
        let response = metrics_handler(State(endpoint), ok).await;
        assert_eq!(response.status(), StatusCode::OK);
        let body = to_bytes(response.into_body(), 1 << 20).await.unwrap();
        assert!(std::str::from_utf8(&body).is_ok());
    }

    #[tokio::test]
    async fn disabled_endpoint_is_not_found_and_open_endpoint_needs_no_token() {
        let handle = install().unwrap();
        let disabled = MetricsEndpoint::new(handle.clone(), false, None);
        assert_eq!(
            metrics_handler(State(disabled), HeaderMap::new()).await.status(),
            StatusCode::NOT_FOUND
        );
        let open = MetricsEndpoint::new(handle, true, None);
        assert_eq!(
            metrics_handler(State(open), HeaderMap::new()).await.status(),
            StatusCode::OK
        );
    }
}
