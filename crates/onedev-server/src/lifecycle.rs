//! Server Lifecycle Management

use anyhow::Result;
use std::sync::Arc;
use tokio::sync::broadcast;
use tracing::{info, warn, error};

/// Lifecycle manager for graceful startup/shutdown
pub struct LifecycleManager {
    shutdown_tx: broadcast::Sender<()>,
    startup_complete: Arc<tokio::sync::Notify>,
    health_check: Arc<tokio::sync::RwLock<HealthStatus>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HealthStatus {
    Starting,
    Healthy,
    Degraded,
    Unhealthy,
    ShuttingDown,
}

impl LifecycleManager {
    /// Create a new lifecycle manager
    pub fn new() -> Self {
        let (shutdown_tx, _) = broadcast::channel(16);
        Self {
            shutdown_tx,
            startup_complete: Arc::new(tokio::sync::Notify::new()),
            health_check: Arc::new(tokio::sync::RwLock::new(HealthStatus::Starting)),
        }
    }

    /// Get shutdown receiver for components
    pub fn shutdown_rx(&self) -> broadcast::Receiver<()> {
        self.shutdown_tx.subscribe()
    }

    /// Mark startup as complete
    pub fn mark_startup_complete(&self) {
        *self.health_check.try_write().unwrap() = HealthStatus::Healthy;
        self.startup_complete.notify_waiters();
        info!("Server startup complete");
    }

    /// Mark server as degraded
    pub fn mark_degraded(&self, reason: &str) {
        *self.health_check.try_write().unwrap() = HealthStatus::Degraded;
        warn!("Server degraded: {}", reason);
    }

    /// Mark server as unhealthy
    pub fn mark_unhealthy(&self, reason: &str) {
        *self.health_check.try_write().unwrap() = HealthStatus::Unhealthy;
        error!("Server unhealthy: {}", reason);
    }

    /// Initiate graceful shutdown
    pub async fn shutdown(&self) -> Result<()> {
        info!("Initiating graceful shutdown...");
        *self.health_check.try_write().unwrap() = HealthStatus::ShuttingDown;

        // Send shutdown signal
        let _ = self.shutdown_tx.send(());

        // Wait for components to shut down
        tokio::time::sleep(std::time::Duration::from_secs(5)).await;

        info!("Graceful shutdown complete");
        Ok(())
    }

    /// Wait for startup to complete
    pub async fn wait_for_startup(&self) {
        self.startup_complete.notified().await;
    }

    /// Get current health status
    pub async fn health_status(&self) -> HealthStatus {
        *self.health_check.read().await
    }

    /// Check if server is ready to serve traffic
    pub async fn is_ready(&self) -> bool {
        matches!(self.health_status().await, HealthStatus::Healthy)
    }

    /// Check if server is alive (not shutting down)
    pub async fn is_alive(&self) -> bool {
        !matches!(self.health_status().await, HealthStatus::ShuttingDown)
    }
}

impl Default for LifecycleManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Lifecycle hooks for components
#[async_trait::async_trait]
pub trait LifecycleHook: Send + Sync {
    /// Called during startup, before server accepts traffic
    async fn on_startup(&self) -> Result<()> {
        Ok(())
    }

    /// Called when server is ready to accept traffic
    async fn on_ready(&self) -> Result<()> {
        Ok(())
    }

    /// Called during graceful shutdown
    async fn on_shutdown(&self) -> Result<()> {
        Ok(())
    }

    /// Called for health checks
    async fn health_check(&self) -> HealthCheckResult {
        HealthCheckResult::Healthy
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HealthCheckResult {
    Healthy,
    Degraded(String),
    Unhealthy(String),
}

/// Registry for lifecycle hooks
pub struct HookRegistry {
    hooks: Vec<Box<dyn LifecycleHook>>,
}

impl HookRegistry {
    pub fn new() -> Self {
        Self { hooks: Vec::new() }
    }

    pub fn register(&mut self, hook: Box<dyn LifecycleHook>) {
        self.hooks.push(hook);
    }

    pub async fn run_startup(&self) -> Result<()> {
        for hook in &self.hooks {
            hook.on_startup().await?;
        }
        Ok(())
    }

    pub async fn run_ready(&self) -> Result<()> {
        for hook in &self.hooks {
            hook.on_ready().await?;
        }
        Ok(())
    }

    pub async fn run_shutdown(&self) -> Result<()> {
        // Run in reverse order
        for hook in self.hooks.iter().rev() {
            if let Err(e) = hook.on_shutdown().await {
                error!("Shutdown hook failed: {}", e);
            }
        }
        Ok(())
    }

    pub async fn run_health_checks(&self) -> Vec<(String, HealthCheckResult)> {
        let mut results = Vec::new();
        for hook in &self.hooks {
            let result = hook.health_check().await;
            results.push((std::any::type_name_of_val(hook).to_string(), result));
        }
        results
    }
}

impl Default for HookRegistry {
    fn default() -> Self {
        Self::new()
    }
}