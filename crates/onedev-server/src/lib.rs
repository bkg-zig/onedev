//! OneDev main server: composition root, lifecycle, background services.
//!
//! [`Server::start`] builds everything (database, services, HTTP stack),
//! [`Server::wait`] serves until shutdown is requested and then drains in a
//! fixed order: stop accepting connections, finish in-flight requests, stop
//! the background services (running builds are cancelled and record their
//! outcome), close the database pool.

pub mod config;
pub mod http;
pub mod lifecycle;
pub mod metrics;
pub mod middleware;
pub mod services;
pub mod state;
pub mod telemetry;

#[cfg(test)]
mod testsupport;

use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;
use std::time::Duration;

use anyhow::Context as _;
use axum::Router;
use tokio::net::TcpListener;
use tokio::task::JoinHandle;
use tracing::{error, info, warn};

pub use config::AppConfig;
pub use lifecycle::{HealthStatus, Lifecycle, ShutdownSignal};
pub use state::AppState;

use crate::metrics::MetricsEndpoint;
use crate::services::supervise;

/// Extra time the background services get on top of the request grace period
/// (the build scheduler waits for the builds it cancelled).
const SERVICE_STOP_MARGIN: Duration = Duration::from_secs(5);

/// A started server: listening, services running, not yet serving requests
/// until [`Server::wait`] is awaited.
pub struct Server {
    state: AppState,
    router: Router,
    listener: TcpListener,
    addr: SocketAddr,
    tasks: Vec<JoinHandle<()>>,
}

impl Server {
    /// Bring the server up: listen, connect and migrate the database, build
    /// the services and start the background tasks.
    ///
    /// The configuration must have been validated ([`AppConfig::validate`]).
    pub async fn start(config: AppConfig) -> anyhow::Result<Server> {
        let config = Arc::new(config);
        let metrics_handle = metrics::install()?;

        // Fail fast on the most common deployment error (port in use)
        // before anything else is touched.
        let ip: IpAddr = config
            .server
            .bind_address
            .parse()
            .with_context(|| format!("server.bind_address `{}` is not an IP address", config.server.bind_address))?;
        let listener = TcpListener::bind(SocketAddr::new(ip, config.server.port))
            .await
            .with_context(|| format!("cannot listen on {}:{}", config.server.bind_address, config.server.port))?;
        let addr = listener.local_addr().context("cannot read the listening address")?;

        state::prepare_directories(&config)?;
        let pool = state::connect_database(&config.database).await?;
        if config.database.run_migrations {
            state::migrate(&pool).await?;
        }

        let lifecycle = Arc::new(Lifecycle::new());
        let state = AppState::build(config.clone(), pool, lifecycle.clone()).await?;

        let endpoint = MetricsEndpoint::new(
            metrics_handle.clone(),
            config.observability.metrics_enabled,
            config.observability.metrics_token.as_deref(),
        );
        let (router, handles) = middleware::apply(http::application(&state, endpoint), &config)?;
        let tasks = spawn_services(&state, metrics_handle, handles.limiter);

        lifecycle.mark_ready();
        info!(
            %addr,
            environment = %config.server.environment,
            version = env!("CARGO_PKG_VERSION"),
            "onedev server listening"
        );
        Ok(Server {
            state,
            router,
            listener,
            addr,
            tasks,
        })
    }

    /// Address the server listens on (useful with port 0).
    pub fn local_addr(&self) -> SocketAddr {
        self.addr
    }

    pub fn lifecycle(&self) -> Arc<Lifecycle> {
        self.state.lifecycle.clone()
    }

    pub fn state(&self) -> &AppState {
        &self.state
    }

