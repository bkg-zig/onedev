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
use sqlx::PgPool;
use std::sync::Arc;
use crate::config::AppConfig;
use onedev_db::{create_pool, run_migrations};
use onedev_api::{AppState as ApiState, create_router as create_api_router};

/// Application state for the server
#[derive(Clone)]
pub struct AppState {
    pub config: Arc<AppConfig>,
    pub db_pool: Arc<PgPool>,
    pub api_state: ApiState,
    pub lifecycle: Arc<LifecycleManager>,
    pub build_scheduler: Arc<BuildScheduler>,
    pub cleanup_service: Arc<CleanupService>,
    pub git_gc_service: Arc<GitGcService>,
    pub search_indexer: Arc<SearchIndexer>,
    pub webhook_dispatcher: Arc<WebhookDispatcher>,
    pub metrics_collector: Arc<MetricsCollector>,
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
    // Initialize Tera template engine for web UI
    let _tera = tera::Tera::new("templates/**/*").unwrap_or_else(|_| tera::Tera::default());

    // Health check endpoint
    async fn health_handler() -> &'static str { "OK" }

    // Metrics placeholder
    async fn metrics_handler() -> &'static str { "OK" }

    // Hello handler for root
    async fn hello_handler() -> &'static str { "OneDev Server - Hello Universe!" }

    // Create API router
    let api_router = create_api_router(state.api_state.clone());

    Router::new()
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

    // Initialize API state (services)
    let api_state = create_api_state(db_pool.clone(), config, lifecycle.clone()).await?;

    // Initialize all services
    let (
        build_scheduler,
        cleanup_service,
        git_gc_service,
        search_indexer,
        webhook_dispatcher,
        metrics_collector,
    ) = state::init_services(
        db_pool.clone(),
        Arc::new(config.clone()),
        api_state.clone(),
        lifecycle.clone(),
    ).await?;

    // Mark startup complete
    lifecycle.mark_startup_complete();

    Ok(AppState {
        config: Arc::new(config.clone()),
        db_pool,
        api_state,
        lifecycle,
        build_scheduler,
        cleanup_service,
        git_gc_service,
        search_indexer,
        webhook_dispatcher,
        metrics_collector,
    })
}

