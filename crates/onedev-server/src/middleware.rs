//! HTTP middleware stack.
//!
//! Layers, outermost first:
//!
//! 1. request id (sanitised, generated when absent, echoed in the response),
//! 2. sensitive header marking (never shown in debug output),
//! 3. observe: tracing span, access log, Prometheus metrics,
//! 4. security headers (also on rejected requests),
//! 5. CORS (only when origins are configured),
//! 6. compression (not for packfiles / archives),
//! 7. per-client rate limit (trusted-proxy aware),
//! 8. load shedding (bounded number of in-flight requests),
//! 9. request timeout (git / LFS / artifact transfers exempt),
//! 10. body size limit (larger for transfers than for JSON APIs),
//! 11. panic catcher (handler panics become JSON 500s).
//!
//! The stack is applied with `Router::layer`, so it runs after routing and
//! can label metrics with the matched route template.

use std::net::{IpAddr, SocketAddr};
use std::num::NonZeroU32;
use std::sync::Arc;
use std::time::{Duration, Instant};

use axum::Router;
use axum::body::Body;
use axum::extract::{ConnectInfo, DefaultBodyLimit, MatchedPath, Request, State};
use axum::http::{HeaderMap, HeaderName, HeaderValue, Method, StatusCode, header};
use axum::middleware::{Next, from_fn, from_fn_with_state};
use axum::response::{IntoResponse, Response};
use governor::clock::Clock;
use governor::{DefaultKeyedRateLimiter, Quota, RateLimiter};
use ipnet::IpNet;
use tokio::sync::Semaphore;
use tower_http::catch_panic::CatchPanicLayer;
use tower_http::compression::predicate::{DefaultPredicate, NotForContentType, Predicate};
use tower_http::compression::CompressionLayer;
use tower_http::cors::{AllowOrigin, Any, CorsLayer};
use tower_http::request_id::{MakeRequestUuid, PropagateRequestIdLayer, SetRequestIdLayer};
use tower_http::sensitive_headers::{
    SetSensitiveRequestHeadersLayer, SetSensitiveResponseHeadersLayer,
};
use tracing::Instrument;

use crate::config::{AppConfig, SecurityConfig};

/// Paths that must stay reachable under load (orchestrator probes, scraping).
const PROBE_PATHS: &[&str] = &["/health", "/ready", "/metrics", "/api/health", "/api/ready"];

/// Per-client keyed rate limiter.
pub type KeyedLimiter = DefaultKeyedRateLimiter<IpAddr>;

/// Handles the caller needs to keep alive / service (limiter garbage collection).
pub struct StackHandles {
    pub limiter: Option<Arc<KeyedLimiter>>,
}

#[derive(Clone)]
struct StackState {
    limiter: Option<Arc<KeyedLimiter>>,
    trusted: Arc<Vec<IpNet>>,
    semaphore: Option<Arc<Semaphore>>,
    timeout: Duration,
    max_api_body: usize,
    max_transfer_body: usize,
    security: Arc<SecurityHeaders>,
}

/// Wrap `router` with the production middleware stack.
pub fn apply(router: Router, config: &AppConfig) -> anyhow::Result<(Router, StackHandles)> {
    let trusted = Arc::new(config.trusted_proxy_nets()?);
    let rate = &config.security.rate_limit;
    let limiter = if rate.enabled {
        let per_minute = NonZeroU32::new(rate.requests_per_minute)
            .ok_or_else(|| anyhow::anyhow!("rate limit requests_per_minute must be positive"))?;
        let burst = NonZeroU32::new(rate.burst)
            .ok_or_else(|| anyhow::anyhow!("rate limit burst must be positive"))?;
        Some(Arc::new(RateLimiter::keyed(
            Quota::per_minute(per_minute).allow_burst(burst),
        )))
    } else {
        None
    };
    let state = StackState {
        limiter: limiter.clone(),
        trusted,
        semaphore: (config.server.max_concurrent_requests > 0)
            .then(|| Arc::new(Semaphore::new(config.server.max_concurrent_requests))),
        timeout: config.request_timeout(),
        max_api_body: mb_to_bytes(config.server.max_api_body_mb),
        max_transfer_body: config.max_body_bytes(),
        security: Arc::new(SecurityHeaders::from_config(&config.security)?),
    };

    // `Router::layer` wraps the previous stack, so the list below runs
    // innermost -> outermost.
    let mut router = router
        .layer(CatchPanicLayer::custom(panic_response))
        .layer(DefaultBodyLimit::disable())
        .layer(from_fn_with_state(state.clone(), body_limit))
        .layer(from_fn_with_state(state.clone(), request_timeout))
        .layer(from_fn_with_state(state.clone(), load_shed))
        .layer(from_fn_with_state(state.clone(), rate_limit))
        .layer(CompressionLayer::new().compress_when(compression_predicate()));
    if let Some(cors) = cors_layer(&config.security)? {
        router = router.layer(cors);
    }
    let router = router
        .layer(from_fn_with_state(state.security.clone(), security_headers))
        .layer(from_fn(observe))
        .layer(SetSensitiveResponseHeadersLayer::new([header::SET_COOKIE]))
        .layer(SetSensitiveRequestHeadersLayer::new([
            header::AUTHORIZATION,
            header::COOKIE,
            header::PROXY_AUTHORIZATION,
            HeaderName::from_static("x-api-key"),
        ]))
        .layer(PropagateRequestIdLayer::x_request_id())
        .layer(SetRequestIdLayer::x_request_id(MakeRequestUuid))
        .layer(from_fn(sanitize_request_id));

    Ok((router, StackHandles { limiter }))
}

