//! Logging, error reporting and distributed tracing.
//!
//! * structured logging (`json` or `text`) with an `EnvFilter`,
//! * Sentry error reporting (guard lives for the whole process),
//! * a native OTLP/HTTP trace exporter: a `tracing` layer turns spans into
//!   OTLP spans and a background task ships them to
//!   `<otlp_endpoint>/v1/traces` using the JSON encoding of the protocol.
//!   No `opentelemetry` crates are involved.

use std::fmt;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use anyhow::Context as _;
use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use serde_json::{Value, json};
use tokio::sync::{mpsc, oneshot};
use tracing::field::{Field, Visit};
use tracing::span::{Attributes, Id, Record};
use tracing::{Event, Level, Subscriber};
use tracing_subscriber::layer::{Context, SubscriberExt};
use tracing_subscriber::registry::LookupSpan;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{EnvFilter, Layer, Registry};

use crate::config::AppConfig;

const EXPORT_QUEUE: usize = 8192;
const MAX_BATCH: usize = 256;
const FLUSH_INTERVAL: Duration = Duration::from_secs(5);
const EXPORT_TIMEOUT: Duration = Duration::from_secs(10);

/// Keeps telemetry sinks alive; call [`TelemetryGuard::shutdown`] before exit
/// so buffered spans and Sentry events are flushed.
pub struct TelemetryGuard {
    sentry: Option<sentry::ClientInitGuard>,
    otlp: Option<OtlpHandle>,
}

impl TelemetryGuard {
    /// Flush and stop all exporters.
    pub async fn shutdown(self) {
        if let Some(otlp) = self.otlp {
            otlp.shutdown().await;
        }
        if let Some(guard) = self.sentry {
            guard.flush(Some(Duration::from_secs(2)));
        }
    }

    pub fn sentry_enabled(&self) -> bool {
        self.sentry.is_some()
    }

    pub fn otlp_enabled(&self) -> bool {
        self.otlp.is_some()
    }
}

/// Initialise logging, Sentry and OTLP according to `config`.
///
/// Must run inside the tokio runtime (the OTLP exporter spawns a task).
pub fn init(config: &AppConfig) -> anyhow::Result<TelemetryGuard> {
    let sentry = init_sentry(config)?;

    let mut layers: Vec<Box<dyn Layer<Registry> + Send + Sync + 'static>> = Vec::new();
    if config.observability.log_format.eq_ignore_ascii_case("text") {
        layers.push(
            tracing_subscriber::fmt::layer()
                .with_target(true)
                .boxed(),
        );
    } else {
        layers.push(
            tracing_subscriber::fmt::layer()
                .json()
                .with_current_span(false)
                .with_span_list(false)
                .with_target(true)
                .boxed(),
        );
    }
    if sentry.is_some() {
        layers.push(sentry_tracing::layer().boxed());
    }

    let otlp = if config.server.otel_enabled {
        let (handle, layer) = OtlpHandle::start(OtlpSettings::from_config(config)?)?;
        layers.push(layer.boxed());
        Some(handle)
    } else {
        None
    };

    let filter = match EnvFilter::try_from_default_env() {
        Ok(filter) => filter,
        Err(_) => EnvFilter::try_new(&config.observability.log_level)
            .context("observability.log_level is not a valid filter")?,
    };

    tracing_subscriber::registry()
        .with(layers)
        .with(filter)
        .try_init()
        .context("a global tracing subscriber is already installed")?;

    Ok(TelemetryGuard { sentry, otlp })
}

fn init_sentry(config: &AppConfig) -> anyhow::Result<Option<sentry::ClientInitGuard>> {
    let Some(dsn) = config.server.sentry_dsn.as_deref().filter(|d| !d.is_empty()) else {
        return Ok(None);
    };
    let dsn: sentry::types::Dsn = dsn.parse().context("server.sentry_dsn is not a valid DSN")?;
    let guard = sentry::init(sentry::ClientOptions {
        dsn: Some(dsn),
        environment: Some(config.server.environment.clone().into()),
        release: sentry::release_name!(),
        traces_sample_rate: config.server.sentry_traces_sample_rate,
        send_default_pii: false,
        attach_stacktrace: true,
        ..Default::default()
    });
    Ok(Some(guard))
}

