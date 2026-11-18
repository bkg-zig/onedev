//! OneDev Main Server Composition

pub mod config;
pub mod lifecycle;
pub mod metrics;
pub mod middleware;
pub mod services;
pub mod state;
pub mod telemetry;

pub use config::*;
pub use lifecycle::*;
pub use metrics::*;
pub use middleware::*;
pub use services::*;
pub use state::*;
pub use telemetry::*;

/// Re-export commonly used types
pub use uuid::Uuid;
pub use chrono::{DateTime, Utc};
pub use serde::{Serialize, Deserialize};
pub use anyhow::Result;
pub use thiserror::Error;

use axum::Router;
use std::sync::Arc;
use onedev_api::{create_router, AppState as ApiState};

/// Application state for the server
#[derive(Clone)]
pub struct AppState {
    pub api_state: ApiState,
    pub config: Arc<AppConfig>,
    pub db_pool: Arc<sqlx::PgPool>,
    pub background_workers: Arc<BackgroundWorkers>,
}

/// Background worker handles
pub struct BackgroundWorkers {
    pub build_executor: tokio::task::JoinHandle<()>,
    pub cleanup_worker: tokio::task::JoinHandle<()>,
    pub webhook_worker: tokio::task::JoinHandle<()>,
    pub search_indexer: tokio::task::JoinHandle<()>,
    pub metrics_collector: tokio::task::JoinHandle<()>,
}

/// Create the main application router
pub fn create_app_router(state: AppState) -> Router {
    let api_router = create_router(state.api_state.clone());

    // Create WebState for the web UI
    let web_config = onedev_web::WebConfig {
        bind_address: state.config.server.bind_address.clone(),
        port: state.config.server.port,
        template_dir: "templates".to_string(),
        static_dir: "static".to_string(),
        api_base_url: format!("http://{}:{}", state.config.server.bind_address, state.config.server.port),
        enable_ssr: true,
        dev_mode: state.config.server.environment == "development",
    };
    
    let web_state = onedev_web::WebState {
        api_state: state.api_state.clone(),
        config: Arc::new(web_config),
        tera: {
            let mut tera = tera::Tera::new("templates/**/*").unwrap_or_else(|_| tera::Tera::default());
            tera.autoescape_on(vec![".html", ".htm"]);
            tera.register_filter("markdown", onedev_web::markdown_filter);
            tera.register_filter("datefmt", onedev_web::date_filter);
            tera.register_filter("truncate", onedev_web::truncate_filter);
            Arc::new(tera)
        },
    };

    Router::new()
        .nest("/api", api_router)
        // Web UI routes (SSR)
        .nest("/", onedev_web::create_web_router(web_state))
        // Metrics endpoint
        .route("/metrics", axum::routing::get(metrics::metrics_handler))
        // Health check (non-API)
        .route("/health", axum::routing::get(health_handler))
        .with_state(state)
}

/// Health check endpoint
async fn health_handler() -> &'static str {
    "OK"
}

/// Create application state from config
pub async fn create_app_state(config: &AppConfig) -> Result<AppState> {
    // Create database pool
    let db_pool = Arc::new(onedev_db::create_pool(&config.database_url, config.database_max_connections).await?);

    // Run migrations
    onedev_db::run_migrations(&db_pool).await?;

    // Create core services
    let auth_service = Arc::new(onedev_auth::services::AuthService::new(
        db_pool.clone(),
        config.jwt_secret.clone(),
        config.token_expiry_hours,
    ));

    let authorization_service = Arc::new(onedev_api::services::AuthorizationService::new(
        db_pool.clone(),
    ));

    let user_service = Arc::new(onedev_api::services::UserService::new(db_pool.clone()));
    let project_service = Arc::new(onedev_api::services::ProjectService::new(db_pool.clone()));
    let group_service = Arc::new(onedev_api::services::GroupService::new(db_pool.clone()));
    let issue_service = Arc::new(onedev_api::services::IssueService::new(db_pool.clone()));
    let mr_service = Arc::new(onedev_api::services::MergeRequestService::new(db_pool.clone()));
    let build_service = Arc::new(onedev_api::services::BuildService::new(db_pool.clone()));
    let git_service = Arc::new(onedev_api::services::GitService::new(db_pool.clone()));
    let lfs_service = Arc::new(onedev_api::services::LfsService::new(db_pool.clone()));
    let system_service = Arc::new(onedev_api::services::SystemService::new(db_pool.clone()));

    let api_state = onedev_api::AppState {
        auth_service,
        authorization_service,
        user_service,
        project_service,
        group_service,
        issue_service,
        mr_service,
        build_service,
        git_service,
        lfs_service,
        system_service,
    };

    // Start background workers
    let background_workers = start_background_workers(db_pool.clone(), config.clone()).await?;

    Ok(AppState {
        api_state,
        config: Arc::new(config.clone()),
        db_pool,
        background_workers: Arc::new(background_workers),
    })
}