/// Drop expired limiter entries so the key map cannot grow without bound.
pub async fn run_limiter_gc(limiter: Arc<KeyedLimiter>, mut shutdown: crate::lifecycle::ShutdownSignal) {
    let mut ticker = tokio::time::interval(Duration::from_secs(60));
    ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    loop {
        tokio::select! {
            _ = ticker.tick() => {
                limiter.retain_recent();
                limiter.shrink_to_fit();
            }
            _ = shutdown.wait() => break,
        }
    }
}

fn mb_to_bytes(mb: u64) -> usize {
    usize::try_from(mb.saturating_mul(1024 * 1024)).unwrap_or(usize::MAX)
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn json_error(status: StatusCode, code: &str, message: &str) -> Response {
    (
        status,
        [(header::CONTENT_TYPE, "application/json")],
        serde_json::json!({ "error": code, "message": message }).to_string(),
    )
        .into_response()
}

fn is_probe(path: &str) -> bool {
    PROBE_PATHS.contains(&path)
}

/// Git smart-HTTP, LFS and artifact transfers: large bodies, long durations.
fn is_transfer(path: &str) -> bool {
    path.ends_with("/git-upload-pack")
        || path.ends_with("/git-receive-pack")
        || path.ends_with("/info/refs")
        || path.contains("/info/lfs/")
        || path.contains("/lfs/objects")
        || path.contains("/artifacts")
        || path.contains("/archive")
}

fn canonical(ip: IpAddr) -> IpAddr {
    ip.to_canonical()
}

fn is_trusted(ip: IpAddr, trusted: &[IpNet]) -> bool {
    let ip = canonical(ip);
    trusted.iter().any(|net| net.contains(&ip))
}

/// Resolve the client address. Forwarding headers are only honoured when the
/// direct peer is a trusted proxy; the right-most untrusted hop wins so a
/// client cannot spoof its address by sending its own `X-Forwarded-For`.
pub fn client_ip(peer: IpAddr, headers: &HeaderMap, trusted: &[IpNet]) -> IpAddr {
    let peer = canonical(peer);
    if !is_trusted(peer, trusted) {
        return peer;
    }
    let chain: Vec<IpAddr> = headers
        .get_all("x-forwarded-for")
        .iter()
        .filter_map(|value| value.to_str().ok())
        .flat_map(|value| value.split(','))
        .filter_map(|part| part.trim().parse::<IpAddr>().ok())
        .map(canonical)
        .collect();
    if let Some(ip) = chain.iter().rev().find(|ip| !is_trusted(**ip, trusted)) {
        return *ip;
    }
    if let Some(first) = chain.first() {
        return *first;
    }
    headers
        .get("x-real-ip")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.trim().parse::<IpAddr>().ok())
        .map(canonical)
        .unwrap_or(peer)
}

/// IPv6 clients get a whole /64 so rotating addresses does not evade the limit.
fn rate_key(ip: IpAddr) -> IpAddr {
    match canonical(ip) {
        IpAddr::V6(v6) => {
            let mut octets = v6.octets();
            octets[8..].fill(0);
            IpAddr::V6(octets.into())
        }
        v4 => v4,
    }
}

fn peer_addr(request: &Request) -> IpAddr {
    request
        .extensions()
        .get::<ConnectInfo<SocketAddr>>()
        .map(|info| info.0.ip())
        .unwrap_or(IpAddr::from([127, 0, 0, 1]))
}

