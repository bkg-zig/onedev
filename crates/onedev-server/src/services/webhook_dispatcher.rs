//! Webhook Dispatcher Service

use anyhow::Result;
use chrono::{DateTime, Utc};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::{PgPool, Row};
use std::sync::Arc;
use tokio::sync::broadcast;
use tokio::time::{Duration, Instant};
use tracing::{info, warn, error, debug};
use uuid::Uuid;

/// Webhook dispatcher configuration
#[derive(Debug, Clone)]
pub struct WebhookDispatcherConfig {
    pub max_retries: u32,
    pub retry_delay_secs: u64,
    pub max_retry_delay_secs: u64,
    pub timeout_secs: u64,
    pub verify_ssl: bool,
    pub poll_interval_secs: u64,
    pub batch_size: usize,
}

impl Default for WebhookDispatcherConfig {
    fn default() -> Self {
        Self {
            max_retries: 3,
            retry_delay_secs: 10,
            max_retry_delay_secs: 300,
            timeout_secs: 30,
            verify_ssl: true,
            poll_interval_secs: 30,
            batch_size: 50,
        }
    }
}

/// Webhook delivery status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "webhook_delivery_status", rename_all = "lowercase")]
pub enum DeliveryStatus {
    Pending,
    Delivered,
    Failed,
    Abandoned,
}

/// Webhook payload
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebhookPayload {
    pub event: String,
    pub timestamp: DateTime<Utc>,
    pub project_id: Option<Uuid>,
    pub actor_id: Option<Uuid>,
    pub payload: Value,
}

/// Webhook delivery record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebhookDelivery {
    pub id: Uuid,
    pub webhook_id: Uuid,
    pub event: String,
    pub payload: Value,
    pub url: String,
    pub status: DeliveryStatus,
    pub attempt: u32,
    pub last_attempt_at: Option<DateTime<Utc>>,
    pub next_retry_at: Option<DateTime<Utc>>,
    pub response_status: Option<u16>,
    pub response_body: Option<String>,
    pub error: Option<String>,
    pub created_at: DateTime<Utc>,
    pub delivered_at: Option<DateTime<Utc>>,
}

/// Webhook dispatcher service
pub struct WebhookDispatcher {
    db_pool: Arc<PgPool>,
    config: WebhookDispatcherConfig,
    http_client: Client,
    shutdown_rx: broadcast::Receiver<()>,
}

impl WebhookDispatcher {
    /// Create a new webhook dispatcher
    pub fn new(
        db_pool: Arc<PgPool>,
        config: WebhookDispatcherConfig,
        shutdown_rx: broadcast::Receiver<()>,
    ) -> Result<Self> {
        let http_client = Client::builder()
            .timeout(Duration::from_secs(config.timeout_secs))
            .danger_accept_invalid_certs(!config.verify_ssl)
            .build()?;

        Ok(Self {
            db_pool,
            config,
            http_client,
            shutdown_rx,
        })
    }

    /// Run the dispatcher
    pub async fn run(mut self) {
        info!("Webhook dispatcher started");

        let mut interval = tokio::time::interval(
            Duration::from_secs(self.config.poll_interval_secs)
        );

        // Process pending at startup
        if let Err(e) = self.process_pending().await {
            error!("Initial webhook processing failed: {}", e);
        }

        loop {
            tokio::select! {
                _ = interval.tick() => {
                    if let Err(e) = self.process_pending().await {
                        error!("Webhook processing failed: {}", e);
                    }
                    if let Err(e) = self.retry_failed().await {
                        error!("Webhook retry failed: {}", e);
                    }
                }
                _ = self.shutdown_rx.recv() => {
                    info!("Webhook dispatcher received shutdown signal");
                    break;
                }
            }
        }

        info!("Webhook dispatcher stopped");
    }

