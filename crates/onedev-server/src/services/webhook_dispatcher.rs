//! Webhook delivery.
//!
//! Events are written to the `webhook_deliveries` outbox by
//! [`enqueue_event`] (usually in the same request that caused the event) and
//! shipped by the [`WebhookDispatcher`]:
//!
//! * deliveries are claimed with `FOR UPDATE SKIP LOCKED`, so several server
//!   instances can run dispatchers on one database,
//! * every request carries `X-OneDev-Event`, `X-OneDev-Delivery` and - when
//!   the webhook has a secret - `X-OneDev-Signature` (lower-case hex
//!   HMAC-SHA256 of the raw body, key = the UTF-8 secret),
//! * transient failures (network errors, 408/425/429/5xx) are retried with
//!   exponential backoff and jitter, permanent ones (other 4xx, blocked
//!   targets) are abandoned immediately,
//! * targets are checked against an SSRF policy (private / loopback /
//!   link-local ranges are refused unless explicitly allowed) both on the
//!   URL and on every DNS answer, and redirects are never followed.

use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;
use std::time::{Duration, Instant};

use chrono::{DateTime, Utc};
use futures::StreamExt;
use hmac::{Hmac, Mac};
use onedev_db::DbPool;
use reqwest::dns::{Addrs, Name, Resolve, Resolving};
use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use serde_json::Value;
use sha2::Sha256;
use sqlx::Row;
use tokio::sync::Notify;
use tracing::{debug, error, info, warn};

use crate::config::WebhookConfig;
use crate::lifecycle::ShutdownSignal;
use crate::services::{Backoff, sleep_or_shutdown, spawn_notify_listener};

type HmacSha256 = Hmac<Sha256>;

const CONCURRENT_DELIVERIES: usize = 8;
pub const SIGNATURE_HEADER: &str = "x-onedev-signature";
const RESERVED_HEADERS: &[&str] = &[
    "host",
    "content-length",
    "content-type",
    "transfer-encoding",
    "connection",
    "user-agent",
    "te",
    "upgrade",
    "proxy-authorization",
];

// ---------------------------------------------------------------------------
// Event enqueueing
// ---------------------------------------------------------------------------

/// An event to deliver to matching webhooks.
#[derive(Debug, Clone)]
pub struct WebhookEvent {
    /// Event name, e.g. `push`, `build.finished`, `issue.opened`.
    pub name: String,
    /// Project the event belongs to (global webhooks always match).
    pub project_id: Option<i64>,
    /// Full ref (`refs/heads/main`, `refs/tags/v1`) for ref-related events;
    /// used for the webhook's branch / tag filters.
    pub ref_name: Option<String>,
    pub payload: Value,
}

/// Does a webhook with `scopes` want `event`? Empty scopes mean "everything";
/// `build` also matches `build.finished`.
pub fn scope_matches(scopes: &[String], event: &str) -> bool {
    if scopes.is_empty() {
        return true;
    }
    let family = event.split('.').next().unwrap_or(event);
    scopes
        .iter()
        .any(|scope| scope == "*" || scope == event || scope == family)
}

/// Branch / tag filters of a webhook against the event's ref.
pub fn ref_filter_matches(branch_filter: Option<&str>, tag_filter: Option<&str>, ref_name: Option<&str>) -> bool {
    let Some(ref_name) = ref_name else { return true };
    let matches_patterns = |patterns: Option<&str>, value: &str| match patterns.map(str::trim) {
        None | Some("") => true,
        Some(patterns) => patterns
            .split_whitespace()
            .any(|pattern| onedev_build::glob_matches(pattern, value)),
    };
    if let Some(branch) = ref_name.strip_prefix("refs/heads/") {
        matches_patterns(branch_filter, branch)
    } else if let Some(tag) = ref_name.strip_prefix("refs/tags/") {
        matches_patterns(tag_filter, tag)
    } else {
        true
    }
}

/// Queue `event` for every active webhook that wants it. Returns the number
/// of deliveries created.
pub async fn enqueue_event(pool: &DbPool, event: &WebhookEvent) -> Result<u64, sqlx::Error> {
    let candidates = sqlx::query(
        "SELECT id, url, scopes, branch_filter, tag_filter FROM webhooks \
         WHERE active AND (project_id = $1 OR project_id IS NULL)",
    )
    .bind(event.project_id)
    .fetch_all(pool)
    .await?;

    let mut targets: Vec<(i64, String)> = Vec::new();
    for row in candidates {
        let scopes: Vec<String> = row
            .try_get::<Value, _>("scopes")?
            .as_array()
            .map(|items| items.iter().filter_map(|v| v.as_str().map(str::to_owned)).collect())
            .unwrap_or_default();
        let branch_filter: Option<String> = row.try_get("branch_filter")?;
        let tag_filter: Option<String> = row.try_get("tag_filter")?;
        if scope_matches(&scopes, &event.name)
            && ref_filter_matches(branch_filter.as_deref(), tag_filter.as_deref(), event.ref_name.as_deref())
        {
            targets.push((row.try_get("id")?, row.try_get("url")?));
        }
    }
    if targets.is_empty() {
        return Ok(0);
    }

    let mut builder = sqlx::QueryBuilder::new("INSERT INTO webhook_deliveries (webhook_id, event, payload, url) ");
    builder.push_values(&targets, |mut row, (webhook_id, url)| {
        row.push_bind(*webhook_id)
            .push_bind(&event.name)
            .push_bind(&event.payload)
            .push_bind(url);
    });
    let result = builder.build().execute(pool).await?;
    Ok(result.rows_affected())
}

// ---------------------------------------------------------------------------
// Target policy (SSRF protection)
// ---------------------------------------------------------------------------