// ---------------------------------------------------------------------------
// OTLP/HTTP JSON exporter
// ---------------------------------------------------------------------------

/// Settings of the OTLP exporter.
#[derive(Clone)]
pub struct OtlpSettings {
    pub traces_url: String,
    pub headers: HeaderMap,
    pub service_name: String,
    pub environment: String,
    pub sample_rate: f64,
}

impl OtlpSettings {
    pub fn from_config(config: &AppConfig) -> anyhow::Result<Self> {
        let endpoint = config
            .server
            .otlp_endpoint
            .as_deref()
            .context("server.otlp_endpoint is required when server.otel_enabled is set")?;
        let mut headers = HeaderMap::new();
        for (name, value) in &config.server.otlp_headers {
            headers.insert(
                HeaderName::from_bytes(name.as_bytes())
                    .with_context(|| format!("invalid OTLP header name `{name}`"))?,
                HeaderValue::from_str(value)
                    .with_context(|| format!("invalid value for OTLP header `{name}`"))?,
            );
        }
        Ok(Self {
            traces_url: format!("{}/v1/traces", endpoint.trim_end_matches('/')),
            headers,
            service_name: config.server.service_name.clone(),
            environment: config.server.environment.clone(),
            sample_rate: config.server.otel_sample_rate,
        })
    }
}

enum ExportMsg {
    Span(Value),
    Flush(oneshot::Sender<()>),
}

/// Handle to the background exporter task.
pub struct OtlpHandle {
    tx: mpsc::Sender<ExportMsg>,
    task: tokio::task::JoinHandle<()>,
}

impl OtlpHandle {
    /// Spawn the exporter and create the layer feeding it.
    pub fn start(settings: OtlpSettings) -> anyhow::Result<(Self, OtlpLayer)> {
        let client = reqwest::Client::builder()
            .timeout(EXPORT_TIMEOUT)
            .connect_timeout(Duration::from_secs(3))
            .user_agent(concat!("onedev-server/", env!("CARGO_PKG_VERSION")))
            .build()
            .context("failed to build OTLP HTTP client")?;
        let (tx, rx) = mpsc::channel(EXPORT_QUEUE);
        let layer = OtlpLayer::new(tx.clone(), settings.sample_rate);
        let task = tokio::spawn(export_loop(rx, client, settings));
        Ok((Self { tx, task }, layer))
    }

    /// Deliver everything queued so far, then stop the exporter.
    pub async fn shutdown(self) {
        let (done_tx, done_rx) = oneshot::channel();
        if self.tx.send(ExportMsg::Flush(done_tx)).await.is_ok() {
            let _ = tokio::time::timeout(Duration::from_secs(8), done_rx).await;
        }
        drop(self.tx);
        let _ = tokio::time::timeout(Duration::from_secs(2), self.task).await;
    }
}

async fn export_loop(mut rx: mpsc::Receiver<ExportMsg>, client: reqwest::Client, settings: OtlpSettings) {
    let resource = json!({
        "attributes": [
            attr_json("service.name", &AttrValue::Str(settings.service_name.clone())),
            attr_json("service.version", &AttrValue::Str(env!("CARGO_PKG_VERSION").to_string())),
            attr_json("service.instance.id", &AttrValue::Str(uuid::Uuid::new_v4().to_string())),
            attr_json("deployment.environment", &AttrValue::Str(settings.environment.clone())),
            attr_json("telemetry.sdk.name", &AttrValue::Str("onedev-native-otlp".to_string())),
            attr_json("telemetry.sdk.language", &AttrValue::Str("rust".to_string())),
        ]
    });
    let mut batch: Vec<Value> = Vec::with_capacity(MAX_BATCH);
    let mut ticker = tokio::time::interval(FLUSH_INTERVAL);
    ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);

    loop {
        tokio::select! {
            msg = rx.recv() => match msg {
                Some(ExportMsg::Span(span)) => {
                    batch.push(span);
                    if batch.len() >= MAX_BATCH {
                        ship(&client, &settings, &resource, &mut batch).await;
                    }
                }
                Some(ExportMsg::Flush(done)) => {
                    ship(&client, &settings, &resource, &mut batch).await;
                    let _ = done.send(());
                }
                None => {
                    ship(&client, &settings, &resource, &mut batch).await;
                    break;
                }
            },
            _ = ticker.tick() => {
                if !batch.is_empty() {
                    ship(&client, &settings, &resource, &mut batch).await;
                }
            }
        }
    }
}