// ---------------------------------------------------------------------------
// Request id
// ---------------------------------------------------------------------------

/// Inbound request ids end up in logs: accept only short, harmless values.
async fn sanitize_request_id(mut request: Request, next: Next) -> Response {
    let valid = request
        .headers()
        .get("x-request-id")
        .map(|value| {
            let bytes = value.as_bytes();
            !bytes.is_empty()
                && bytes.len() <= 128
                && bytes
                    .iter()
                    .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b':'))
        })
        .unwrap_or(true);
    if !valid {
        request.headers_mut().remove("x-request-id");
    }
    next.run(request).await
}

// ---------------------------------------------------------------------------
// Observability
// ---------------------------------------------------------------------------

struct InFlight;

impl InFlight {
    fn enter() -> Self {
        metrics::gauge!("http_requests_in_flight").increment(1.0);
        Self
    }
}

impl Drop for InFlight {
    fn drop(&mut self) {
        metrics::gauge!("http_requests_in_flight").decrement(1.0);
    }
}

fn method_label(method: &Method) -> &'static str {
    match *method {
        Method::GET => "GET",
        Method::POST => "POST",
        Method::PUT => "PUT",
        Method::PATCH => "PATCH",
        Method::DELETE => "DELETE",
        Method::HEAD => "HEAD",
        Method::OPTIONS => "OPTIONS",
        _ => "OTHER",
    }
}

/// Tracing span + access log + metrics for every request.
async fn observe(request: Request, next: Next) -> Response {
    let started = Instant::now();
    let method = method_label(request.method());
    let path = request.uri().path().to_owned();
    let route = request
        .extensions()
        .get::<MatchedPath>()
        .map(|matched| matched.as_str().to_owned())
        .unwrap_or_else(|| "unmatched".to_owned());
    let request_id = request
        .headers()
        .get("x-request-id")
        .and_then(|value| value.to_str().ok())
        .unwrap_or("-")
        .to_owned();
    let probe = is_probe(&path);

    let span = tracing::info_span!(
        "http",
        otel.kind = "server",
        otel.name = %format!("{method} {route}"),
        http.method = method,
        http.route = %route,
        url.path = %path,
        request_id = %request_id,
        http.status_code = tracing::field::Empty,
    );

    let _in_flight = InFlight::enter();
    let response = next.run(request).instrument(span.clone()).await;

    let status = response.status();
    let elapsed = started.elapsed();
    span.record("http.status_code", i64::from(status.as_u16()));
    metrics::counter!(
        "http_requests_total",
        "method" => method,
        "path" => route.clone(),
        "status" => status.as_u16().to_string()
    )
    .increment(1);
    metrics::histogram!(
        "http_request_duration_seconds",
        "method" => method,
        "path" => route.clone()
    )
    .record(elapsed.as_secs_f64());

    let latency_ms = u64::try_from(elapsed.as_millis()).unwrap_or(u64::MAX);
    let _entered = span.enter();
    if status.is_server_error() {
        tracing::error!(status = status.as_u16(), latency_ms, "request failed");
    } else if probe && status.is_success() {
        tracing::debug!(status = status.as_u16(), latency_ms, "request completed");
    } else if status.is_client_error() {
        tracing::warn!(status = status.as_u16(), latency_ms, "request rejected");
    } else {
        tracing::info!(status = status.as_u16(), latency_ms, "request completed");
    }
    response
}

fn panic_response(panic: Box<dyn std::any::Any + Send + 'static>) -> Response {
    let detail = panic
        .downcast_ref::<&str>()
        .map(|s| (*s).to_owned())
        .or_else(|| panic.downcast_ref::<String>().cloned())
        .unwrap_or_else(|| "non-string panic payload".to_owned());
    metrics::counter!("http_panics_total").increment(1);
    tracing::error!(panic = %detail, "handler panicked");
    json_error(
        StatusCode::INTERNAL_SERVER_ERROR,
        "internal_error",
        "internal server error",
    )
}

// ---------------------------------------------------------------------------
// Security headers
// ---------------------------------------------------------------------------

const API_CSP: &str = "default-src 'none'; frame-ancestors 'none'";
const UI_CSP: &str =
    "default-src 'self'; object-src 'none'; base-uri 'self'; frame-ancestors 'none'; form-action 'self'";
const DEFAULT_PERMISSIONS: &str = "geolocation=(), microphone=(), camera=(), payment=()";

