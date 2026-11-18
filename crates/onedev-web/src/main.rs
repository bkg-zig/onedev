//! OneDev SSR Web UI Binary

use onedev_web::{WebConfig, WebState};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize tracing
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::try_from_default_env()
            .unwrap_or_else(|_| "onedev_web=info,axum=info,tower_http=info".into()))
        .with(tracing_subscriber::fmt::layer().json())
        .init();

    // Load configuration
    let config = WebConfig::load()?;

    // Create web state
    let state = WebState::new(config.clone()).await?;

    // Create router
    let _app = onedev_web::create_web_router(state);

    let addr = format!("{}:{}", config.server.bind_address, config.server.port);
    tracing::info!("OneDev Web UI configured for {} (library ready)", addr);
    tracing::info!("Note: Server binary placeholder - implement proper axum 0.8.x serve pattern");

    // Server is a stub for now
    Ok(())
}