/// Is this address one a webhook must not reach (internal networks)?
pub fn is_blocked_ip(ip: IpAddr) -> bool {
    match ip.to_canonical() {
        IpAddr::V4(v4) => {
            let o = v4.octets();
            v4.is_loopback()
                || v4.is_private()
                || v4.is_link_local()
                || v4.is_unspecified()
                || v4.is_broadcast()
                || v4.is_multicast()
                || v4.is_documentation()
                || (o[0] == 100 && (64..=127).contains(&o[1])) // CGNAT 100.64.0.0/10
                || (o[0] == 192 && o[1] == 0 && o[2] == 0) // IETF protocol assignments
                || (o[0] == 198 && (18..=19).contains(&o[1])) // benchmarking
                || o[0] >= 240 // reserved
        }
        IpAddr::V6(v6) => {
            let segments = v6.segments();
            v6.is_loopback()
                || v6.is_unspecified()
                || v6.is_multicast()
                || (segments[0] & 0xfe00) == 0xfc00 // unique local fc00::/7
                || (segments[0] & 0xffc0) == 0xfe80 // link local fe80::/10
                || (segments[0] == 0x2001 && segments[1] == 0x0db8) // documentation
        }
    }
}

fn host_matches(pattern: &str, host: &str) -> bool {
    let pattern = pattern.trim().to_ascii_lowercase();
    let host = host.to_ascii_lowercase();
    match pattern.strip_prefix("*.") {
        Some(suffix) => host == suffix || host.ends_with(&format!(".{suffix}")),
        None => host == pattern,
    }
}

/// Why a URL may not be called.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum TargetError {
    #[error("invalid URL: {0}")]
    Invalid(String),
    #[error("only http and https targets are allowed")]
    Scheme,
    #[error("host `{0}` is blocked")]
    Blocked(String),
    #[error("host `{0}` is not in the allowed host list")]
    NotAllowed(String),
    #[error("target address {0} is in a private or reserved range")]
    PrivateAddress(IpAddr),
}

/// Static part of the target policy (the dynamic part is the DNS resolver).
#[derive(Debug, Clone)]
pub struct TargetPolicy {
    pub allowed_hosts: Vec<String>,
    pub blocked_hosts: Vec<String>,
    pub allow_private: bool,
}

impl TargetPolicy {
    pub fn from_config(config: &WebhookConfig) -> Self {
        Self {
            allowed_hosts: config.allowed_hosts.clone(),
            blocked_hosts: config.blocked_hosts.clone(),
            allow_private: config.allow_private_targets,
        }
    }

    pub fn check(&self, raw: &str) -> Result<reqwest::Url, TargetError> {
        let url = reqwest::Url::parse(raw).map_err(|e| TargetError::Invalid(e.to_string()))?;
        if !matches!(url.scheme(), "http" | "https") {
            return Err(TargetError::Scheme);
        }
        let host = url
            .host_str()
            .ok_or_else(|| TargetError::Invalid("missing host".into()))?
            .trim_matches(|c| c == '[' || c == ']')
            .to_owned();
        if self.blocked_hosts.iter().any(|p| host_matches(p, &host)) {
            return Err(TargetError::Blocked(host));
        }
        if !self.allowed_hosts.is_empty() && !self.allowed_hosts.iter().any(|p| host_matches(p, &host)) {
            return Err(TargetError::NotAllowed(host));
        }
        if let Ok(ip) = host.parse::<IpAddr>() {
            if !self.allow_private && is_blocked_ip(ip) {
                return Err(TargetError::PrivateAddress(ip));
            }
        }
        Ok(url)
    }
}

/// DNS resolver that drops private / reserved answers, so a public hostname
/// cannot be pointed at internal services (DNS rebinding).
struct FilteringResolver {
    allow_private: bool,
}

impl Resolve for FilteringResolver {
    fn resolve(&self, name: Name) -> Resolving {
        let allow_private = self.allow_private;
        Box::pin(async move {
            let host = name.as_str().to_owned();
            let addrs: Vec<SocketAddr> = tokio::net::lookup_host((host.as_str(), 0))
                .await?
                .filter(|addr| allow_private || !is_blocked_ip(addr.ip()))
                .collect();
            if addrs.is_empty() {
                let error: Box<dyn std::error::Error + Send + Sync> =
                    format!("`{host}` resolves only to private or reserved addresses").into();
                return Err(error);
            }
            let addrs: Addrs = Box::new(addrs.into_iter());
            Ok(addrs)
        })
    }
}

// ---------------------------------------------------------------------------
// Retry policy
// ---------------------------------------------------------------------------

