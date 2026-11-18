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

// Conversion traits for config
impl From<crate::config::BuildConfig> for crate::services::build_scheduler::BuildSchedulerConfig {
    fn from(c: crate::config::BuildConfig) -> Self {
        Self {
            max_concurrent_builds: c.max_concurrent_builds,
            default_timeout_minutes: c.default_timeout_minutes,
            queue_poll_interval_secs: 10,
            stuck_build_threshold_minutes: 10,
        }
    }
}

impl From<crate::config::CleanupConfig> for crate::services::cleanup::CleanupConfig {
    fn from(c: crate::config::CleanupConfig) -> Self {
        Self {
            build_log_retention_days: c.build_log_retention_days,
            artifact_retention_days: c.artifact_retention_days,
            temp_file_retention_hours: c.temp_file_retention_hours,
            audit_log_retention_days: c.audit_log_retention_days,
            webhook_log_retention_days: c.webhook_log_retention_days,
            search_index_cleanup_days: c.search_index_cleanup_days,
            run_interval_hours: c.run_interval_hours,
        }
    }
}

impl From<crate::config::GitConfig> for crate::services::git_gc::GitGcConfig {
    fn from(c: crate::config::GitConfig) -> Self {
        Self {
            repos_root: c.repos_root,
            gc_interval_hours: c.gc_interval_hours,
            auto_gc_threshold: 1000,
            aggressive: false,
            prune_expire: "2.weeks.ago".to_string(),
            max_repos_per_run: 10,
        }
    }
}

impl From<crate::config::SearchConfig> for crate::services::search_indexer::SearchIndexerConfig {
    fn from(c: crate::config::SearchConfig) -> Self {
        Self {
            batch_size: c.batch_size,
            index_interval_secs: c.index_interval_secs,
            full_reindex_interval_hours: c.full_reindex_interval_hours,
            enable_code_search: c.enable_code_search,
            enable_issue_search: c.enable_issue_search,
            enable_wiki_search: c.enable_wiki_search,
        }
    }
}

impl From<crate::config::WebhookConfig> for crate::services::webhook_dispatcher::WebhookDispatcherConfig {
    fn from(c: crate::config::WebhookConfig) -> Self {
        Self {
            max_retries: c.max_retries,
            retry_delay_secs: c.retry_delay_secs,
            max_retry_delay_secs: c.max_retry_delay_secs,
            timeout_secs: c.timeout_secs,
            verify_ssl: c.verify_ssl,
            poll_interval_secs: 30,
            batch_size: 50,
        }
    }
}