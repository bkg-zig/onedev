//! OneDev Main Server Binary

use onedev_server::{config::AppConfig, create_app_state, init_logging, run_server};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Load configuration first
    let config = AppConfig::load()?;

    // Initialize logging
    init_logging(&config)?;

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
