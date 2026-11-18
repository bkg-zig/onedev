//! OpenTelemetry and Observability Setup

use anyhow::Result;
use opentelemetry::{global, KeyValue};
use opentelemetry_otlp::WithExportConfig;
use opentelemetry_sdk::{
    trace::{self, RandomIdGenerator, Sampler},
    Resource,
};
use std::sync::OnceLock;
use tracing_opentelemetry::OpenTelemetryLayer;
use tracing_subscriber::{layer::SubscriberExt, registry::LookupSpan, Layer};

static OTEL_INITIALIZED: OnceLock<()> = OnceLock::new();

/// Initialize OpenTelemetry
pub fn init_otel(config: &crate::config::AppConfig) -> Result<()> {
    OTEL_INITIALIZED.get_or_try_init(|| {
        if !config.observability.tracing_enabled {
            return Ok(());
        }

        let otlp_endpoint = config.server.otlp_endpoint.clone()
            .unwrap_or_else(|| "http://localhost:4317".to_string());

        // Create OTLP trace exporter
        let tracer = opentelemetry_otlp::new_pipeline()
            .tracing()
            .with_exporter(
                opentelemetry_otlp::new_exporter()
                    .tonic()
                    .with_endpoint(otlp_endpoint),
            )
            .with_trace_config(
                trace::config()
                    .with_sampler(Sampler::TraceIdRatioBased(
                        config.observability.trace_sample_rate,
                    ))
                    .with_id_generator(RandomIdGenerator::default())
                    .with_resource(Resource::new(vec![
                        KeyValue::new("service.name", "onedev"),
                        KeyValue::new("service.version", env!("CARGO_PKG_VERSION")),
                        KeyValue::new("deployment.environment", config.server.environment.clone()),
                    ])),
            )
            .install_batch(opentelemetry_sdk::runtime::Tokio)?;

        // Create OpenTelemetry layer for tracing
        let otel_layer = OpenTelemetryLayer::new(tracer);

        // Add to global subscriber
        let subscriber = tracing_subscriber::registry()
            .with(otel_layer);

        // Note: In practice, you'd set this as the global subscriber
        // tracing::subscriber::set_global_default(subscriber)?;

        tracing::info!("OpenTelemetry initialized with endpoint: {}", config.server.otlp_endpoint.as_deref().unwrap_or("default"));
        Ok(())
    })?;

    Ok(())
}

/// Initialize metrics
pub fn init_metrics(config: &crate::config::AppConfig) -> Result<()> {
    if !config.observability.metrics_enabled {
        return Ok(());
    }

    let otlp_endpoint = config.server.otlp_endpoint.clone()
        .unwrap_or_else(|| "http://localhost:4317".to_string());

    // Create OTLP metrics exporter
    let meter_provider = opentelemetry_otlp::new_pipeline()
        .metrics()
        .with_exporter(
            opentelemetry_otlp::new_exporter()
                .tonic()
                .with_endpoint(otlp_endpoint),
        )
        .with_resource(Resource::new(vec![
            KeyValue::new("service.name", "onedev"),
            KeyValue::new("service.version", env!("CARGO_PKG_VERSION")),
            KeyValue::new("deployment.environment", config.server.environment.clone()),
        ]))
        .install_batch(opentelemetry_sdk::runtime::Tokio)?;

    global::set_meter_provider(meter_provider);

    tracing::info!("OpenTelemetry metrics initialized");
    Ok(())
}

/// Initialize logging with structured format
pub fn init_logging(config: &crate::config::AppConfig) -> Result<()> {
    use tracing_subscriber::{fmt, EnvFilter};

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

/// Create a new span with common attributes
pub fn create_span(name: &str) -> tracing::Span {
    use tracing::{info_span, Span};
    info_span!(name)
}

/// Add baggage to current span
pub fn add_baggage(key: &str, value: &str) {
    use opentelemetry::trace::Tracer;
    if let Some(span) = tracing::Span::current().context().span().span_context().span_id().to_u64() {
        // Baggage is typically added via context propagation
        tracing::Span::current().record(key, value);
    }
}

/// Record an error in the current span
pub fn record_error(err: &dyn std::error::Error) {
    use tracing::Span;
    let span = Span::current();
    span.record("error", true);
    span.record("error.message", err.to_string());
    span.record("error.type", std::any::type_name_of_val(err));
}

/// Shutdown OpenTelemetry
pub fn shutdown_otel() {
    global::shutdown_tracer_provider();
    global::shutdown_meter_provider();
}