/// Result of one delivery attempt.
#[derive(Debug, Clone)]
pub enum Outcome {
    Response { status: u16, retry_after: Option<Duration> },
    NetworkError(String),
    Blocked(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Decision {
    Delivered,
    Retry(Duration),
    Abandon(String),
}

/// `base * 2^(attempt-1)`, capped at `max`. `attempt` starts at 1.
pub fn backoff_delay(base: Duration, max: Duration, attempt: u32) -> Duration {
    let factor = 1u32.checked_shl(attempt.saturating_sub(1).min(20)).unwrap_or(u32::MAX);
    base.saturating_mul(factor).min(max)
}

fn with_jitter(delay: Duration) -> Duration {
    // +/- 10 %
    let factor = 0.9 + rand::random::<f64>() * 0.2;
    Duration::from_secs_f64(delay.as_secs_f64() * factor)
}

/// Decide what to do after `attempt` (1-based) produced `outcome`.
pub fn decide(outcome: &Outcome, attempt: u32, config: &WebhookConfig) -> Decision {
    let base = Duration::from_secs(config.retry_delay_secs.max(1));
    let max = Duration::from_secs(config.max_retry_delay_secs.max(1));
    let retry = |hint: Option<Duration>, reason: String| {
        if attempt >= config.max_retries {
            Decision::Abandon(format!("{reason}; gave up after {attempt} attempts"))
        } else {
            let computed = backoff_delay(base, max, attempt);
            let delay = hint.map_or(computed, |hint| hint.max(computed)).min(max);
            Decision::Retry(delay)
        }
    };
    match outcome {
        Outcome::Response { status, .. } if (200..300).contains(status) => Decision::Delivered,
        Outcome::Response { status, retry_after } if matches!(status, 408 | 425 | 429) || *status >= 500 => {
            retry(*retry_after, format!("HTTP {status}"))
        }
        Outcome::Response { status, .. } => Decision::Abandon(format!("permanent failure: HTTP {status}")),
        Outcome::NetworkError(error) => retry(None, error.clone()),
        Outcome::Blocked(reason) => Decision::Abandon(format!("target refused: {reason}")),
    }
}

/// Lower-case hex HMAC-SHA256 of `body` keyed with `secret`.
pub fn sign(secret: &str, body: &[u8]) -> String {
    let mut mac = HmacSha256::new_from_slice(secret.as_bytes()).expect("HMAC accepts any key length");
    mac.update(body);
    hex::encode(mac.finalize().into_bytes())
}

// ---------------------------------------------------------------------------
// Dispatcher
// ---------------------------------------------------------------------------

#[derive(Debug)]
struct Claimed {
    id: i64,
    uuid: uuid::Uuid,
    webhook_id: i64,
    event: String,
    payload: Value,
    url: String,
    attempt: u32,
}

#[derive(Debug)]
struct HookSettings {
    secret: Option<String>,
    ssl_verification: bool,
    headers: Vec<(HeaderName, HeaderValue)>,
    active: bool,
}

pub struct WebhookDispatcher {
    pool: DbPool,
    config: WebhookConfig,
    policy: TargetPolicy,
    verified: reqwest::Client,
    insecure: reqwest::Client,
    wake: Arc<Notify>,
}

impl WebhookDispatcher {
    pub fn new(pool: DbPool, config: WebhookConfig) -> anyhow::Result<Self> {
        let build = |verify: bool| -> anyhow::Result<reqwest::Client> {
            Ok(reqwest::Client::builder()
                .user_agent(concat!("OneDev-Webhook/", env!("CARGO_PKG_VERSION")))
                .timeout(Duration::from_secs(config.timeout_secs.max(1)))
                .connect_timeout(Duration::from_secs(10))
                .redirect(reqwest::redirect::Policy::none())
                .no_proxy()
                .pool_idle_timeout(Duration::from_secs(30))
                .danger_accept_invalid_certs(!verify)
                .dns_resolver(Arc::new(FilteringResolver {
                    allow_private: config.allow_private_targets,
                }))
                .build()?)
        };
        Ok(Self {
            policy: TargetPolicy::from_config(&config),
            verified: build(true)?,
            insecure: build(false)?,
            pool,
            config,
            wake: Arc::new(Notify::new()),
        })
    }

    pub fn wake_handle(&self) -> Arc<Notify> {
        self.wake.clone()
    }

    /// Main loop; returns when `shutdown` fires (after finishing the batch in flight).
    pub async fn run(self: Arc<Self>, mut shutdown: ShutdownSignal) {
        let listener = spawn_notify_listener(
            self.pool.clone(),
            "onedev_webhook_queued",
            self.wake.clone(),
            shutdown.child(),
        );
        let poll = Duration::from_secs(self.config.poll_interval_secs.max(1));
        let mut error_backoff = Backoff::new(Duration::from_secs(1), Duration::from_secs(60));
        info!("webhook dispatcher started");

        while !shutdown.is_set() {
            match self.process_batch().await {
                Ok(0) => {
                    error_backoff.reset();
                    tokio::select! {
                        _ = self.wake.notified() => {}
                        _ = tokio::time::sleep(poll) => {}
                        _ = shutdown.wait() => {}
                    }
                }
                Ok(_) => error_backoff.reset(),
                Err(error) => {
                    metrics::counter!("background_task_restarts_total", "task" => "webhook_dispatcher").increment(1);
                    error!(%error, "webhook batch failed");
                    if !sleep_or_shutdown(error_backoff.next_delay(), &mut shutdown).await {
                        break;
                    }
                }
            }
        }
        listener.abort();
        info!("webhook dispatcher stopped");
    }

    /// Claim and deliver one batch; returns the number of deliveries handled.
    pub async fn process_batch(&self) -> Result<usize, sqlx::Error> {
        self.reset_stale().await?;
        let claimed = self.claim().await?;
        let count = claimed.len();
        futures::stream::iter(claimed)
            .map(|delivery| self.deliver(delivery))
            .buffer_unordered(CONCURRENT_DELIVERIES)
            .collect::<Vec<()>>()
            .await;
        if let Ok(depth) = sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM webhook_deliveries WHERE status = 'pending'",
        )
        .fetch_one(&self.pool)
        .await
        {
            metrics::gauge!("webhook_queue_depth").set(depth as f64);
        }
        Ok(count)
    }

    /// Deliveries whose dispatcher died mid-flight go back to the queue.
    async fn reset_stale(&self) -> Result<(), sqlx::Error> {
        let stale_after = (self.config.timeout_secs * 2 + 60) as f64;
        let reset = sqlx::query(
            "UPDATE webhook_deliveries SET status = 'pending', next_retry_at = NOW() \
             WHERE status = 'sending' AND last_attempt_at < NOW() - make_interval(secs => $1)",
        )
        .bind(stale_after)
        .execute(&self.pool)
        .await?;
        if reset.rows_affected() > 0 {
            warn!(count = reset.rows_affected(), "requeued webhook deliveries stuck in `sending`");
        }
        Ok(())
    }

    async fn claim(&self) -> Result<Vec<Claimed>, sqlx::Error> {
        let rows = sqlx::query(
            r#"
            UPDATE webhook_deliveries d
            SET status = 'sending', attempt = d.attempt + 1, last_attempt_at = NOW()
            WHERE d.id IN (
                SELECT id FROM webhook_deliveries
                WHERE status = 'pending' AND (next_retry_at IS NULL OR next_retry_at <= NOW())
                ORDER BY next_retry_at NULLS FIRST, id
                FOR UPDATE SKIP LOCKED
                LIMIT $1
            )
            RETURNING d.id, d.uuid, d.webhook_id, d.event, d.payload, d.url, d.attempt
            "#,
        )
        .bind(self.config.batch_size as i64)
        .fetch_all(&self.pool)
        .await?;
        rows.into_iter()
            .map(|row| {
                Ok(Claimed {
                    id: row.try_get("id")?,
                    uuid: row.try_get("uuid")?,
                    webhook_id: row.try_get("webhook_id")?,
                    event: row.try_get("event")?,
                    payload: row.try_get("payload")?,
                    url: row.try_get("url")?,
                    attempt: u32::try_from(row.try_get::<i32, _>("attempt")?).unwrap_or(1),
                })
            })
            .collect()
    }

    async fn load_settings(&self, webhook_id: i64) -> Result<Option<HookSettings>, sqlx::Error> {
        let row = sqlx::query("SELECT secret, ssl_verification, headers, active FROM webhooks WHERE id = $1")
            .bind(webhook_id)
            .fetch_optional(&self.pool)
            .await?;
        let Some(row) = row else { return Ok(None) };
        let mut headers = Vec::new();
        if let Some(map) = row.try_get::<Value, _>("headers")?.as_object() {
            for (name, value) in map {
                let lowered = name.to_ascii_lowercase();
                if RESERVED_HEADERS.contains(&lowered.as_str()) || lowered.starts_with("x-onedev-") {
                    warn!(webhook_id, header = %name, "ignoring reserved webhook header");
                    continue;
                }
                let value = value.as_str().map(str::to_owned).unwrap_or_else(|| value.to_string());
                match (HeaderName::from_bytes(name.as_bytes()), HeaderValue::from_str(&value)) {
                    (Ok(name), Ok(mut value)) => {
                        value.set_sensitive(true);
                        headers.push((name, value));
                    }
                    _ => warn!(webhook_id, header = %name, "ignoring invalid webhook header"),
                }
            }
        }
        Ok(Some(HookSettings {
            secret: row.try_get::<Option<String>, _>("secret")?.filter(|s| !s.is_empty()),
            ssl_verification: row.try_get("ssl_verification")?,
            headers,
            active: row.try_get("active")?,
        }))
    }

    /// Deliver one claimed delivery and record the result. Never fails: DB
    /// errors while recording are logged and the stale-reset recovers the row.
    async fn deliver(&self, delivery: Claimed) {
        let started = Instant::now();
        let settings = match self.load_settings(delivery.webhook_id).await {
            Ok(settings) => settings,
            Err(error) => {
                error!(%error, delivery = delivery.id, "cannot load webhook settings");
                return;
            }
        };
        let (outcome, response_body) = match &settings {
            None => (Outcome::Blocked("webhook was deleted".into()), None),
            Some(settings) if !settings.active => (Outcome::Blocked("webhook is disabled".into()), None),
            Some(settings) => self.send(&delivery, settings).await,
        };
        metrics::histogram!("webhook_delivery_duration_seconds").record(started.elapsed().as_secs_f64());

        let decision = decide(&outcome, delivery.attempt, &self.config);
        let (status_code, error_text) = match &outcome {
            Outcome::Response { status, .. } => (Some(i32::from(*status)), None),
            Outcome::NetworkError(error) => (None, Some(error.clone())),
            Outcome::Blocked(reason) => (None, Some(reason.clone())),
        };
        let label = match &decision {
            Decision::Delivered => "delivered",
            Decision::Retry(_) => "retry",
            Decision::Abandon(_) => "abandoned",
        };
        metrics::counter!("webhook_deliveries_total", "outcome" => label).increment(1);

        let recorded = match decision {
            Decision::Delivered => self.record(&delivery, "delivered", None, status_code, response_body, None).await,
            Decision::Retry(delay) => {
                let next = Utc::now() + chrono::Duration::from_std(with_jitter(delay)).unwrap_or_default();
                debug!(delivery = delivery.id, attempt = delivery.attempt, ?delay, "webhook delivery will be retried");
                self.record(&delivery, "pending", Some(next), status_code, response_body, error_text).await
            }
            Decision::Abandon(reason) => {
                warn!(delivery = delivery.id, %reason, url = %delivery.url, "webhook delivery abandoned");
                self.record(&delivery, "abandoned", None, status_code, response_body, Some(reason)).await
            }
        };
        if let Err(error) = recorded {
            error!(%error, delivery = delivery.id, "cannot record webhook delivery result");
        }
        if let Err(error) = sqlx::query(
            "UPDATE webhooks SET last_delivery_at = NOW(), last_delivery_status = $2 WHERE id = $1",
        )
        .bind(delivery.webhook_id)
        .bind(status_code.unwrap_or(0))
        .execute(&self.pool)
        .await
        {
            debug!(%error, "cannot update webhook last delivery");
        }
    }

    async fn record(
        &self,
        delivery: &Claimed,
        status: &str,
        next_retry_at: Option<DateTime<Utc>>,
        response_status: Option<i32>,
        response_body: Option<String>,
        error: Option<String>,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            "UPDATE webhook_deliveries SET status = $2::webhook_delivery_status, next_retry_at = $3, \
             response_status = $4, response_body = $5, error = $6, \
             delivered_at = CASE WHEN $2 = 'delivered' THEN NOW() ELSE delivered_at END \
             WHERE id = $1",
        )
        .bind(delivery.id)
        .bind(status)
        .bind(next_retry_at)
        .bind(response_status)
        .bind(response_body)
        .bind(error)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn send(&self, delivery: &Claimed, settings: &HookSettings) -> (Outcome, Option<String>) {
        let url = match self.policy.check(&delivery.url) {
            Ok(url) => url,
            Err(error) => return (Outcome::Blocked(error.to_string()), None),
        };
        let body = match serde_json::to_vec(&delivery.payload) {
            Ok(body) => body,
            Err(error) => return (Outcome::Blocked(format!("payload is not serializable: {error}")), None),
        };

        let mut headers = HeaderMap::new();
        headers.insert("content-type", HeaderValue::from_static("application/json"));
        headers.insert(
            "x-onedev-delivery",
            HeaderValue::from_str(&delivery.uuid.to_string()).expect("uuid is a valid header value"),
        );
        if let Ok(value) = HeaderValue::from_str(&delivery.event) {
            headers.insert("x-onedev-event", value);
        }
        if let Some(secret) = &settings.secret {
            if let Ok(mut value) = HeaderValue::from_str(&sign(secret, &body)) {
                value.set_sensitive(true);
                headers.insert(SIGNATURE_HEADER, value);
            }
        }
        for (name, value) in &settings.headers {
            headers.insert(name.clone(), value.clone());
        }

        let client = if settings.ssl_verification && self.config.verify_ssl {
            &self.verified
        } else {
            &self.insecure
        };
        let response = match client.post(url).headers(headers).body(body).send().await {
            Ok(response) => response,
            Err(error) => {
                return (Outcome::NetworkError(describe_request_error(&error)), None);
            }
        };
        let status = response.status().as_u16();
        let retry_after = response
            .headers()
            .get("retry-after")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.trim().parse::<u64>().ok())
            .map(Duration::from_secs);
        let body = read_limited(response, self.config.max_response_bytes).await;
        (Outcome::Response { status, retry_after }, Some(body))
    }
}

