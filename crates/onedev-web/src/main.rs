//! OneDev SSR Web UI Binary

use onedev_web::{WebConfig, WebState, create_web_router};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize tracing
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "onedev_web=info,axum=info,tower_http=info".into()),
        )
        .with(tracing_subscriber::fmt::layer().json())
        .init();

    // Load configuration
    let config = WebConfig::load().map_err(|e| anyhow::anyhow!("{e}"))?;

    // Create web state
    let state = WebState::new(config.clone()).await?;

    // Create router and serve
    let app = create_web_router(state);
    let addr = format!("{}:{}", config.server.bind_address, config.server.port);
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    tracing::info!("OneDev Web UI listening on {}", addr);
    axum::serve(listener, app).await?;

    Ok(())
}