/// Start background workers
async fn start_background_workers(
    db_pool: Arc<sqlx::PgPool>,
    config: AppConfig,
) -> Result<BackgroundWorkers> {
    use onedev_build::services::BuildExecutor;
    use onedev_core::events::EventBus;

    let event_bus = Arc::new(EventBus::new());

    // Build executor
    let build_executor = {
        let db = db_pool.clone();
        let executor = BuildExecutor::new(db.clone(), event_bus.clone());
        tokio::spawn(async move {
            executor.run().await;
        })
    };

    // Cleanup worker
    let cleanup_worker = {
        let db = db_pool.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(3600));
            loop {
                interval.tick().await;
                tracing::info!("Running cleanup tasks...");
                // Cleanup old build logs, temp files, etc.
                if let Err(e) = onedev_db::cleanup_old_data(&db).await {
                    tracing::error!("Cleanup failed: {}", e);
                }
            }
        })
    };

    // Webhook worker
    let webhook_worker = {
        let db = db_pool.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(30));
            loop {
                interval.tick().await;
                if let Err(e) = onedev_db::process_webhook_queue(&db).await {
                    tracing::error!("Webhook processing failed: {}", e);
                }
            }
        })
    };

    // Search indexer
    let search_indexer = {
        let db = db_pool.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(60));
            loop {
                interval.tick().await;
                if let Err(e) = onedev_db::update_search_index(&db).await {
                    tracing::error!("Search indexing failed: {}", e);
                }
            }
        })
    };

    // Metrics collector
    let metrics_collector = {
        let db = db_pool.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(15));
            loop {
                interval.tick().await;
                if let Err(e) = onedev_server::metrics::collect_system_metrics(&db).await {
                    tracing::error!("Metrics collection failed: {}", e);
                }
            }
        })
    };

    Ok(BackgroundWorkers {
        build_executor,
        cleanup_worker,
        webhook_worker,
        search_indexer,
        metrics_collector,
    })
}

/// Run the HTTP server
pub async fn run_server(state: AppState, config: AppConfig) -> Result<()> {
    let app = create_app_router(state);

    // Add middleware
    let app = app
        .layer(tower_http::trace::TraceLayer::new_for_http())
        .layer(tower_http::cors::CorsLayer::permissive())
        .layer(tower_http::compression::CompressionLayer::new())
        .layer(tower::ServiceBuilder::new()
            .layer(tower::limit::ConcurrencyLimitLayer::new(config.max_concurrent_requests))
            .layer(tower::timeout::TimeoutLayer::new(
                std::time::Duration::from_secs(config.request_timeout_secs),
            ))
        );

    let addr = format!("{}:{}", config.bind_address, config.port);
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    tracing::info!("OneDev server listening on {}", addr);

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    Ok(())
}

/// Graceful shutdown signal handler
async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("Failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("Failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {
            tracing::info!("Received Ctrl+C, shutting down gracefully...");
        },
        _ = terminate => {
            tracing::info!("Received SIGTERM, shutting down gracefully...");
        },
    }
}