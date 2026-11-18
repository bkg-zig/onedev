//! OneDev Main Server Binary

use onedev_server::{config::AppConfig, create_app_state, run_server};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize tracing
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "onedev_server=info,axum=info,tower_http=info".into()),
        )
        .with(tracing_subscriber::fmt::layer().json())
        .init();

    // Load configuration
    let config = AppConfig::load()?;

    // Initialize Sentry if DSN is configured
    if let Some(dsn) = &config.server.sentry_dsn {
        let _guard = sentry::init(sentry::ClientOptions {
            dsn: Some(dsn.parse()?),
            environment: Some(config.server.environment.clone().into()),
            release: sentry::release_name!(),
            traces_sample_rate: 0.1,
            ..Default::default()
        });
        tracing::info!("Sentry initialized");
    }

    // Initialize telemetry
    if config.observability.tracing_enabled || config.observability.metrics_enabled {
        onedev_server::init_telemetry(&config)?;
    }

    // Create application state
    let app_state = create_app_state(&config).await?;

    // Run the server
    run_server(app_state, config).await?;

    Ok(())
}