    /// Serve until shutdown is requested ([`Lifecycle::begin_shutdown`]),
    /// then stop everything in order.
    pub async fn wait(self) -> anyhow::Result<()> {
        let Server {
            state,
            router,
            listener,
            addr: _,
            tasks,
        } = self;
        let grace = Duration::from_secs(state.config.server.shutdown_grace_secs.max(1));
        let mut shutdown = state.lifecycle.signal();
        let mut stop_accepting = state.lifecycle.signal();

        let service = router.into_make_service_with_connect_info::<SocketAddr>();
        let mut serve = tokio::spawn(async move {
            axum::serve(listener, service)
                .with_graceful_shutdown(async move { stop_accepting.wait().await })
                .await
        });

        let outcome = tokio::select! {
            result = &mut serve => {
                // the HTTP server ended without being asked to
                state.lifecycle.begin_shutdown();
                server_result(result)
            }
            _ = shutdown.wait() => {
                match tokio::time::timeout(grace, &mut serve).await {
                    Ok(result) => server_result(result),
                    Err(_) => {
                        warn!("open connections did not finish within the grace period, closing them");
                        serve.abort();
                        Ok(())
                    }
                }
            }
        };

        let deadline = tokio::time::Instant::now() + grace + SERVICE_STOP_MARGIN;
        for mut task in tasks {
            match tokio::time::timeout_at(deadline, &mut task).await {
                Ok(Ok(())) => {}
                Ok(Err(join_error)) => error!(%join_error, "a background service failed while stopping"),
                Err(_) => {
                    warn!("a background service did not stop in time, abandoning it");
                    task.abort();
                }
            }
        }
        state.pool.close().await;
        info!("server stopped");
        outcome
    }
}

fn server_result(result: Result<std::io::Result<()>, tokio::task::JoinError>) -> anyhow::Result<()> {
    match result {
        Ok(Ok(())) => Ok(()),
        Ok(Err(error)) => Err(anyhow::Error::new(error).context("the HTTP server failed")),
        Err(join_error) => Err(anyhow::Error::new(join_error).context("the HTTP server task failed")),
    }
}

/// Start the supervised background services.
fn spawn_services(
    state: &AppState,
    metrics: metrics_exporter_prometheus::PrometheusHandle,
    limiter: Option<Arc<middleware::KeyedLimiter>>,
) -> Vec<JoinHandle<()>> {
    let config = &state.config;
    let lifecycle = &state.lifecycle;
    let mut tasks = Vec::new();

    if config.features.builds {
        let scheduler = state.scheduler.clone();
        tasks.push(tokio::spawn(supervise(
            "build_scheduler",
            lifecycle.clone(),
            move |signal| scheduler.clone().run(signal),
        )));
    } else {
        info!("builds are disabled (features.builds = false)");
    }

    if config.features.webhooks {
        let dispatcher = state.webhooks.clone();
        tasks.push(tokio::spawn(supervise(
            "webhook_dispatcher",
            lifecycle.clone(),
            move |signal| dispatcher.clone().run(signal),
        )));
    } else {
        info!("webhook delivery is disabled (features.webhooks = false)");
    }

    if config.observability.metrics_enabled {
        let pool = state.pool.clone();
        let interval = Duration::from_secs(config.observability.db_metrics_interval_secs.max(1));
        let started = state.started_at;
        tasks.push(tokio::spawn(supervise(
            "metrics_collector",
            lifecycle.clone(),
            move |signal| metrics::run_collector(pool.clone(), metrics.clone(), interval, started, signal),
        )));
    }

    if let Some(limiter) = limiter {
        tasks.push(tokio::spawn(supervise(
            "rate_limiter_gc",
            lifecycle.clone(),
            move |signal| middleware::run_limiter_gc(limiter.clone(), signal),
        )));
    }

    tasks
}

/// Run the server until SIGINT / SIGTERM.
pub async fn run(config: AppConfig) -> anyhow::Result<()> {
    let server = Server::start(config).await?;
    handle_signals(server.lifecycle());
    server.wait().await
}

/// First SIGINT / SIGTERM starts a graceful shutdown, a second one exits at once.
pub fn handle_signals(lifecycle: Arc<Lifecycle>) {
    tokio::spawn(async move {
        let mut signals = match ShutdownSignals::new() {
            Ok(signals) => signals,
            Err(error) => {
                warn!(%error, "cannot install signal handlers, the server can only be killed");
                return;
            }
        };
        signals.next().await;
        info!("shutdown signal received, stopping gracefully (signal again to force)");
        lifecycle.begin_shutdown();
        signals.next().await;
        warn!("second shutdown signal received, exiting immediately");
        std::process::exit(130);
    });
}

#[cfg(unix)]
struct ShutdownSignals {
    terminate: tokio::signal::unix::Signal,
    interrupt: tokio::signal::unix::Signal,
}