async fn ship(client: &reqwest::Client, settings: &OtlpSettings, resource: &Value, batch: &mut Vec<Value>) {
    if batch.is_empty() {
        return;
    }
    let spans = std::mem::take(batch);
    let count = spans.len();
    let payload = json!({
        "resourceSpans": [{
            "resource": resource,
            "scopeSpans": [{
                "scope": {"name": "onedev-server", "version": env!("CARGO_PKG_VERSION")},
                "spans": spans,
            }]
        }]
    });
    let result = client
        .post(&settings.traces_url)
        .headers(settings.headers.clone())
        .json(&payload)
        .send()
        .await
        .and_then(|response| response.error_for_status());
    match result {
        Ok(_) => metrics::counter!("otel_spans_exported_total").increment(count as u64),
        Err(error) => {
            metrics::counter!("otel_export_failures_total").increment(1);
            // `warn!` outside of any span: no feedback loop through the layer.
            tracing::warn!(%error, spans = count, "OTLP export failed, spans dropped");
        }
    }
}

// ---------------------------------------------------------------------------
// Span capture
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
enum AttrValue {
    Str(String),
    Int(i64),
    Bool(bool),
    Double(f64),
}

fn attr_json(key: &str, value: &AttrValue) -> Value {
    let value = match value {
        AttrValue::Str(s) => json!({ "stringValue": s }),
        AttrValue::Int(i) => json!({ "intValue": i.to_string() }),
        AttrValue::Bool(b) => json!({ "boolValue": b }),
        AttrValue::Double(d) => json!({ "doubleValue": d }),
    };
    json!({ "key": key, "value": value })
}

#[derive(Default)]
struct AttrVisitor {
    attrs: Vec<(String, AttrValue)>,
    message: Option<String>,
}

impl AttrVisitor {
    fn push(&mut self, name: &str, value: AttrValue) {
        if let Some(slot) = self.attrs.iter_mut().find(|(k, _)| k == name) {
            slot.1 = value;
        } else {
            self.attrs.push((name.to_string(), value));
        }
    }
}

impl Visit for AttrVisitor {
    fn record_str(&mut self, field: &Field, value: &str) {
        if field.name() == "message" {
            self.message = Some(value.to_string());
        }
        self.push(field.name(), AttrValue::Str(value.to_string()));
    }

    fn record_i64(&mut self, field: &Field, value: i64) {
        self.push(field.name(), AttrValue::Int(value));
    }

    fn record_u64(&mut self, field: &Field, value: u64) {
        match i64::try_from(value) {
            Ok(v) => self.push(field.name(), AttrValue::Int(v)),
            Err(_) => self.push(field.name(), AttrValue::Str(value.to_string())),
        }
    }

    fn record_bool(&mut self, field: &Field, value: bool) {
        self.push(field.name(), AttrValue::Bool(value));
    }

    fn record_f64(&mut self, field: &Field, value: f64) {
        self.push(field.name(), AttrValue::Double(value));
    }

    fn record_debug(&mut self, field: &Field, value: &dyn fmt::Debug) {
        let rendered = format!("{value:?}");
        if field.name() == "message" {
            self.message = Some(rendered.clone());
        }
        self.push(field.name(), AttrValue::Str(rendered));
    }
}

struct OtelData {
    trace_id: [u8; 16],
    span_id: [u8; 8],
    parent_span_id: Option<[u8; 8]>,
    sampled: bool,
    start_unix_nano: u64,
    started: Instant,
    name: String,
    attrs: Vec<(String, AttrValue)>,
    error: Option<String>,
}

fn random_nonzero<const N: usize>() -> [u8; N] {
    loop {
        let id: [u8; N] = rand::random();
        if id.iter().any(|b| *b != 0) {
            return id;
        }
    }
}