    /// Process pending webhook deliveries
    async fn process_pending(&self) -> Result<()> {
        let deliveries = sqlx::query(
            r#"
            SELECT id, webhook_id, event, payload, url, status, attempt, last_attempt_at, next_retry_at,
                   response_status, response_body, error, created_at, delivered_at
            FROM webhook_deliveries
            WHERE status = 'Pending'
            AND (next_retry_at IS NULL OR next_retry_at <= NOW())
            ORDER BY created_at
            LIMIT $1
            FOR UPDATE SKIP LOCKED
            "#
        )
            .bind(self.config.batch_size as i64)
            .fetch_all(self.db_pool.as_ref()).await?;

        if deliveries.is_empty() {
            return Ok(());
        }

        debug!("Processing {} pending webhook deliveries", deliveries.len());

        for delivery in deliveries {
            let delivery_id: Uuid = delivery.try_get("id")?;
            let webhook_id: Uuid = delivery.try_get("webhook_id")?;
            let event: String = delivery.try_get("event")?;
            let payload: Value = delivery.try_get("payload")?;
            let url: String = delivery.try_get("url")?;
            let attempt: i32 = delivery.try_get("attempt").unwrap_or(0) + 1;

            // Update attempt count
            sqlx::query("UPDATE webhook_deliveries SET attempt = $1, last_attempt_at = $2 WHERE id = $3")
                .bind(attempt)
                .bind(Utc::now())
                .bind(delivery_id)
                .execute(self.db_pool.as_ref()).await?;

            // Create webhook payload
            let webhook_payload = WebhookPayload {
                event: event.clone(),
                timestamp: Utc::now(),
                project_id: None, // Would extract from payload
                actor_id: None,   // Would extract from payload
                payload: payload.clone(),
            };

            // Attempt delivery
            match self.deliver(&url, &webhook_payload, webhook_id).await {
                Ok(response) => {
                    info!("Webhook {} delivered successfully to {}", delivery_id, url);
                    let status_code = response.status().as_u16() as i16;
                    let response_body = response.text().await.unwrap_or_default();
                    sqlx::query(
                        r#"
                        UPDATE webhook_deliveries
                        SET status = 'Delivered', delivered_at = $1, response_status = $2, response_body = $3
                        WHERE id = $4
                        "#
                    )
                        .bind(Utc::now())
                        .bind(status_code)
                        .bind(response_body)
                        .bind(delivery_id)
                        .execute(self.db_pool.as_ref()).await?;
                }
                Err(e) => {
                    warn!("Webhook {} delivery failed (attempt {}): {}", delivery_id, attempt, e);

                    if attempt >= self.config.max_retries as i32 {
                        // Abandon
                        error!("Webhook {} abandoned after {} attempts", delivery_id, attempt);
                        sqlx::query(
                            r#"
                            UPDATE webhook_deliveries
                            SET status = 'Abandoned', error = $1
                            WHERE id = $2
                            "#
                        )
                            .bind(e.to_string())
                            .bind(delivery_id)
                            .execute(self.db_pool.as_ref()).await?;
                    } else {
                        // Schedule retry
                        let delay = self.calculate_backoff(attempt as u32);
                        let next_retry = Utc::now() + chrono::Duration::seconds(delay as i64);

                        sqlx::query(
                            r#"
                            UPDATE webhook_deliveries
                            SET status = 'Pending', next_retry_at = $1, error = $2
                            WHERE id = $3
                            "#
                        )
                            .bind(next_retry)
                            .bind(e.to_string())
                            .bind(delivery_id)
                            .execute(self.db_pool.as_ref()).await?;
                    }
                }
            }
        }

        Ok(())
    }

    /// Retry failed deliveries
    async fn retry_failed(&self) -> Result<()> {
        let deliveries = sqlx::query(
            r#"
            SELECT id FROM webhook_deliveries
            WHERE status = 'Failed'
            AND next_retry_at <= NOW()
            AND attempt < $1
            LIMIT $2
            "#
        )
            .bind(self.config.max_retries as i32)
            .bind(self.config.batch_size as i64)
            .fetch_all(self.db_pool.as_ref()).await?;

        for delivery in deliveries {
            let id: Uuid = delivery.try_get("id")?;
            // Just update status to Pending, process_pending will pick it up
            sqlx::query("UPDATE webhook_deliveries SET status = 'Pending' WHERE id = $1")
                .bind(id)
                .execute(self.db_pool.as_ref()).await?;
        }

        Ok(())
    }

