//! Logging Setup (OpenTelemetry temporarily disabled)

use anyhow::Result;
use tracing_subscriber::{fmt, EnvFilter};

/// Initialize logging with structured format
pub fn init_logging(config: &crate::config::AppConfig) -> Result<()> {
    let env_filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| config.observability.log_level.parse().unwrap());

    let fmt_layer = fmt::layer()
        .with_target(true)
        .with_thread_ids(true)
        .with_thread_names(true)
        .with_file(true)
        .with_line_number(true);

    let subscriber = match config.observability.log_format.as_str() {
        "json" => fmt_layer.json().boxed(),
        _ => fmt_layer.pretty().boxed(),
    };

    tracing::subscriber::set_global_default(
        tracing_subscriber::registry()
            .with(env_filter)
            .with(subscriber)
    )?;

    Ok(())
}

/// Initialize OpenTelemetry (stub - temporarily disabled)
pub fn init_otel(_config: &crate::config::AppConfig) -> Result<()> {
    tracing::info!("OpenTelemetry initialization skipped (temporarily disabled)");
    Ok(())
}

/// Initialize metrics (stub - temporarily disabled)
pub fn init_metrics(_config: &crate::config::AppConfig) -> Result<()> {
    tracing::info!("OpenTelemetry metrics initialization skipped (temporarily disabled)");
    Ok(())
}

/// Create a new span with common attributes
pub fn create_span(name: &str) -> tracing::Span {
    use tracing::info_span;
    info_span!("", name = %name)
}

/// Add baggage to current span
pub fn add_baggage(_key: &str, _value: &str) {
    // Baggage is typically added via context propagation
}

/// Record an error in the current span
pub fn record_error(err: &dyn std::error::Error) {
    use tracing::Span;
    let span = Span::current();
    span.record("error", true);
    span.record("error.message", err.to_string());
    span.record("error.type", std::any::type_name_of_val(err));
}

/// Shutdown OpenTelemetry (stub - temporarily disabled)
pub fn shutdown_otel() {
    // No-op when OpenTelemetry is disabled
}