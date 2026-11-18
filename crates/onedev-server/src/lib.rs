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

use axum::{Router, routing::get};
use jsonwebtoken::Algorithm;
use secrecy::SecretString;
use sqlx::PgPool;
use std::convert::Infallible;
use std::sync::Arc;
use crate::config::AppConfig;
use onedev_db::{create_pool, run_migrations};
use onedev_api::{AppState as ApiState, create_router as create_api_router};

// Application state is defined in state module
pub use state::AppState;

// Allow nesting ApiState router inside AppState router
impl axum::extract::FromRef<AppState> for ApiState {
    fn from_ref(app_state: &AppState) -> Self {
        app_state.api_state.clone()
    }
}

/// Build scheduler handle
pub struct BuildScheduler {
    inner: Arc<tokio::sync::Mutex<Option<crate::services::build_scheduler::BuildScheduler>>>,
}

impl BuildScheduler {
    pub fn new(scheduler: crate::services::build_scheduler::BuildScheduler) -> Self {
        Self {
            inner: Arc::new(tokio::sync::Mutex::new(Some(scheduler))),
        }
    }

    pub async fn running_count(&self) -> usize {
        if let Some(s) = self.inner.lock().await.as_ref() {
            s.running_count().await
        } else {
            0
        }
    }

    pub async fn queue_depth(&self) -> usize {
        if let Some(s) = self.inner.lock().await.as_ref() {
            s.queue_depth().await
        } else {
            0
        }
    }
}

/// Cleanup service handle
pub struct CleanupService {
    inner: Arc<tokio::sync::Mutex<Option<crate::services::cleanup::CleanupService>>>,
}

impl CleanupService {
    pub fn new(service: crate::services::cleanup::CleanupService) -> Self {
        Self {
            inner: Arc::new(tokio::sync::Mutex::new(Some(service))),
        }
    }

    pub async fn trigger_cleanup(&self) -> anyhow::Result<()> {
        if let Some(s) = self.inner.lock().await.as_ref() {
            s.trigger_cleanup().await
        } else {
            Ok(())
        }
    }
}

/// Git GC service handle
pub struct GitGcService {
    inner: Arc<tokio::sync::Mutex<Option<crate::services::git_gc::GitGcService>>>,
}

impl GitGcService {
    pub fn new(service: crate::services::git_gc::GitGcService) -> Self {
        Self {
            inner: Arc::new(tokio::sync::Mutex::new(Some(service))),
        }
    }

    pub async fn run_aggressive_gc(&self, project_id: uuid::Uuid) -> anyhow::Result<()> {
        if let Some(s) = self.inner.lock().await.as_ref() {
            s.run_aggressive_gc(project_id).await
        } else {
            Ok(())
        }
    }

    pub async fn get_gc_stats(&self, project_id: uuid::Uuid) -> anyhow::Result<crate::services::git_gc::GitGcStats> {
        if let Some(s) = self.inner.lock().await.as_ref() {
            s.get_gc_stats(project_id).await
        } else {
            Ok(Default::default())
        }
    }
}

/// Search indexer handle
pub struct SearchIndexer {
    inner: Arc<tokio::sync::Mutex<Option<crate::services::search_indexer::SearchIndexer>>>,
}

impl SearchIndexer {
    pub fn new(indexer: crate::services::search_indexer::SearchIndexer) -> Self {
        Self {
            inner: Arc::new(tokio::sync::Mutex::new(Some(indexer))),
        }
    }

    pub async fn trigger_reindex(&self) -> anyhow::Result<()> {
        if let Some(s) = self.inner.lock().await.as_ref() {
            s.trigger_reindex().await
        } else {
            Ok(())
        }
    }

    pub async fn get_stats(&self) -> anyhow::Result<crate::services::search_indexer::SearchIndexStats> {
        if let Some(s) = self.inner.lock().await.as_ref() {
            s.get_stats().await
        } else {
            Ok(Default::default())
        }
    }
}

/// Webhook dispatcher handle
pub struct WebhookDispatcher {
    inner: Arc<tokio::sync::Mutex<Option<crate::services::webhook_dispatcher::WebhookDispatcher>>>,
}

impl WebhookDispatcher {
    pub fn new(dispatcher: crate::services::webhook_dispatcher::WebhookDispatcher) -> Self {
        Self {
            inner: Arc::new(tokio::sync::Mutex::new(Some(dispatcher))),
        }
    }