struct SecurityHeaders {
    csp: Option<HeaderValue>,
    hsts: Option<HeaderValue>,
    referrer: HeaderValue,
    permissions: HeaderValue,
}

impl SecurityHeaders {
    fn from_config(config: &SecurityConfig) -> anyhow::Result<Self> {
        let value = |text: &str, what: &str| {
            HeaderValue::from_str(text)
                .map_err(|_| anyhow::anyhow!("security.{what} is not a valid header value"))
        };
        Ok(Self {
            csp: config.csp.as_deref().map(|c| value(c, "csp")).transpose()?,
            hsts: config
                .hsts_enabled
                .then(|| value(&format!("max-age={}; includeSubDomains", config.hsts_max_age), "hsts_max_age"))
                .transpose()?,
            referrer: value(&config.referrer_policy, "referrer_policy")?,
            permissions: value(
                config
                    .permissions_policy
                    .as_deref()
                    .unwrap_or(DEFAULT_PERMISSIONS),
                "permissions_policy",
            )?,
        })
    }
}

async fn security_headers(
    State(security): State<Arc<SecurityHeaders>>,
    request: Request,
    next: Next,
) -> Response {
    let api = request.uri().path().starts_with("/api/");
    let mut response = next.run(request).await;
    let headers = response.headers_mut();

    let set = |headers: &mut HeaderMap, name: HeaderName, value: HeaderValue| {
        headers.entry(name).or_insert(value);
    };
    set(headers, header::X_CONTENT_TYPE_OPTIONS, HeaderValue::from_static("nosniff"));
    set(headers, header::X_FRAME_OPTIONS, HeaderValue::from_static("DENY"));
    set(headers, header::REFERRER_POLICY, security.referrer.clone());
    set(headers, HeaderName::from_static("permissions-policy"), security.permissions.clone());
    set(
        headers,
        HeaderName::from_static("cross-origin-opener-policy"),
        HeaderValue::from_static("same-origin"),
    );
    let csp = security
        .csp
        .clone()
        .unwrap_or_else(|| HeaderValue::from_static(if api { API_CSP } else { UI_CSP }));
    set(headers, header::CONTENT_SECURITY_POLICY, csp);
    if let Some(hsts) = &security.hsts {
        set(headers, header::STRICT_TRANSPORT_SECURITY, hsts.clone());
    }
    if api {
        // API responses carry per-user data: never cache them in shared caches.
        set(headers, header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    }
    response
}

// ---------------------------------------------------------------------------
// CORS / compression
// ---------------------------------------------------------------------------

fn cors_layer(config: &SecurityConfig) -> anyhow::Result<Option<CorsLayer>> {
    if config.cors_origins.is_empty() {
        return Ok(None);
    }
    let layer = CorsLayer::new()
        .allow_methods([
            Method::GET,
            Method::POST,
            Method::PUT,
            Method::PATCH,
            Method::DELETE,
            Method::HEAD,
            Method::OPTIONS,
        ])
        .allow_headers([
            header::AUTHORIZATION,
            header::CONTENT_TYPE,
            header::ACCEPT,
            header::IF_NONE_MATCH,
            HeaderName::from_static("x-request-id"),
            HeaderName::from_static("x-requested-with"),
        ])
        .expose_headers([
            HeaderName::from_static("x-request-id"),
            header::RETRY_AFTER,
            header::ETAG,
            header::LINK,
            header::WWW_AUTHENTICATE,
        ])
        .max_age(Duration::from_secs(600));
    let layer = if config.cors_origins.iter().any(|o| o == "*") {
        // Credentials cannot be combined with a wildcard origin.
        layer.allow_origin(Any)
    } else {
        let origins = config
            .cors_origins
            .iter()
            .map(|origin| {
                HeaderValue::from_str(origin)
                    .map_err(|_| anyhow::anyhow!("invalid CORS origin `{origin}`"))
            })
            .collect::<Result<Vec<_>, _>>()?;
        layer
            .allow_origin(AllowOrigin::list(origins))
            .allow_credentials(true)
    };
    Ok(Some(layer))
}

fn compression_predicate() -> impl Predicate {
    DefaultPredicate::new()
        .and(NotForContentType::const_new("application/x-git-"))
        .and(NotForContentType::const_new("application/octet-stream"))
        .and(NotForContentType::const_new("application/zip"))
        .and(NotForContentType::const_new("application/gzip"))
        .and(NotForContentType::const_new("application/x-tar"))
}

// ---------------------------------------------------------------------------
// Admission control
// ---------------------------------------------------------------------------

async fn rate_limit(State(state): State<StackState>, request: Request, next: Next) -> Response {
    let Some(limiter) = &state.limiter else {
        return next.run(request).await;
    };
    if is_probe(request.uri().path()) || request.method() == Method::OPTIONS {
        return next.run(request).await;
    }
    let ip = client_ip(peer_addr(&request), request.headers(), &state.trusted);
    match limiter.check_key(&rate_key(ip)) {
        Ok(()) => next.run(request).await,
        Err(not_until) => {
            let wait = not_until.wait_time_from(limiter.clock().now());
            let retry_after = wait.as_secs_f64().ceil().max(1.0) as u64;
            metrics::counter!("http_requests_rejected_total", "reason" => "rate_limited").increment(1);
            tracing::warn!(client = %ip, retry_after, "rate limit exceeded");
            let mut response = json_error(
                StatusCode::TOO_MANY_REQUESTS,
                "rate_limited",
                "too many requests, slow down",
            );
            if let Ok(value) = HeaderValue::from_str(&retry_after.to_string()) {
                response.headers_mut().insert(header::RETRY_AFTER, value);
            }
            response
        }
    }
}

/// Fail fast with 503 instead of queueing unbounded work.
async fn load_shed(State(state): State<StackState>, request: Request, next: Next) -> Response {
    let Some(semaphore) = &state.semaphore else {
        return next.run(request).await;
    };
    if is_probe(request.uri().path()) {
        return next.run(request).await;
    }
    match semaphore.clone().try_acquire_owned() {
        Ok(_permit) => next.run(request).await,
        Err(_) => {
            metrics::counter!("http_requests_rejected_total", "reason" => "overloaded").increment(1);
            let mut response = json_error(
                StatusCode::SERVICE_UNAVAILABLE,
                "overloaded",
                "server is at capacity, retry shortly",
            );
            response
                .headers_mut()
                .insert(header::RETRY_AFTER, HeaderValue::from_static("1"));
            response
        }
    }
}

async fn request_timeout(State(state): State<StackState>, request: Request, next: Next) -> Response {
    if is_transfer(request.uri().path()) {
        return next.run(request).await;
    }
    match tokio::time::timeout(state.timeout, next.run(request)).await {
        Ok(response) => response,
        Err(_) => {
            metrics::counter!("http_requests_rejected_total", "reason" => "timeout").increment(1);
            json_error(
                StatusCode::REQUEST_TIMEOUT,
                "timeout",
                "the request took too long to process",
            )
        }
    }
}

async fn body_limit(State(state): State<StackState>, request: Request, next: Next) -> Response {
    let limit = if is_transfer(request.uri().path()) {
        state.max_transfer_body
    } else {
        state.max_api_body
    };
    let declared = request
        .headers()
        .get(header::CONTENT_LENGTH)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse::<u64>().ok());
    if declared.is_some_and(|len| len > limit as u64) {
        metrics::counter!("http_requests_rejected_total", "reason" => "body_too_large").increment(1);
        return json_error(
            StatusCode::PAYLOAD_TOO_LARGE,
            "payload_too_large",
            "the request body exceeds the allowed size",
        );
    }
    let (parts, body) = request.into_parts();
    let body = Body::new(http_body_util::Limited::new(body, limit));
    next.run(Request::from_parts(parts, body)).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::to_bytes;
    use axum::routing::{get, post};
    use tower::ServiceExt;

    fn test_config() -> AppConfig {
        let mut config = AppConfig::default();
        config.security.rate_limit.enabled = false;
        config
    }

    fn app(config: &AppConfig) -> Router {
        let router = Router::new()
            .route("/ok", get(|| async { "ok" }))
            .route("/health", get(|| async { "healthy" }))
            .route("/api/data", get(|| async { axum::Json(serde_json::json!({"a": 1})) }))
            .route(
                "/boom",
                get(|| async {
                    let reply: &'static str = panic!("handler exploded");
                    reply
                }),
            )
            .route(
                "/slow",
                get(|| async {
                    tokio::time::sleep(Duration::from_millis(400)).await;
                    "late"
                }),
            )
            .route(
                "/g/p.git/git-receive-pack",
                get(|| async {
                    tokio::time::sleep(Duration::from_millis(400)).await;
                    "pushed"
                }),
            )
            .route("/echo", post(|body: axum::body::Bytes| async move { body.len().to_string() }))
            .route(
                "/g/p.git/git-upload-pack",
                post(|body: axum::body::Bytes| async move { body.len().to_string() }),
            )
            .route(
                "/big",
                get(|| async {
                    (
                        [(header::CONTENT_TYPE, "application/json")],
                        "x".repeat(8192),
                    )
                }),
            )
            .route(
                "/pack",
                get(|| async {
                    (
                        [(header::CONTENT_TYPE, "application/x-git-upload-pack-result")],
                        "x".repeat(8192),
                    )
                }),
            );
        apply(router, config).expect("stack builds").0
    }

    fn request(uri: &str) -> axum::http::request::Builder {
        Request::builder().uri(uri)
    }

    fn with_peer(mut req: Request, peer: &str) -> Request {
        let addr: SocketAddr = format!("{peer}:4000").parse().unwrap();
        req.extensions_mut().insert(ConnectInfo(addr));
        req
    }

    #[test]
    fn client_ip_ignores_forwarding_headers_from_untrusted_peers() {
        let trusted = vec!["10.0.0.0/8".parse::<IpNet>().unwrap()];
        let mut headers = HeaderMap::new();
        headers.insert("x-forwarded-for", "1.2.3.4".parse().unwrap());
        let peer: IpAddr = "203.0.113.9".parse().unwrap();
        assert_eq!(client_ip(peer, &headers, &trusted), peer);
    }

    #[test]
    fn client_ip_takes_the_rightmost_untrusted_hop() {
        let trusted = vec!["10.0.0.0/8".parse::<IpNet>().unwrap()];
        let mut headers = HeaderMap::new();
        // client spoofed 9.9.9.9; the real chain is 198.51.100.7 -> 10.0.0.5 (proxy)
        headers.insert("x-forwarded-for", "9.9.9.9, 198.51.100.7, 10.0.0.5".parse().unwrap());
        let peer: IpAddr = "10.0.0.1".parse().unwrap();
        assert_eq!(
            client_ip(peer, &headers, &trusted),
            "198.51.100.7".parse::<IpAddr>().unwrap()
        );
        headers.clear();
        headers.insert("x-real-ip", "198.51.100.8".parse().unwrap());
        assert_eq!(
            client_ip(peer, &headers, &trusted),
            "198.51.100.8".parse::<IpAddr>().unwrap()
        );
        headers.clear();
        assert_eq!(client_ip(peer, &headers, &trusted), peer);
    }

    #[test]
    fn ipv6_clients_share_a_64_bit_bucket_and_mapped_v4_is_canonical() {
        let a: IpAddr = "2001:db8::1".parse().unwrap();
        let b: IpAddr = "2001:db8::ffff".parse().unwrap();
        let other: IpAddr = "2001:db8:0:1::1".parse().unwrap();
        assert_eq!(rate_key(a), rate_key(b));
        assert_ne!(rate_key(a), rate_key(other));
        let mapped: IpAddr = "::ffff:192.0.2.1".parse().unwrap();
        assert_eq!(rate_key(mapped), "192.0.2.1".parse::<IpAddr>().unwrap());
    }

    #[tokio::test]
    async fn adds_request_id_and_security_headers() {
        let app = app(&test_config());
        let response = app.clone().oneshot(request("/ok").body(Body::empty()).unwrap()).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let headers = response.headers();
        assert!(headers.get("x-request-id").is_some());
        assert_eq!(headers[header::X_CONTENT_TYPE_OPTIONS], "nosniff");
        assert_eq!(headers[header::X_FRAME_OPTIONS], "DENY");
        assert_eq!(headers[header::CONTENT_SECURITY_POLICY], UI_CSP);
        assert!(headers.get(header::STRICT_TRANSPORT_SECURITY).is_some());
        assert!(headers.get(header::CACHE_CONTROL).is_none());

        let api = app.oneshot(request("/api/data").body(Body::empty()).unwrap()).await.unwrap();
        assert_eq!(api.headers()[header::CONTENT_SECURITY_POLICY], API_CSP);
        assert_eq!(api.headers()[header::CACHE_CONTROL], "no-store");
    }

    #[tokio::test]
    async fn valid_inbound_request_id_is_kept_and_hostile_ones_are_replaced() {
        let app = app(&test_config());
        let kept = app
            .clone()
            .oneshot(request("/ok").header("x-request-id", "trace-123").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(kept.headers()["x-request-id"], "trace-123");

        let hostile = "a".repeat(300);
        let replaced = app
            .oneshot(request("/ok").header("x-request-id", hostile.as_str()).body(Body::empty()).unwrap())
            .await
            .unwrap();
        let id = replaced.headers()["x-request-id"].to_str().unwrap().to_owned();
        assert_ne!(id, hostile);
        assert_eq!(id.len(), 36, "fresh UUID expected, got {id}");
    }

    #[tokio::test]
    async fn rate_limit_rejects_after_burst_with_retry_after_and_exempts_probes() {
        let mut config = test_config();
        config.security.rate_limit = crate::config::RateLimitConfig {
            enabled: true,
            requests_per_minute: 60,
            burst: 3,
        };
        let app = app(&config);
        for _ in 0..3 {
            let ok = app
                .clone()
                .oneshot(with_peer(request("/ok").body(Body::empty()).unwrap(), "192.0.2.10"))
                .await
                .unwrap();
            assert_eq!(ok.status(), StatusCode::OK);
        }
        let limited = app
            .clone()
            .oneshot(with_peer(request("/ok").body(Body::empty()).unwrap(), "192.0.2.10"))
            .await
            .unwrap();
        assert_eq!(limited.status(), StatusCode::TOO_MANY_REQUESTS);
        let retry: u64 = limited.headers()[header::RETRY_AFTER].to_str().unwrap().parse().unwrap();
        assert!(retry >= 1);
        // security headers also decorate rejected requests
        assert_eq!(limited.headers()[header::X_CONTENT_TYPE_OPTIONS], "nosniff");
        let body = to_bytes(limited.into_body(), 4096).await.unwrap();
        assert!(std::str::from_utf8(&body).unwrap().contains("rate_limited"));

        // another client is unaffected, probes are exempt
        let other = app
            .clone()
            .oneshot(with_peer(request("/ok").body(Body::empty()).unwrap(), "192.0.2.11"))
            .await
            .unwrap();
        assert_eq!(other.status(), StatusCode::OK);
        let probe = app
            .oneshot(with_peer(request("/health").body(Body::empty()).unwrap(), "192.0.2.10"))
            .await
            .unwrap();
        assert_eq!(probe.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn spoofed_forwarded_for_cannot_dodge_the_limit_from_untrusted_peers() {
        let mut config = test_config();
        config.security.rate_limit = crate::config::RateLimitConfig {
            enabled: true,
            requests_per_minute: 60,
            burst: 1,
        };
        let app = app(&config);
        let send = |xff: &'static str| {
            let app = app.clone();
            async move {
                app.oneshot(with_peer(
                    request("/ok").header("x-forwarded-for", xff).body(Body::empty()).unwrap(),
                    "192.0.2.50",
                ))
                .await
                .unwrap()
                .status()
            }
        };
        assert_eq!(send("1.1.1.1").await, StatusCode::OK);
        assert_eq!(send("2.2.2.2").await, StatusCode::TOO_MANY_REQUESTS);
    }

    #[tokio::test]
    async fn handler_panics_become_json_500() {
        let app = app(&test_config());
        let response = app.oneshot(request("/boom").body(Body::empty()).unwrap()).await.unwrap();
        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
        let body = to_bytes(response.into_body(), 4096).await.unwrap();
        let text = std::str::from_utf8(&body).unwrap();
        assert!(text.contains("internal_error"));
        assert!(!text.contains("exploded"), "panic details must not leak to clients");
    }

    #[tokio::test]
    async fn slow_requests_time_out_but_git_transfers_do_not() {
        let config = test_config();
        // Build the timeout layer manually with a short timeout.
        let router = Router::new()
            .route(
                "/slow",
                get(|| async {
                    tokio::time::sleep(Duration::from_millis(400)).await;
                    "late"
                }),
            )
            .route(
                "/g/p.git/git-receive-pack",
                get(|| async {
                    tokio::time::sleep(Duration::from_millis(400)).await;
                    "pushed"
                }),
            );
        let state = StackState {
            limiter: None,
            trusted: Arc::new(Vec::new()),
            semaphore: None,
            timeout: Duration::from_millis(150),
            max_api_body: 1024,
            max_transfer_body: 1024,
            security: Arc::new(SecurityHeaders::from_config(&config.security).unwrap()),
        };
        let router = router.layer(from_fn_with_state(state, request_timeout));

        let slow = router.clone().oneshot(request("/slow").body(Body::empty()).unwrap()).await.unwrap();
        assert_eq!(slow.status(), StatusCode::REQUEST_TIMEOUT);
        let push = router
            .oneshot(request("/g/p.git/git-receive-pack").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(push.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn body_limit_differs_between_api_and_transfers() {
        let mut config = test_config();
        config.server.max_api_body_mb = 1;
        config.server.max_body_mb = 4;
        config.git.max_pack_size_mb = 4;
        config.storage.max_file_size_mb = 4;
        let app = app(&config);

        // 2 MiB to a JSON endpoint: rejected up front via Content-Length
        let too_big = vec![b'a'; 2 * 1024 * 1024];
        let rejected = app
            .clone()
            .oneshot(
                request("/echo")
                    .method(Method::POST)
                    .header(header::CONTENT_LENGTH, too_big.len())
                    .body(Body::from(too_big.clone()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(rejected.status(), StatusCode::PAYLOAD_TOO_LARGE);

        // chunked body without Content-Length is cut off while streaming
        let stream = futures::stream::iter(
            too_big
                .chunks(64 * 1024)
                .map(|chunk| Ok::<_, std::io::Error>(bytes::Bytes::copy_from_slice(chunk)))
                .collect::<Vec<_>>(),
        );
        let streamed = app
            .clone()
            .oneshot(
                request("/echo")
                    .method(Method::POST)
                    .body(Body::from_stream(stream))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(streamed.status(), StatusCode::PAYLOAD_TOO_LARGE);

        // the same payload is fine for a git transfer
        let accepted = app
            .oneshot(
                request("/g/p.git/git-upload-pack")
                    .method(Method::POST)
                    .body(Body::from(too_big))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(accepted.status(), StatusCode::OK);
        let body = to_bytes(accepted.into_body(), 64).await.unwrap();
        assert_eq!(&body[..], b"2097152");
    }

    #[tokio::test]
    async fn overload_is_shed_with_503() {
        let mut config = test_config();
        config.server.max_concurrent_requests = 1;
        let app = app(&config);
        let slow = {
            let app = app.clone();
            tokio::spawn(async move {
                app.oneshot(request("/slow").body(Body::empty()).unwrap()).await.unwrap().status()
            })
        };
        tokio::time::sleep(Duration::from_millis(100)).await;
        let shed = app
            .clone()
            .oneshot(request("/ok").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(shed.status(), StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(shed.headers()[header::RETRY_AFTER], "1");
        // probes bypass load shedding
        let probe = app.oneshot(request("/health").body(Body::empty()).unwrap()).await.unwrap();
        assert_eq!(probe.status(), StatusCode::OK);
        assert_eq!(slow.await.unwrap(), StatusCode::OK);
    }

    #[tokio::test]
    async fn cors_only_answers_configured_origins() {
        let mut config = test_config();
        config.security.cors_origins = vec!["https://git.example.com".into()];
        let app = app(&config);
        let preflight = |origin: &'static str| {
            request("/api/data")
                .method(Method::OPTIONS)
                .header(header::ORIGIN, origin)
                .header(header::ACCESS_CONTROL_REQUEST_METHOD, "GET")
                .body(Body::empty())
                .unwrap()
        };
        let allowed = app.clone().oneshot(preflight("https://git.example.com")).await.unwrap();
        assert_eq!(
            allowed.headers()[header::ACCESS_CONTROL_ALLOW_ORIGIN],
            "https://git.example.com"
        );
        assert_eq!(allowed.headers()[header::ACCESS_CONTROL_ALLOW_CREDENTIALS], "true");
        let denied = app.oneshot(preflight("https://evil.example.org")).await.unwrap();
        assert!(denied.headers().get(header::ACCESS_CONTROL_ALLOW_ORIGIN).is_none());
    }

    #[tokio::test]
    async fn compresses_json_but_not_git_packs() {
        let app = app(&test_config());
        let json = app
            .clone()
            .oneshot(request("/big").header(header::ACCEPT_ENCODING, "gzip").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(json.headers()[header::CONTENT_ENCODING], "gzip");
        let pack = app
            .oneshot(request("/pack").header(header::ACCEPT_ENCODING, "gzip").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert!(pack.headers().get(header::CONTENT_ENCODING).is_none());
    }

    #[test]
    fn invalid_security_config_is_reported() {
        let mut config = AppConfig::default();
        config.security.csp = Some("bad\nvalue".into());
        assert!(apply(Router::new(), &config).is_err());
        let mut config = AppConfig::default();
        config.security.cors_origins = vec!["bad\norigin".into()];
        assert!(apply(Router::new(), &config).is_err());
    }
}