/// Create API state with all services
async fn create_api_state(
    db_pool: Arc<PgPool>,
    config: &AppConfig,
    lifecycle: Arc<LifecycleManager>,
) -> Result<ApiState> {
    use onedev_auth::{AuthService, AuthorizationService, SessionService, TokenService, TwoFactorService};
    use onedev_db::{
        UserRepository, ProjectRepository, GroupRepository, IssueRepository,
        PullRequestRepository, BuildRepository, RoleRepository, MembershipRepository,
    };

    // Create repositories
    let user_repo = UserRepository::new();
    let project_repo = ProjectRepository::new();
    let group_repo = GroupRepository::new();
    let issue_repo = IssueRepository::new();
    let mr_repo = PullRequestRepository::new();
    let build_repo = BuildRepository::new();
    let role_repo = RoleRepository::new();
    let membership_repo = MembershipRepository::new();

    // Create auth services
    let auth_config = onedev_auth::AuthConfig {
        jwt_secret: config.auth.jwt_secret.clone(),
        token_expiry_hours: config.auth.token_expiry_hours,
        refresh_expiry_days: config.auth.refresh_expiry_days,
        argon2_memory_kb: config.auth.argon2_memory_kb,
        argon2_iterations: config.auth.argon2_iterations,
        argon2_parallelism: config.auth.argon2_parallelism,
        enable_2fa: config.auth.enable_2fa,
        two_fa_issuer: config.auth.two_fa_issuer.clone(),
        password_min_length: config.auth.password_min_length,
        password_require_uppercase: config.auth.password_require_uppercase,
        password_require_lowercase: config.auth.password_require_lowercase,
        password_require_digits: config.auth.password_require_digits,
        password_require_special: config.auth.password_require_special,
        session_timeout_minutes: config.auth.session_timeout_minutes,
        max_failed_attempts: config.auth.max_failed_attempts,
        lockout_duration_minutes: config.auth.lockout_duration_minutes,
    };

    let auth_service = AuthService::new(
        db_pool.clone(),
        user_repo.clone(),
        auth_config,
        lifecycle.shutdown_rx(),
    );

    let authorization_service = AuthorizationService::new(
        db_pool.clone(),
        user_repo.clone(),
        project_repo.clone(),
        role_repo.clone(),
        membership_repo.clone(),
        group_repo.clone(),
    );

    let session_service = SessionService::new(db_pool.clone(), config.auth.session_timeout_minutes);
    let token_service = TokenService::new(db_pool.clone());
    let two_factor_service = TwoFactorService::new(db_pool.clone());

    // Create API services
    let auth_svc = Arc::new(crate::services::auth::AuthService::new(auth_service));
    let authz_svc = Arc::new(crate::services::authz::AuthorizationService::new(authorization_service));
    let user_svc = Arc::new(crate::services::UserService::new(user_repo));
    let project_svc = Arc::new(crate::services::ProjectService::new(project_repo));
    let group_svc = Arc::new(crate::services::GroupService::new(group_repo));
    let issue_svc = Arc::new(crate::services::IssueService::new(issue_repo));
    let mr_svc = Arc::new(crate::services::MergeRequestService::new(mr_repo));
    let build_svc = Arc::new(crate::services::BuildService::new(build_repo));

    // Create stub services for git, lfs, system
    let git_svc = Arc::new(crate::services::GitService::new(db_pool.clone()));
    let lfs_svc = Arc::new(crate::services::LfsService::new(db_pool.clone()));
    let system_svc = Arc::new(crate::services::SystemService::new(db_pool.clone()));

    Ok(ApiState {
        auth_service: auth_svc,
        authorization_service: authz_svc,
        user_service: user_svc,
        project_service: project_svc,
        group_service: group_svc,
        issue_service: issue_svc,
        mr_service: mr_svc,
        build_service: build_svc,
        git_service: git_svc,
        lfs_service: lfs_svc,
        system_service: system_svc,
    })
}

/// Initialize all services and return handles
pub async fn init_services(
    db_pool: Arc<PgPool>,
    config: Arc<AppConfig>,
    api_state: ApiState,
    lifecycle: Arc<LifecycleManager>,
) -> anyhow::Result<(
    Arc<BuildScheduler>,
    Arc<CleanupService>,
    Arc<GitGcService>,
    Arc<SearchIndexer>,
    Arc<WebhookDispatcher>,
    Arc<MetricsCollector>,
)> {
    // Build scheduler
    let build_scheduler = Arc::new(BuildScheduler::new(
        crate::services::build_scheduler::BuildScheduler::new(
            db_pool.clone(),
            config.build.clone().into(),
            lifecycle.shutdown_rx(),
        )
    ));

    // Cleanup service
    let cleanup_service = Arc::new(CleanupService::new(
        crate::services::cleanup::CleanupService::new(
            db_pool.clone(),
            config.cleanup.clone().into(),
            lifecycle.shutdown_rx(),
        )
    ));

    // Git GC service
    let git_gc_service = Arc::new(GitGcService::new(
        crate::services::git_gc::GitGcService::new(
            db_pool.clone(),
            config.git.clone().into(),
            lifecycle.shutdown_rx(),
        )
    ));

    // Search indexer
    let search_indexer = Arc::new(SearchIndexer::new(
        crate::services::search_indexer::SearchIndexer::new(
            db_pool.clone(),
            config.search.clone().into(),
            lifecycle.shutdown_rx(),
        )
    ));

    // Webhook dispatcher
    let webhook_dispatcher = Arc::new(WebhookDispatcher::new(
        crate::services::webhook_dispatcher::WebhookDispatcher::new(
            db_pool.clone(),
            config.webhook.clone().into(),
            lifecycle.shutdown_rx(),
        )?
    ));

    // Metrics collector
    let metrics_collector = Arc::new(MetricsCollector::new());

    Ok((
        build_scheduler,
        cleanup_service,
        git_gc_service,
        search_indexer,
        webhook_dispatcher,
        metrics_collector,
    ))
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