    pub async fn queue_delivery(
        &self,
        webhook_id: uuid::Uuid,
        event: &str,
        payload: serde_json::Value,
        url: &str,
    ) -> anyhow::Result<uuid::Uuid> {
        if let Some(d) = self.inner.lock().await.as_ref() {
            d.queue_delivery(webhook_id, event, payload, url).await
        } else {
            Err(anyhow::anyhow!("Webhook dispatcher not initialized"))
        }
    }

    pub async fn get_stats(&self) -> anyhow::Result<crate::services::webhook_dispatcher::WebhookStats> {
        if let Some(d) = self.inner.lock().await.as_ref() {
            d.get_stats().await
        } else {
            Ok(Default::default())
        }
    }
}

/// Metrics collector
pub struct MetricsCollector {
    inner: Arc<tokio::sync::Mutex<Option<tokio::task::JoinHandle<()>>>>,
}

impl MetricsCollector {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(tokio::sync::Mutex::new(None)),
        }
    }
}

impl Default for MetricsCollector {
    fn default() -> Self {
        Self::new()
    }
}

/// Create the main application router
pub fn create_app_router(state: AppState) -> Router {
    // Health check endpoint
    async fn health_handler() -> &'static str { "OK" }

    // Metrics placeholder
    async fn metrics_handler() -> &'static str { "OK" }

    // Hello handler for root
    async fn hello_handler() -> &'static str { "OneDev Server - Hello Universe!" }

    // Create API router
    let api_router = create_api_router(state.api_state.clone());

    Router::<AppState>::new()
        .route("/", get(hello_handler))
        .route("/health", get(health_handler))
        .route("/metrics", get(metrics_handler))
        .route("/api/health", get(health_handler))
        .nest("/api", api_router)
        .with_state(state)
}

/// Health check endpoint
pub async fn health_handler() -> &'static str {
    "OK"
}

/// Create application state from config
pub async fn create_app_state(config: &AppConfig) -> Result<AppState> {
    // Initialize logging
    crate::init_logging(config)?;

    // Create database pool
    let db_pool = Arc::new(create_pool(&config.database.url, config.database.max_connections).await?);

    // Run migrations
    run_migrations(&db_pool).await?;

    // Initialize lifecycle manager
    let lifecycle = Arc::new(LifecycleManager::new());

    // Create AuthConfig for onedev-auth
    let private_key = config.auth.private_key.clone().unwrap_or_default();
    let public_key = config.auth.public_key.clone().unwrap_or_default();
    
    let auth_config = onedev_auth::AuthConfig {
        jwt_config: onedev_auth::models::token::TokenConfig {
            issuer: "onedev".to_string(),
            audience: "onedev".to_string(),
            access_ttl_seconds: (config.auth.token_expiry_hours * 3600) as i64,
            refresh_ttl_seconds: (config.auth.refresh_expiry_days * 86400) as i64,
            temporary_ttl_seconds: 300,
            private_key: secrecy::SecretString::new(private_key.into()),
            public_key: secrecy::SecretString::new(public_key.into()),
            algorithm: jsonwebtoken::Algorithm::RS256,
        },
        totp_config: onedev_auth::models::two_factor::TotpConfig {
            algorithm: onedev_auth::models::two_factor::TotpAlgorithm::Sha1,
            digits: 6,
            period: 30,
            skew: 1,
        },
        session_ttl_seconds: (config.auth.session_timeout_minutes * 60) as i64,
        remember_me_ttl_seconds: 30 * 24 * 60 * 60,
        max_failed_attempts: config.auth.max_failed_attempts,
        lockout_duration_seconds: (config.auth.lockout_duration_minutes * 60) as i64,
        password_reset_ttl_seconds: 60 * 60,
    };

    // Create API state using the onedev-api factory
    let api_state = onedev_api::create_app_state((*db_pool).clone(), auth_config).await?;

    // Initialize all services using state module
    let app_state = state::init_state(
        db_pool.clone(),
        Arc::new(config.clone()),
        api_state.clone(),
        lifecycle.clone(),
    ).await?;

    // Mark startup complete
    lifecycle.mark_startup_complete();

    Ok(app_state)
}

/// Run the HTTP server
pub async fn run_server(state: AppState, config: AppConfig) -> Result<()> {
    let app = create_app_router(state);

    let addr = format!("{}:{}", config.server.bind_address, config.server.port);
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