    /// Deliver webhook to URL
    async fn deliver(
        &self,
        url: &str,
        payload: &WebhookPayload,
        webhook_id: Uuid,
    ) -> Result<reqwest::Response> {
        let body = serde_json::to_vec(payload)?;

        // Get webhook secret for signature
        let secret = sqlx::query("SELECT secret FROM webhooks WHERE id = $1")
            .bind(webhook_id)
            .fetch_optional(self.db_pool.as_ref()).await?;

        let mut request = self.http_client
            .post(url)
            .header("Content-Type", "application/json")
            .header("User-Agent", "OneDev-Webhook/1.0")
            .header("X-OneDev-Event", &payload.event)
            .header("X-OneDev-Delivery", Uuid::new_v4().to_string())
            .body(body);

        // Add signature if secret exists
        if let Some(secret) = secret {
            let secret_str: String = secret.try_get("secret")?;
            let signature = self.compute_signature(&secret_str, &payload)?;
            request = request.header("X-OneDev-Signature", signature);
        }

        let response = request.send().await?;

        if !response.status().is_success() {
            return Err(anyhow::anyhow!(
                "Webhook returned status {}",
                response.status()
            ));
        }

        Ok(response)
    }

    /// Compute HMAC signature
    fn compute_signature(&self, secret: &str, payload: &WebhookPayload) -> Result<String> {
        use hmac::{Hmac, Mac};
        use sha2::Sha256;

        let body = serde_json::to_vec(payload)?;
        let mut mac = Hmac::<Sha256>::new_from_slice(secret.as_bytes())?;
        mac.update(&body);
        let result = mac.finalize();
        Ok(hex::encode(result.into_bytes()))
    }

    /// Calculate exponential backoff
    fn calculate_backoff(&self, attempt: u32) -> u64 {
        let base = self.config.retry_delay_secs;
        let max = self.config.max_retry_delay_secs;
        let delay = base * 2_u64.pow(attempt - 1);
        delay.min(max)
    }

    /// Queue a webhook delivery
    pub async fn queue_delivery(
        &self,
        webhook_id: Uuid,
        event: &str,
        payload: Value,
        url: &str,
    ) -> Result<Uuid> {
        let delivery_id = Uuid::new_v4();

        sqlx::query(
            r#"
            INSERT INTO webhook_deliveries (id, webhook_id, event, payload, url, status, attempt, created_at)
            VALUES ($1, $2, $3, $4, $5, 'Pending', 0, $6)
            "#
        )
            .bind(delivery_id)
            .bind(webhook_id)
            .bind(event)
            .bind(payload)
            .bind(url)
            .bind(Utc::now())
            .execute(self.db_pool.as_ref()).await?;

        Ok(delivery_id)
    }

    /// Get delivery stats
    pub async fn get_stats(&self) -> Result<WebhookStats> {
        let stats = sqlx::query(
            r#"
            SELECT
                status,
                COUNT(*) as count
            FROM webhook_deliveries
            WHERE created_at > NOW() - INTERVAL '24 hours'
            GROUP BY status
            "#
        ).fetch_all(self.db_pool.as_ref()).await?;

        let mut by_status = std::collections::HashMap::new();
        for row in stats {
            let status: String = row.try_get("status")?;
            let count: i64 = row.try_get("count").unwrap_or(0);
            by_status.insert(status, count as usize);
        }

        let avg_latency = sqlx::query(
            r#"
            SELECT AVG(EXTRACT(EPOCH FROM (delivered_at - created_at))) as avg_secs
            FROM webhook_deliveries
            WHERE status = 'Delivered'
            AND created_at > NOW() - INTERVAL '24 hours'
            "#
        ).fetch_one(self.db_pool.as_ref()).await?;

        let avg_secs: Option<f64> = avg_latency.try_get("avg_secs").ok();

        Ok(WebhookStats {
            by_status,
            avg_delivery_latency_secs: avg_secs,
        })
    }
}

/// Webhook statistics
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct WebhookStats {
    pub by_status: std::collections::HashMap<String, usize>,
    pub avg_delivery_latency_secs: Option<f64>,
}