fn unix_nanos(time: SystemTime) -> u64 {
    time.duration_since(UNIX_EPOCH)
        .map(|d| u64::try_from(d.as_nanos()).unwrap_or(u64::MAX))
        .unwrap_or(0)
}

/// `tracing` layer converting spans (INFO and above) into OTLP spans.
pub struct OtlpLayer {
    tx: mpsc::Sender<ExportMsg>,
    sample_rate: f64,
}

impl OtlpLayer {
    fn new(tx: mpsc::Sender<ExportMsg>, sample_rate: f64) -> Self {
        Self { tx, sample_rate }
    }
}

impl<S> Layer<S> for OtlpLayer
where
    S: Subscriber + for<'a> LookupSpan<'a>,
{
    fn on_new_span(&self, attrs: &Attributes<'_>, id: &Id, ctx: Context<'_, S>) {
        // DEBUG / TRACE spans are too chatty for a trace backend.
        if *attrs.metadata().level() > Level::INFO {
            return;
        }
        let Some(span) = ctx.span(id) else { return };

        let parent = if let Some(parent_id) = attrs.parent() {
            ctx.span(parent_id)
        } else if attrs.is_contextual() {
            ctx.lookup_current()
        } else {
            None
        };
        let inherited = parent.as_ref().and_then(|p| {
            p.scope().find_map(|ancestor| {
                let ext = ancestor.extensions();
                let found = ext
                    .get::<OtelData>()
                    .map(|d| (d.trace_id, d.span_id, d.sampled));
                found
            })
        });

        let (trace_id, parent_span_id, sampled) = match inherited {
            Some((trace_id, parent_span, sampled)) => (trace_id, Some(parent_span), sampled),
            None => (random_nonzero::<16>(), None, rand::random::<f64>() < self.sample_rate),
        };

        let mut attributes = Vec::new();
        if sampled {
            let mut visitor = AttrVisitor::default();
            attrs.record(&mut visitor);
            attributes = visitor.attrs;
        }
        span.extensions_mut().insert(OtelData {
            trace_id,
            span_id: random_nonzero::<8>(),
            parent_span_id,
            sampled,
            start_unix_nano: unix_nanos(SystemTime::now()),
            started: Instant::now(),
            name: attrs.metadata().name().to_string(),
            attrs: attributes,
            error: None,
        });
    }

    fn on_record(&self, id: &Id, values: &Record<'_>, ctx: Context<'_, S>) {
        let Some(span) = ctx.span(id) else { return };
        let mut ext = span.extensions_mut();
        if let Some(data) = ext.get_mut::<OtelData>() {
            if data.sampled {
                let mut visitor = AttrVisitor::default();
                values.record(&mut visitor);
                for (key, value) in visitor.attrs {
                    if let Some(slot) = data.attrs.iter_mut().find(|(k, _)| *k == key) {
                        slot.1 = value;
                    } else {
                        data.attrs.push((key, value));
                    }
                }
            }
        }
    }

    fn on_event(&self, event: &Event<'_>, ctx: Context<'_, S>) {
        if *event.metadata().level() != Level::ERROR {
            return;
        }
        let Some(current) = ctx.event_span(event) else { return };
        let Some(owner) = current
            .scope()
            .find(|s| s.extensions().get::<OtelData>().is_some())
        else {
            return;
        };
        let mut visitor = AttrVisitor::default();
        event.record(&mut visitor);
        let mut ext = owner.extensions_mut();
        if let Some(data) = ext.get_mut::<OtelData>() {
            if data.sampled && data.error.is_none() {
                data.error = Some(visitor.message.unwrap_or_else(|| "error".to_string()));
            }
        }
    }

    fn on_close(&self, id: Id, ctx: Context<'_, S>) {
        let Some(span) = ctx.span(&id) else { return };
        let Some(data) = span.extensions_mut().remove::<OtelData>() else {
            return;
        };
        if !data.sampled {
            return;
        }
        let exported = finish_span(data);
        if self.tx.try_send(ExportMsg::Span(exported)).is_err() {
            metrics::counter!("otel_spans_dropped_total").increment(1);
        }
    }
}