fn describe_request_error(error: &reqwest::Error) -> String {
    use std::error::Error as _;
    let mut text = error.to_string();
    let mut source = error.source();
    while let Some(inner) = source {
        text.push_str(": ");
        text.push_str(&inner.to_string());
        source = inner.source();
    }
    text
}

/// Read at most `limit` bytes of the response body as (lossy) text.
async fn read_limited(mut response: reqwest::Response, limit: usize) -> String {
    let mut collected: Vec<u8> = Vec::new();
    while let Ok(Some(chunk)) = response.chunk().await {
        let room = limit.saturating_sub(collected.len());
        collected.extend_from_slice(&chunk[..chunk.len().min(room)]);
        if collected.len() >= limit {
            break;
        }
    }
    // NUL is not allowed in Postgres TEXT.
    String::from_utf8_lossy(&collected).replace('\0', "\u{fffd}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    use crate::testsupport::{TestDb, seed_project, seed_user};
    use axum::extract::State;
    use axum::http::{HeaderMap as AxumHeaders, StatusCode};
    use axum::routing::post;
    use axum::Router;

    // -- pure logic ---------------------------------------------------------

    fn config() -> WebhookConfig {
        WebhookConfig {
            max_retries: 4,
            retry_delay_secs: 10,
            max_retry_delay_secs: 100,
            ..WebhookConfig::default()
        }
    }

    #[test]
    fn signature_is_lowercase_hex_hmac_sha256() {
        // RFC 4231 test case 2
        assert_eq!(
            sign("Jefe", b"what do ya want for nothing?"),
            "5bdcc146bf60754e6a042426089575c75a003f089d2739839dec58b964ec3843"
        );
    }

    #[test]
    fn backoff_grows_exponentially_up_to_the_cap() {
        let base = Duration::from_secs(10);
        let max = Duration::from_secs(100);
        let delays: Vec<u64> = (1..=6).map(|a| backoff_delay(base, max, a).as_secs()).collect();
        assert_eq!(delays, vec![10, 20, 40, 80, 100, 100]);
        // absurd attempt counts must not overflow
        assert_eq!(backoff_delay(base, max, u32::MAX), max);
    }

    #[test]
    fn retry_decisions_follow_the_status_class() {
        let cfg = config();
        let response = |status| Outcome::Response { status, retry_after: None };
        assert_eq!(decide(&response(200), 1, &cfg), Decision::Delivered);
        assert_eq!(decide(&response(204), 1, &cfg), Decision::Delivered);
        assert_eq!(decide(&response(500), 1, &cfg), Decision::Retry(Duration::from_secs(10)));
        assert_eq!(decide(&response(503), 3, &cfg), Decision::Retry(Duration::from_secs(40)));
        assert_eq!(decide(&response(429), 2, &cfg), Decision::Retry(Duration::from_secs(20)));
        assert!(matches!(decide(&response(500), 4, &cfg), Decision::Abandon(_)), "attempts exhausted");
        assert!(matches!(decide(&response(404), 1, &cfg), Decision::Abandon(r) if r.contains("404")));
        assert!(matches!(decide(&response(301), 1, &cfg), Decision::Abandon(_)), "redirects are not followed");
        assert!(matches!(decide(&Outcome::Blocked("x".into()), 1, &cfg), Decision::Abandon(_)));
        assert_eq!(
            decide(&Outcome::NetworkError("connection refused".into()), 1, &cfg),
            Decision::Retry(Duration::from_secs(10))
        );
        // Retry-After raises the delay but never above the cap
        let hinted = Outcome::Response { status: 429, retry_after: Some(Duration::from_secs(60)) };
        assert_eq!(decide(&hinted, 1, &cfg), Decision::Retry(Duration::from_secs(60)));
        let huge = Outcome::Response { status: 429, retry_after: Some(Duration::from_secs(86_400)) };
        assert_eq!(decide(&huge, 1, &cfg), Decision::Retry(Duration::from_secs(100)));
    }

    #[test]
    fn scope_and_ref_filters() {
        assert!(scope_matches(&[], "push"));
        assert!(scope_matches(&["push".into()], "push"));
        assert!(scope_matches(&["build".into()], "build.finished"));
        assert!(scope_matches(&["*".into()], "anything"));
        assert!(!scope_matches(&["issue".into()], "push"));

        assert!(ref_filter_matches(Some("main release-*"), None, Some("refs/heads/release-1")));
        assert!(!ref_filter_matches(Some("main"), None, Some("refs/heads/dev")));
        assert!(ref_filter_matches(None, Some("v*"), Some("refs/tags/v1.0")));
        assert!(!ref_filter_matches(None, Some("v*"), Some("refs/tags/nightly")));
        assert!(ref_filter_matches(Some("main"), Some("v*"), None), "no ref => filters do not apply");
        assert!(ref_filter_matches(Some("main"), None, Some("refs/tags/x")), "branch filter ignores tags");
    }

    #[test]
    fn ssrf_policy_blocks_internal_ranges() {
        for blocked in [
            "127.0.0.1", "10.1.2.3", "172.16.0.1", "172.31.255.254", "192.168.1.1", "169.254.169.254",
            "100.64.0.1", "0.0.0.0", "224.0.0.1", "255.255.255.255", "::1", "fd00::1", "fe80::1", "::",
            "::ffff:127.0.0.1", "::ffff:10.0.0.1",
        ] {
            assert!(is_blocked_ip(blocked.parse().unwrap()), "{blocked} must be blocked");
        }
        for allowed in ["8.8.8.8", "1.1.1.1", "93.184.216.34", "2606:4700:4700::1111"] {
            assert!(!is_blocked_ip(allowed.parse().unwrap()), "{allowed} must be allowed");
        }
    }

    #[test]
    fn target_policy_checks_scheme_host_lists_and_literals() {
        let policy = TargetPolicy {
            allowed_hosts: vec![],
            blocked_hosts: vec!["evil.example.com".into(), "*.bad.org".into()],
            allow_private: false,
        };
        assert!(policy.check("https://hooks.example.com/x").is_ok());
        assert_eq!(policy.check("ftp://hooks.example.com"), Err(TargetError::Scheme));
        assert!(matches!(policy.check("https://evil.example.com/"), Err(TargetError::Blocked(_))));
        assert!(matches!(policy.check("https://a.b.bad.org/"), Err(TargetError::Blocked(_))));
        assert!(matches!(policy.check("http://127.0.0.1:8080/"), Err(TargetError::PrivateAddress(_))));
        assert!(matches!(policy.check("http://[::1]/"), Err(TargetError::PrivateAddress(_))));
        assert!(matches!(policy.check("not a url"), Err(TargetError::Invalid(_))));

        let allow_list = TargetPolicy {
            allowed_hosts: vec!["*.corp.example".into()],
            blocked_hosts: vec![],
            allow_private: false,
        };
        assert!(allow_list.check("https://ci.corp.example/hook").is_ok());
        assert!(matches!(allow_list.check("https://other.example/"), Err(TargetError::NotAllowed(_))));

        let permissive = TargetPolicy { allowed_hosts: vec![], blocked_hosts: vec![], allow_private: true };
        assert!(permissive.check("http://127.0.0.1:9000/").is_ok());
    }

    // -- database + HTTP integration ------------------------------------------

    #[derive(Clone, Default)]
    struct Receiver {
        requests: Arc<Mutex<Vec<(AxumHeaders, Vec<u8>)>>>,
        statuses: Arc<Mutex<Vec<u16>>>,
    }

    async fn start_receiver(statuses: Vec<u16>) -> (String, Receiver) {
        let receiver = Receiver {
            requests: Arc::default(),
            statuses: Arc::new(Mutex::new(statuses)),
        };
        let app = Router::new()
            .route(
                "/hook",
                post(|State(r): State<Receiver>, headers: AxumHeaders, body: axum::body::Bytes| async move {
                    r.requests.lock().unwrap().push((headers, body.to_vec()));
                    let mut statuses = r.statuses.lock().unwrap();
                    let status = if statuses.is_empty() { 200 } else { statuses.remove(0) };
                    (StatusCode::from_u16(status).unwrap(), format!("answered {status}"))
                }),
            )
            .with_state(receiver.clone());
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        (format!("http://{addr}/hook"), receiver)
    }

    async fn seed_webhook(pool: &DbPool, project_id: i64, url: &str, secret: Option<&str>) -> i64 {
        sqlx::query_scalar::<_, i64>(
            "INSERT INTO webhooks (project_id, url, secret, scopes, headers) \
             VALUES ($1, $2, $3, '[]'::jsonb, '{\"X-Custom\": \"yes\", \"X-OneDev-Event\": \"forged\"}'::jsonb) \
             RETURNING id",
        )
        .bind(project_id)
        .bind(url)
        .bind(secret)
        .fetch_one(pool)
        .await
        .unwrap()
    }

    fn local_config() -> WebhookConfig {
        WebhookConfig {
            allow_private_targets: true,
            max_retries: 3,
            retry_delay_secs: 1,
            max_retry_delay_secs: 2,
            timeout_secs: 5,
            ..WebhookConfig::default()
        }
    }

    async fn delivery_row(pool: &DbPool, id: i64) -> (String, i32, Option<i32>, Option<String>, Option<String>) {
        let row = sqlx::query(
            "SELECT status::text AS status, attempt, response_status, response_body, error \
             FROM webhook_deliveries WHERE id = $1",
        )
        .bind(id)
        .fetch_one(pool)
        .await
        .unwrap();
        (
            row.get("status"),
            row.get("attempt"),
            row.get("response_status"),
            row.get("response_body"),
            row.get("error"),
        )
    }

    #[tokio::test]
    async fn delivers_signed_events_with_custom_headers() {
        let Some(db) = TestDb::new().await else { return };
        let owner = seed_user(&db.pool, "owner").await;
        let project = seed_project(&db.pool, owner, "app", None).await;
        let (url, receiver) = start_receiver(vec![200]).await;
        let hook = seed_webhook(&db.pool, project, &url, Some("topsecret")).await;

        let queued = enqueue_event(
            &db.pool,
            &WebhookEvent {
                name: "build.finished".into(),
                project_id: Some(project),
                ref_name: None,
                payload: serde_json::json!({"build": {"number": 7, "status": "success"}}),
            },
        )
        .await
        .unwrap();
        assert_eq!(queued, 1);

        let dispatcher = WebhookDispatcher::new(db.pool.clone(), local_config()).unwrap();
        assert_eq!(dispatcher.process_batch().await.unwrap(), 1);

        let requests = receiver.requests.lock().unwrap();
        assert_eq!(requests.len(), 1);
        let (headers, body) = &requests[0];
        assert_eq!(headers["x-onedev-signature"].to_str().unwrap(), sign("topsecret", body));
        assert_eq!(headers["x-onedev-event"], "build.finished", "reserved headers cannot be forged");
        assert_eq!(headers["x-custom"], "yes");
        assert_eq!(headers["content-type"], "application/json");
        assert!(headers.contains_key("x-onedev-delivery"));
        let parsed: Value = serde_json::from_slice(body).unwrap();
        assert_eq!(parsed["build"]["number"], 7);

        let id: i64 = sqlx::query_scalar("SELECT id FROM webhook_deliveries").fetch_one(&db.pool).await.unwrap();
        let (status, attempt, response_status, response_body, error) = delivery_row(&db.pool, id).await;
        assert_eq!((status.as_str(), attempt, response_status), ("delivered", 1, Some(200)));
        assert_eq!(response_body.as_deref(), Some("answered 200"));
        assert!(error.is_none());
        let last: Option<i32> = sqlx::query_scalar("SELECT last_delivery_status FROM webhooks WHERE id = $1")
            .bind(hook)
            .fetch_one(&db.pool)
            .await
            .unwrap();
        assert_eq!(last, Some(200));
    }

    #[tokio::test]
    async fn retries_server_errors_then_succeeds() {
        let Some(db) = TestDb::new().await else { return };
        let owner = seed_user(&db.pool, "owner").await;
        let project = seed_project(&db.pool, owner, "app", None).await;
        let (url, receiver) = start_receiver(vec![503, 200]).await;
        seed_webhook(&db.pool, project, &url, None).await;
        enqueue_event(
            &db.pool,
            &WebhookEvent { name: "push".into(), project_id: Some(project), ref_name: None, payload: serde_json::json!({}) },
        )
        .await
        .unwrap();
        let id: i64 = sqlx::query_scalar("SELECT id FROM webhook_deliveries").fetch_one(&db.pool).await.unwrap();

        let dispatcher = WebhookDispatcher::new(db.pool.clone(), local_config()).unwrap();
        assert_eq!(dispatcher.process_batch().await.unwrap(), 1);
        let (status, attempt, response_status, _, _) = delivery_row(&db.pool, id).await;
        assert_eq!((status.as_str(), attempt, response_status), ("pending", 1, Some(503)));
        let next: Option<DateTime<Utc>> = sqlx::query_scalar("SELECT next_retry_at FROM webhook_deliveries WHERE id = $1")
            .bind(id)
            .fetch_one(&db.pool)
            .await
            .unwrap();
        assert!(next.unwrap() > Utc::now(), "retry must be scheduled in the future");

        // not due yet -> nothing happens
        assert_eq!(dispatcher.process_batch().await.unwrap(), 0);

        sqlx::query("UPDATE webhook_deliveries SET next_retry_at = NOW() - interval '1 second'")
            .execute(&db.pool)
            .await
            .unwrap();
        assert_eq!(dispatcher.process_batch().await.unwrap(), 1);
        let (status, attempt, response_status, _, _) = delivery_row(&db.pool, id).await;
        assert_eq!((status.as_str(), attempt, response_status), ("delivered", 2, Some(200)));
        assert_eq!(receiver.requests.lock().unwrap().len(), 2);
    }

    #[tokio::test]
    async fn permanent_failures_and_blocked_targets_are_abandoned_immediately() {
        let Some(db) = TestDb::new().await else { return };
        let owner = seed_user(&db.pool, "owner").await;
        let project = seed_project(&db.pool, owner, "app", None).await;
        let (url, receiver) = start_receiver(vec![404]).await;
        seed_webhook(&db.pool, project, &url, None).await;
        enqueue_event(
            &db.pool,
            &WebhookEvent { name: "push".into(), project_id: Some(project), ref_name: None, payload: serde_json::json!({}) },
        )
        .await
        .unwrap();
        let id: i64 = sqlx::query_scalar("SELECT id FROM webhook_deliveries").fetch_one(&db.pool).await.unwrap();

        let dispatcher = WebhookDispatcher::new(db.pool.clone(), local_config()).unwrap();
        dispatcher.process_batch().await.unwrap();
        let (status, attempt, response_status, _, error) = delivery_row(&db.pool, id).await;
        assert_eq!((status.as_str(), attempt, response_status), ("abandoned", 1, Some(404)));
        assert!(error.unwrap().contains("404"));

        // the same target is refused when private targets are not allowed
        sqlx::query("UPDATE webhook_deliveries SET status = 'pending', attempt = 0")
            .execute(&db.pool)
            .await
            .unwrap();
        let strict = WebhookDispatcher::new(
            db.pool.clone(),
            WebhookConfig { allow_private_targets: false, ..local_config() },
        )
        .unwrap();
        strict.process_batch().await.unwrap();
        let (status, _, response_status, _, error) = delivery_row(&db.pool, id).await;
        assert_eq!(status, "abandoned");
        assert_eq!(response_status, None);
        assert!(error.unwrap().contains("private or reserved"));
        assert_eq!(receiver.requests.lock().unwrap().len(), 1, "blocked delivery must not hit the network");
    }

    #[tokio::test]
    async fn network_errors_exhaust_retries() {
        let Some(db) = TestDb::new().await else { return };
        let owner = seed_user(&db.pool, "owner").await;
        let project = seed_project(&db.pool, owner, "app", None).await;
        // a port nobody listens on
        let closed = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}/hook", closed.local_addr().unwrap());
        drop(closed);
        seed_webhook(&db.pool, project, &url, None).await;
        enqueue_event(
            &db.pool,
            &WebhookEvent { name: "push".into(), project_id: Some(project), ref_name: None, payload: serde_json::json!({}) },
        )
        .await
        .unwrap();
        let id: i64 = sqlx::query_scalar("SELECT id FROM webhook_deliveries").fetch_one(&db.pool).await.unwrap();
        let dispatcher = WebhookDispatcher::new(db.pool.clone(), local_config()).unwrap();

        for expected_attempt in 1..=3 {
            sqlx::query("UPDATE webhook_deliveries SET next_retry_at = NOW() - interval '1 second' WHERE status = 'pending'")
                .execute(&db.pool)
                .await
                .unwrap();
            assert_eq!(dispatcher.process_batch().await.unwrap(), 1);
            let (status, attempt, _, _, error) = delivery_row(&db.pool, id).await;
            assert_eq!(attempt, expected_attempt);
            assert!(error.is_some());
            if expected_attempt < 3 {
                assert_eq!(status, "pending");
            } else {
                assert_eq!(status, "abandoned");
            }
        }
    }

    #[tokio::test]
    async fn enqueue_respects_scopes_filters_and_global_hooks() {
        let Some(db) = TestDb::new().await else { return };
        let owner = seed_user(&db.pool, "owner").await;
        let project = seed_project(&db.pool, owner, "app", None).await;
        let other = seed_project(&db.pool, owner, "other", None).await;
        let pool = &db.pool;
        let insert = |project: Option<i64>, scopes: &'static str, branch: Option<&'static str>, active: bool| async move {
            sqlx::query(
                "INSERT INTO webhooks (project_id, url, scopes, branch_filter, active) VALUES ($1, 'http://example.test/h', $2::jsonb, $3, $4)",
            )
            .bind(project)
            .bind(scopes)
            .bind(branch)
            .bind(active)
            .execute(pool)
            .await
            .unwrap();
        };
        insert(Some(project), "[\"push\"]", Some("main"), true).await; // matches main pushes
        insert(Some(project), "[\"issue\"]", None, true).await; // wrong scope
        insert(Some(project), "[]", None, false).await; // inactive
        insert(Some(other), "[]", None, true).await; // other project
        insert(None, "[]", None, true).await; // global

        let push = |ref_name: &str| WebhookEvent {
            name: "push".into(),
            project_id: Some(project),
            ref_name: Some(ref_name.into()),
            payload: serde_json::json!({}),
        };
        assert_eq!(enqueue_event(pool, &push("refs/heads/main")).await.unwrap(), 2, "project hook + global hook");
        assert_eq!(enqueue_event(pool, &push("refs/heads/dev")).await.unwrap(), 1, "branch filter excludes the project hook");
    }

    #[tokio::test]
    async fn stuck_sending_deliveries_are_requeued() {
        let Some(db) = TestDb::new().await else { return };
        let owner = seed_user(&db.pool, "owner").await;
        let project = seed_project(&db.pool, owner, "app", None).await;
        let (url, receiver) = start_receiver(vec![200]).await;
        seed_webhook(&db.pool, project, &url, None).await;
        enqueue_event(
            &db.pool,
            &WebhookEvent { name: "push".into(), project_id: Some(project), ref_name: None, payload: serde_json::json!({}) },
        )
        .await
        .unwrap();
        // simulate a dispatcher that crashed after claiming
        sqlx::query("UPDATE webhook_deliveries SET status = 'sending', attempt = 1, last_attempt_at = NOW() - interval '1 hour'")
            .execute(&db.pool)
            .await
            .unwrap();
        let dispatcher = WebhookDispatcher::new(db.pool.clone(), local_config()).unwrap();
        assert_eq!(dispatcher.process_batch().await.unwrap(), 1);
        assert_eq!(receiver.requests.lock().unwrap().len(), 1);
        let status: String = sqlx::query_scalar("SELECT status::text FROM webhook_deliveries")
            .fetch_one(&db.pool)
            .await
            .unwrap();
        assert_eq!(status, "delivered");
    }

    #[tokio::test]
    async fn concurrent_dispatchers_never_deliver_twice() {
        let Some(db) = TestDb::new().await else { return };
        let owner = seed_user(&db.pool, "owner").await;
        let project = seed_project(&db.pool, owner, "app", None).await;
        let (url, receiver) = start_receiver(vec![]).await;
        seed_webhook(&db.pool, project, &url, None).await;
        for n in 0..20 {
            enqueue_event(
                &db.pool,
                &WebhookEvent { name: "push".into(), project_id: Some(project), ref_name: None, payload: serde_json::json!({"n": n}) },
            )
            .await
            .unwrap();
        }
        let a = Arc::new(WebhookDispatcher::new(db.pool.clone(), WebhookConfig { batch_size: 7, ..local_config() }).unwrap());
        let b = Arc::new(WebhookDispatcher::new(db.pool.clone(), WebhookConfig { batch_size: 7, ..local_config() }).unwrap());
        let run = |d: Arc<WebhookDispatcher>| async move {
            let mut total = 0;
            loop {
                let n = d.process_batch().await.unwrap();
                if n == 0 {
                    break;
                }
                total += n;
            }
            total
        };
        let (x, y) = tokio::join!(run(a), run(b));
        assert_eq!(x + y, 20);
        assert_eq!(receiver.requests.lock().unwrap().len(), 20, "each event is delivered exactly once");
        let delivered: i64 = sqlx::query_scalar("SELECT count(*) FROM webhook_deliveries WHERE status = 'delivered'")
            .fetch_one(&db.pool)
            .await
            .unwrap();
        assert_eq!(delivered, 20);
    }
}