#[cfg(unix)]
impl ShutdownSignals {
    fn new() -> std::io::Result<Self> {
        use tokio::signal::unix::{SignalKind, signal};
        Ok(Self {
            terminate: signal(SignalKind::terminate())?,
            interrupt: signal(SignalKind::interrupt())?,
        })
    }

    async fn next(&mut self) {
        tokio::select! {
            _ = self.terminate.recv() => {}
            _ = self.interrupt.recv() => {}
        }
    }
}

#[cfg(not(unix))]
struct ShutdownSignals;

#[cfg(not(unix))]
impl ShutdownSignals {
    fn new() -> std::io::Result<Self> {
        Ok(Self)
    }

    async fn next(&mut self) {
        let _ = tokio::signal::ctrl_c().await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testsupport::TestDb;
    use std::path::Path;

    fn test_config(db: &TestDb, dir: &Path) -> AppConfig {
        let mut config = AppConfig::default();
        config.database.url = db.url.clone();
        config.database.max_connections = 8;
        config.database.min_connections = 1;
        config.server.bind_address = "127.0.0.1".to_owned();
        config.server.port = 0;
        config.server.shutdown_grace_secs = 5;
        config.storage.local_path = dir.join("storage");
        config.git.repos_root = dir.join("repos");
        config.security.rate_limit.enabled = false;
        config.build.poll_interval_secs = 1;
        config.observability.db_metrics_interval_secs = 1;
        config
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn the_server_boots_serves_its_endpoints_and_shuts_down_gracefully() {
        let Some(db) = TestDb::new().await else { return };
        let dir = tempfile::tempdir().unwrap();
        let config = test_config(&db, dir.path());

        let server = Server::start(config).await.expect("server starts");
        let addr = server.local_addr();
        let lifecycle = server.lifecycle();
        let running = tokio::spawn(server.wait());
        let client = reqwest::Client::new();
        let url = |path: &str| format!("http://{addr}{path}");

        let health = client.get(url("/health")).send().await.unwrap();
        assert_eq!(health.status(), 200);
        assert!(health.headers().contains_key("x-request-id"), "the middleware stack is active");
        assert!(health.headers().contains_key("x-content-type-options"));
        let body: serde_json::Value = health.json().await.unwrap();
        assert_eq!(body["status"], "healthy");

        let ready = client.get(url("/ready")).send().await.unwrap();
        assert_eq!(ready.status(), 200);
        assert_eq!(ready.json::<serde_json::Value>().await.unwrap()["ready"], true);

        let version = client.get(url("/api/version")).send().await.unwrap();
        assert_eq!(version.status(), 200, "the REST API is mounted below /api");

        let missing = client.get(url("/nothing-here")).send().await.unwrap();
        assert_eq!(missing.status(), 404);

        // the collector publishes the build info gauge after its first tick
        let mut rendered = String::new();
        for _ in 0..50 {
            rendered = client.get(url("/metrics")).send().await.unwrap().text().await.unwrap();
            if rendered.contains("onedev_build_info") {
                break;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        assert!(rendered.contains("onedev_build_info"), "{rendered}");
        assert!(rendered.contains("http_requests_total"), "requests are counted");

        // shutdown: readiness flips first, then the server stops
        lifecycle.begin_shutdown();
        let outcome = tokio::time::timeout(Duration::from_secs(20), running)
            .await
            .expect("the server stops within the grace period")
            .expect("the server task does not panic");
        outcome.expect("a requested shutdown is not an error");
        assert!(
            client.get(url("/health")).send().await.is_err(),
            "the listener is closed after shutdown"
        );
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn starting_on_a_taken_port_fails_before_touching_the_database() {
        let Some(db) = TestDb::new().await else { return };
        let dir = tempfile::tempdir().unwrap();
        let taken = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let mut config = test_config(&db, dir.path());
        config.server.port = taken.local_addr().unwrap().port();
        // an unreachable database must not matter: the port is checked first
        config.database.url = "postgres://nobody:nothing@127.0.0.1:1/none".to_owned();
        let error = Server::start(config).await.err().expect("start must fail");
        assert!(format!("{error:#}").contains("cannot listen"), "{error:#}");
        assert!(!dir.path().join("storage").exists(), "nothing was created");
    }
}
