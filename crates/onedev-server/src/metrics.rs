//! Metrics Collection and Export

use anyhow::Result;
use axum::{
    extract::State,
    response::{IntoResponse, Response},
    routing::get,
    Router,
};
use metrics::{counter, gauge, histogram};
use metrics_exporter_prometheus::{PrometheusBuilder, PrometheusHandle};
use regex::Regex;
use sqlx::{PgPool, Row};
use std::time::{Duration, Instant};
use tracing::{info, debug};

/// Metrics handle for Prometheus exposition
pub type MetricsHandle = PrometheusHandle;

/// Initialize metrics recorder
pub fn init_metrics() -> MetricsHandle {
    let builder = PrometheusBuilder::new()
        .set_buckets(&[
            0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0,
        ]);

    let handle = builder.install_recorder().expect("Failed to install Prometheus recorder");
    info!("Prometheus metrics initialized");
    handle
}

/// Create metrics router
pub fn create_metrics_router(handle: MetricsHandle) -> Router {
    Router::new().route(
        "/metrics",
        get(move || async move { metrics_handler(handle.clone()).await }),
    )
}

/// Prometheus metrics handler
pub async fn metrics_handler(handle: MetricsHandle) -> impl IntoResponse {
    handle.render()
}

/// Record HTTP request metrics
pub fn record_http_request(
    method: &str,
    path: &str,
    status: u16,
    duration: Duration,
) {
    let labels = [
        ("method", method.to_string()),
        ("path", normalize_path(path)),
        ("status", status.to_string()),
    ];

    counter!("http_requests_total", &labels).increment(1);
    histogram!("http_request_duration_seconds", &labels).record(duration.as_secs_f64());

    if status >= 500 {
        counter!("http_requests_server_errors_total", &labels).increment(1);
    } else if status >= 400 {
        counter!("http_requests_client_errors_total", &labels).increment(1);
    }
}

/// Record database query metrics
pub fn record_db_query(query_type: &str, duration: Duration, success: bool) {
    let labels = [
        ("query_type", query_type.to_string()),
        ("success", success.to_string()),
    ];

    counter!("db_queries_total", &labels).increment(1);
    histogram!("db_query_duration_seconds", &labels).record(duration.as_secs_f64());
}

/// Record build metrics
pub fn record_build_metrics(
    project_id: &str,
    status: &str,
    duration: Duration,
    queue_time: Duration,
) {
    let labels = [
        ("project_id", project_id.to_string()),
        ("status", status.to_string()),
    ];

    counter!("builds_total", &labels).increment(1);
    histogram!("build_duration_seconds", &labels).record(duration.as_secs_f64());
    histogram!("build_queue_time_seconds", &labels).record(queue_time.as_secs_f64());
}

/// Record Git operation metrics
pub fn record_git_operation(operation: &str, repo_path: &str, duration: Duration, success: bool) {
    let labels = [
        ("operation", operation.to_string()),
        ("repo", repo_path.to_string()),
        ("success", success.to_string()),
    ];

    counter!("git_operations_total", &labels).increment(1);
    histogram!("git_operation_duration_seconds", &labels).record(duration.as_secs_f64());
}

/// Record webhook delivery metrics
pub fn record_webhook_delivery(
    webhook_id: &str,
    event: &str,
    status: &str,
    duration: Duration,
    attempt: u32,
) {
    let labels = [
        ("webhook_id", webhook_id.to_string()),
        ("event", event.to_string()),
        ("status", status.to_string()),
    ];

    counter!("webhook_deliveries_total", &labels).increment(1);
    histogram!("webhook_delivery_duration_seconds", &labels).record(duration.as_secs_f64());
    gauge!("webhook_delivery_attempts", &labels).set(attempt as f64);
}

/// Record search index metrics
pub fn record_search_index(entity_type: &str, count: usize, duration: Duration) {
    let labels = [("entity_type", entity_type.to_string())];

    counter!("search_indexed_total", &labels).increment(count as u64);
    histogram!("search_index_duration_seconds", &labels).record(duration.as_secs_f64());
}

/// Record active connections
pub fn set_active_connections(count: usize) {
    gauge!("active_connections").set(count as f64);
}