fn finish_span(mut data: OtelData) -> Value {
    let elapsed = u64::try_from(data.started.elapsed().as_nanos()).unwrap_or(u64::MAX);
    let end = data.start_unix_nano.saturating_add(elapsed);

    // OTel conventions carried through span fields.
    let mut kind = 1; // INTERNAL
    let mut name = data.name.clone();
    data.attrs.retain(|(key, value)| match (key.as_str(), value) {
        ("otel.kind", AttrValue::Str(k)) => {
            kind = match k.to_ascii_lowercase().as_str() {
                "server" => 2,
                "client" => 3,
                "producer" => 4,
                "consumer" => 5,
                _ => 1,
            };
            false
        }
        ("otel.name", AttrValue::Str(n)) => {
            name = n.clone();
            false
        }
        _ => true,
    });

    let http_failed = data.attrs.iter().any(|(key, value)| {
        key == "http.status_code" && matches!(value, AttrValue::Int(code) if *code >= 500)
    });
    let status = match (&data.error, http_failed) {
        (Some(message), _) => Some(json!({ "code": 2, "message": message })),
        (None, true) => Some(json!({ "code": 2, "message": "server error" })),
        _ => None,
    };

    let mut span = json!({
        "traceId": hex::encode(data.trace_id),
        "spanId": hex::encode(data.span_id),
        "name": name,
        "kind": kind,
        "startTimeUnixNano": data.start_unix_nano.to_string(),
        "endTimeUnixNano": end.to_string(),
        "attributes": data.attrs.iter().map(|(k, v)| attr_json(k, v)).collect::<Vec<_>>(),
    });
    if let Some(parent) = data.parent_span_id {
        span["parentSpanId"] = Value::String(hex::encode(parent));
    }
    if let Some(status) = status {
        span["status"] = status;
    }
    span
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    use axum::extract::State;
    use axum::http::HeaderMap as AxumHeaders;
    use axum::routing::post;
    use axum::{Json, Router};

    type Received = Arc<Mutex<Vec<(AxumHeaders, Value)>>>;

    fn collect_spans(rx: &mut mpsc::Receiver<ExportMsg>) -> Vec<Value> {
        let mut out = Vec::new();
        while let Ok(msg) = rx.try_recv() {
            if let ExportMsg::Span(span) = msg {
                out.push(span);
            }
        }
        out
    }

    #[test]
    fn span_tree_is_exported_with_parent_links_and_attributes() {
        let (tx, mut rx) = mpsc::channel(64);
        let subscriber = tracing_subscriber::registry().with(OtlpLayer::new(tx, 1.0));
        tracing::subscriber::with_default(subscriber, || {
            let root = tracing::info_span!("root", otel.kind = "server", http.method = "GET");
            let _root = root.enter();
            let child = tracing::info_span!("child", rows = 3_i64, ok = true, ratio = 0.5_f64);
            let _child = child.enter();
            tracing::error!("boom");
            // DEBUG spans are skipped and must not break parent linking.
            let skipped = tracing::debug_span!("skipped");
            let _skipped = skipped.enter();
            let grandchild = tracing::info_span!("grandchild");
            drop(grandchild.enter());
        });
        let spans = collect_spans(&mut rx);
        assert_eq!(spans.len(), 3, "{spans:#?}");

        let by_name = |name: &str| spans.iter().find(|s| s["name"] == name).unwrap().clone();
        let root = by_name("root");
        let child = by_name("child");
        let grandchild = by_name("grandchild");

        assert_eq!(root["kind"], 2);
        assert!(root.get("parentSpanId").is_none());
        assert_eq!(child["traceId"], root["traceId"]);
        assert_eq!(child["parentSpanId"], root["spanId"]);
        assert_eq!(grandchild["traceId"], root["traceId"]);
        assert_eq!(grandchild["parentSpanId"], child["spanId"]);
        assert_eq!(child["status"]["code"], 2);
        assert_eq!(child["status"]["message"], "boom");
        assert!(root.get("status").is_none());

        let attrs = child["attributes"].as_array().unwrap();
        let find = |key: &str| attrs.iter().find(|a| a["key"] == key).unwrap()["value"].clone();
        assert_eq!(find("rows")["intValue"], "3");
        assert_eq!(find("ok")["boolValue"], true);
        assert_eq!(find("ratio")["doubleValue"], 0.5);

        assert_eq!(root["traceId"].as_str().unwrap().len(), 32);
        assert_eq!(root["spanId"].as_str().unwrap().len(), 16);
        let start: u64 = root["startTimeUnixNano"].as_str().unwrap().parse().unwrap();
        let end: u64 = root["endTimeUnixNano"].as_str().unwrap().parse().unwrap();
        assert!(end >= start && start > 0);
    }

    #[test]
    fn unsampled_traces_export_nothing_including_children() {
        let (tx, mut rx) = mpsc::channel(64);
        let subscriber = tracing_subscriber::registry().with(OtlpLayer::new(tx, 0.0));
        tracing::subscriber::with_default(subscriber, || {
            let root = tracing::info_span!("root");
            let _root = root.enter();
            let child = tracing::info_span!("child");
            drop(child.enter());
        });
        assert!(collect_spans(&mut rx).is_empty());
    }

    #[test]
    fn recorded_fields_and_http_5xx_mark_the_span_failed() {
        let (tx, mut rx) = mpsc::channel(64);
        let subscriber = tracing_subscriber::registry().with(OtlpLayer::new(tx, 1.0));
        tracing::subscriber::with_default(subscriber, || {
            let span = tracing::info_span!(
                "http",
                otel.name = "GET /x",
                http.status_code = tracing::field::Empty
            );
            span.record("http.status_code", 503_i64);
        });
        let spans = collect_spans(&mut rx);
        assert_eq!(spans.len(), 1);
        assert_eq!(spans[0]["name"], "GET /x");
        assert_eq!(spans[0]["status"]["code"], 2);
    }

    #[tokio::test]
    async fn exporter_posts_otlp_json_with_auth_header() {
        let received: Received = Arc::default();
        let app = Router::new()
            .route(
                "/v1/traces",
                post(
                    |State(store): State<Received>, headers: AxumHeaders, Json(body): Json<Value>| async move {
                        store.lock().unwrap().push((headers, body));
                        axum::http::StatusCode::OK
                    },
                ),
            )
            .with_state(received.clone());
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });

        let mut headers = HeaderMap::new();
        headers.insert("authorization", HeaderValue::from_static("Bearer test-token"));
        let settings = OtlpSettings {
            traces_url: format!("http://{addr}/v1/traces"),
            headers,
            service_name: "onedev-test".into(),
            environment: "test".into(),
            sample_rate: 1.0,
        };
        let (handle, layer) = OtlpHandle::start(settings).unwrap();
        let subscriber = tracing_subscriber::registry().with(layer);
        tracing::subscriber::with_default(subscriber, || {
            let span = tracing::info_span!("work", job = "build");
            drop(span.enter());
        });
        handle.shutdown().await;

        let received = received.lock().unwrap();
        assert_eq!(received.len(), 1, "one batch expected");
        let (headers, body) = &received[0];
        assert_eq!(headers.get("authorization").unwrap(), "Bearer test-token");
        let resource = &body["resourceSpans"][0]["resource"]["attributes"];
        assert!(resource.to_string().contains("onedev-test"));
        let spans = &body["resourceSpans"][0]["scopeSpans"][0]["spans"];
        assert_eq!(spans.as_array().unwrap().len(), 1);
        assert_eq!(spans[0]["name"], "work");
    }

    #[test]
    fn settings_require_endpoint_and_valid_headers() {
        let mut config = AppConfig::default();
        config.server.otel_enabled = true;
        assert!(OtlpSettings::from_config(&config).is_err());
        config.server.otlp_endpoint = Some("http://collector:4318/".into());
        config
            .server
            .otlp_headers
            .insert("authorization".into(), "Bearer x".into());
        let settings = OtlpSettings::from_config(&config).unwrap();
        assert_eq!(settings.traces_url, "http://collector:4318/v1/traces");
        config.server.otlp_headers.insert("bad header".into(), "x".into());
        assert!(OtlpSettings::from_config(&config).is_err());
    }
}
