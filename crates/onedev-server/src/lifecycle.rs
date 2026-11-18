//! Server lifecycle: readiness state and cooperative shutdown.

use std::time::{Duration, Instant};

use tokio::sync::{broadcast, watch};
use tracing::{info, warn};

/// Coarse health of the server.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HealthStatus {
    Starting,
    Healthy,
    Degraded(String),
    ShuttingDown,
}

impl HealthStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            HealthStatus::Starting => "starting",
            HealthStatus::Healthy => "healthy",
            HealthStatus::Degraded(_) => "degraded",
            HealthStatus::ShuttingDown => "shutting_down",
        }
    }
}

/// Shared lifecycle handle: readiness plus a shutdown broadcast every
/// background service subscribes to.
pub struct Lifecycle {
    shutdown_tx: broadcast::Sender<()>,
    status_tx: watch::Sender<HealthStatus>,
    started_at: Instant,
}

impl Lifecycle {
    pub fn new() -> Self {
        let (shutdown_tx, _) = broadcast::channel(4);
        let (status_tx, _) = watch::channel(HealthStatus::Starting);
        Self {
            shutdown_tx,
            status_tx,
            started_at: Instant::now(),
        }
    }

    /// Receiver that fires once when shutdown begins.
    pub fn subscribe(&self) -> broadcast::Receiver<()> {
        self.shutdown_tx.subscribe()
    }

    pub fn status(&self) -> HealthStatus {
        self.status_tx.borrow().clone()
    }

    pub fn watch_status(&self) -> watch::Receiver<HealthStatus> {
        self.status_tx.subscribe()
    }

    pub fn uptime(&self) -> Duration {
        self.started_at.elapsed()
    }

    pub fn mark_ready(&self) {
        // Never leave the ShuttingDown state again.
        self.status_tx.send_if_modified(|status| {
            if *status == HealthStatus::ShuttingDown || *status == HealthStatus::Healthy {
                false
            } else {
                *status = HealthStatus::Healthy;
                true
            }
        });
        info!("server is ready");
    }

    pub fn mark_degraded(&self, reason: impl Into<String>) {
        let reason = reason.into();
        warn!(%reason, "server degraded");
        self.status_tx.send_if_modified(|status| {
            if *status == HealthStatus::ShuttingDown {
                false
            } else {
                *status = HealthStatus::Degraded(reason);
                true
            }
        });
    }

    /// Ready to receive traffic (degraded still serves requests).
    pub fn is_ready(&self) -> bool {
        matches!(
            *self.status_tx.borrow(),
            HealthStatus::Healthy | HealthStatus::Degraded(_)
        )
    }

    pub fn is_shutting_down(&self) -> bool {
        *self.status_tx.borrow() == HealthStatus::ShuttingDown
    }

    /// Flip to `ShuttingDown` and notify all subscribers (idempotent).
    pub fn begin_shutdown(&self) {
        let first = self.status_tx.send_if_modified(|status| {
            if *status == HealthStatus::ShuttingDown {
                false
            } else {
                *status = HealthStatus::ShuttingDown;
                true
            }
        });
        if first {
            info!("shutdown initiated");
            let _ = self.shutdown_tx.send(());
        }
    }
}

impl Default for Lifecycle {
    fn default() -> Self {
        Self::new()
    }
}

/// Shutdown notification usable inside `tokio::select!`.
///
/// Unlike a bare broadcast receiver it also fires when shutdown began before
/// the signal was created, so a late-starting task can never miss it.
pub struct ShutdownSignal {
    rx: broadcast::Receiver<()>,
    lifecycle: std::sync::Arc<Lifecycle>,
}

impl ShutdownSignal {
    /// Resolves once shutdown has begun (cancel safe, may be awaited repeatedly).
    pub async fn wait(&mut self) {
        if self.lifecycle.is_shutting_down() {
            return;
        }
        // Err(Closed|Lagged) also means "stop".
        let _ = self.rx.recv().await;
    }

    pub fn is_set(&self) -> bool {
        self.lifecycle.is_shutting_down()
    }

    /// Independent signal for another task.
    pub fn child(&self) -> ShutdownSignal {
        self.lifecycle.signal()
    }
}

impl Lifecycle {
    /// A fresh [`ShutdownSignal`].
    pub fn signal(self: &std::sync::Arc<Self>) -> ShutdownSignal {
        ShutdownSignal {
            rx: self.subscribe(),
            lifecycle: self.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn readiness_transitions() {
        let lifecycle = Lifecycle::new();
        assert!(!lifecycle.is_ready());
        lifecycle.mark_ready();
        assert!(lifecycle.is_ready());
        lifecycle.mark_degraded("db slow");
        assert!(lifecycle.is_ready());
        assert_eq!(lifecycle.status().as_str(), "degraded");
    }

    #[tokio::test]
    async fn signal_created_after_shutdown_fires_immediately() {
        let lifecycle = std::sync::Arc::new(Lifecycle::new());
        lifecycle.begin_shutdown();
        let mut late = lifecycle.signal();
        tokio::time::timeout(std::time::Duration::from_millis(200), late.wait())
            .await
            .expect("late signal must resolve");
        assert!(late.is_set());
        // and repeatedly
        tokio::time::timeout(std::time::Duration::from_millis(200), late.wait())
            .await
            .expect("second wait must resolve too");
    }

    #[tokio::test]
    async fn signal_wakes_waiting_tasks() {
        let lifecycle = std::sync::Arc::new(Lifecycle::new());
        let mut signal = lifecycle.signal();
        let waiter = tokio::spawn(async move { signal.wait().await });
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        lifecycle.begin_shutdown();
        tokio::time::timeout(std::time::Duration::from_secs(1), waiter)
            .await
            .expect("waiter wakes")
            .unwrap();
    }

    #[tokio::test]
    async fn shutdown_is_final_and_broadcast_once() {
        let lifecycle = Lifecycle::new();
        let mut rx = lifecycle.subscribe();
        lifecycle.mark_ready();
        lifecycle.begin_shutdown();
        lifecycle.begin_shutdown();
        assert!(rx.recv().await.is_ok());
        assert!(rx.try_recv().is_err(), "second shutdown must not re-broadcast");
        lifecycle.mark_ready();
        lifecycle.mark_degraded("late");
        assert!(lifecycle.is_shutting_down());
        assert!(!lifecycle.is_ready());
    }
}
