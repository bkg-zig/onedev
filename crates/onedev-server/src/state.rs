//! Shared Application State

use std::sync::Arc;
use sqlx::PgPool;
use crate::config::AppConfig;
use crate::services::*;
use crate::lifecycle::LifecycleManager;
use crate::{BuildScheduler, CleanupService, GitGcService, SearchIndexer, WebhookDispatcher, MetricsCollector};
use onedev_api::AppState as ApiState;

/// Complete application state
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

/// Initialize application state
pub async fn init_state(
    db_pool: Arc<PgPool>,
    config: Arc<AppConfig>,
    api_state: ApiState,
    lifecycle: Arc<LifecycleManager>,
) -> anyhow::Result<AppState> {
    let (
        build_scheduler,
        cleanup_service,
        git_gc_service,
        search_indexer,
        webhook_dispatcher,
        metrics_collector,
    ) = init_services(
        db_pool.clone(),
        config.clone(),
        api_state.clone(),
        lifecycle.clone(),
    ).await?;

    Ok(AppState {
        config,
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

/// Initialize all background services
pub async fn init_services(
    db_pool: Arc<PgPool>,
    config: Arc<AppConfig>,
    _api_state: ApiState,
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
            config.webhooks.clone().into(),
            lifecycle.shutdown_rx(),
        )
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