/// Record queue depth
pub fn set_queue_depth(queue_name: &str, depth: usize) {
    let labels = [("queue", queue_name.to_string())];
    gauge!("queue_depth", &labels).set(depth as f64);
}

/// Record cache hit/miss
pub fn record_cache(cache_name: &str, hit: bool) {
    let labels = [("cache", cache_name.to_string()), ("result", if hit { "hit" } else { "miss" }.to_string())];
    counter!("cache_operations_total", &labels).increment(1);
}

/// Collect system metrics periodically
pub async fn collect_system_metrics(db_pool: &PgPool) -> Result<()> {
    let start = Instant::now();

    // Database connections
    if let Ok(stats) = sqlx::query("SELECT count(*) as count FROM pg_stat_activity WHERE state = 'active'")
        .fetch_one(db_pool).await {
        let count: i64 = stats.try_get("count")?;
        gauge!("db_active_connections").set(count as f64);
    }

    // Database size
    if let Ok(size) = sqlx::query("SELECT pg_database_size(current_database()) as size")
        .fetch_one(db_pool).await {
        let s: i64 = size.try_get("size")?;
        gauge!("db_size_bytes").set(s as f64);
    }

    // Table sizes
    if let Ok(tables) = sqlx::query(
        r#"
        SELECT schemaname, tablename, pg_total_relation_size(schemaname||'.'||tablename) as size
        FROM pg_tables
        WHERE schemaname = 'public'
        ORDER BY size DESC
        LIMIT 20
        "#
    ).fetch_all(db_pool).await {
        for table in tables {
            let schema: String = table.try_get("schemaname")?;
            let tablename: String = table.try_get("tablename")?;
            let size: i64 = table.try_get("size")?;
            let labels = [("schema", schema), ("table", tablename)];
            gauge!("db_table_size_bytes", &labels).set(size as f64);
        }
    }

    // Build queue depth
    if let Ok(count) = sqlx::query("SELECT count(*) as count FROM builds WHERE status = 'Pending'")
        .fetch_one(db_pool).await {
        let c: i64 = count.try_get("count")?;
        gauge!("build_queue_depth").set(c as f64);
    }

    // Running builds
    if let Ok(count) = sqlx::query("SELECT count(*) as count FROM builds WHERE status = 'Running'")
        .fetch_one(db_pool).await {
        let c: i64 = count.try_get("count")?;
        gauge!("builds_running").set(c as f64);
    }

    // Webhook queue depth
    if let Ok(count) = sqlx::query("SELECT count(*) as count FROM webhook_deliveries WHERE status = 'Pending'")
        .fetch_one(db_pool).await {
        let c: i64 = count.try_get("count")?;
        gauge!("webhook_queue_depth").set(c as f64);
    }

    // Search index size
    if let Ok(count) = sqlx::query("SELECT count(*) as count FROM search_index")
        .fetch_one(db_pool).await {
        let c: i64 = count.try_get("count")?;
        gauge!("search_index_size").set(c as f64);
    }

    debug!("System metrics collected in {:?}", start.elapsed());
    Ok(())
}

/// Normalize path for metrics (remove IDs, etc.)
fn normalize_path(path: &str) -> String {
    let mut normalized = path.to_string();

    // Replace UUIDs
    let uuid_regex = Regex::new(r"/[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}").unwrap();
    normalized = uuid_regex.replace_all(&normalized, "/:id").to_string();

    // Replace numeric IDs
    let id_regex = Regex::new(r"/\d+").unwrap();
    normalized = id_regex.replace_all(&normalized, "/:id").to_string();

    normalized
}

/// Middleware to record HTTP metrics
pub async fn metrics_middleware(
    State(handle): State<MetricsHandle>,
    req: axum::http::Request<axum::body::Body>,
    next: axum::middleware::Next,
) -> Response {
    let start = Instant::now();
    let method = req.method().to_string();
    let path = req.uri().path().to_string();

    let response = next.run(req).await;
    let status = response.status().as_u16();
    let duration = start.elapsed();

    record_http_request(&method, &path, status, duration);

    response
}