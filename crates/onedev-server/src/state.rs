//! Shared Application State - Simplified for compilation

use std::sync::Arc;
use sqlx::PgPool;
use crate::config::AppConfig;

/// Complete application state
#[derive(Clone)]
pub struct AppState {
    pub config: Arc<AppConfig>,
    pub db_pool: Arc<PgPool>,
}

/// Initialize application state
pub async fn init_state(
    db_pool: Arc<PgPool>,
    config: Arc<AppConfig>,
) -> anyhow::Result<AppState> {
    Ok(AppState {
        config,
        db_pool,
    })
}