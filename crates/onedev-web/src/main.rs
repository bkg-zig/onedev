//! OneDev SSR Web UI Binary

use onedev_web::{create_web_router, WebConfig, WebState};
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
    let state = WebState::new(config).await?;

    // Create router
    let app = create_web_router(state);

    let addr = format!("{}:{}", state.config.bind_address, state.config.port);
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    tracing::info!("OneDev Web UI listening on {}", addr);

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    Ok(())
}

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