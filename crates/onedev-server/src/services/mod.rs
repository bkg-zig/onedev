//! Background services of the server.
//!
//! Every service is a long-running task that owns its loop, reacts to the
//! shutdown signal and survives individual failures: iteration errors are
//! logged, counted and retried with backoff; a panicking task is restarted by
//! [`supervise`].

pub mod build_scheduler;
// pub mod cleanup;
pub mod git_cli;
// pub mod git_gc;
pub mod project_paths;
// pub mod search_indexer;
pub mod webhook_dispatcher;

use std::future::Future;
use std::sync::Arc;
use std::time::Duration;

use sqlx::postgres::PgListener;
use tokio::sync::Notify;
use tokio::task::JoinHandle;
use tracing::{error, info, warn};

use crate::lifecycle::{Lifecycle, ShutdownSignal};

/// Exponential backoff with an upper bound.
#[derive(Debug, Clone)]
pub struct Backoff {
    current: Duration,
    initial: Duration,
    max: Duration,
}

impl Backoff {
    pub fn new(initial: Duration, max: Duration) -> Self {
        Self {
            current: initial,
            initial,
            max,
        }
    }

    /// Delay to wait now; the following delay doubles up to the maximum.
    pub fn next_delay(&mut self) -> Duration {
        let delay = self.current;
        self.current = (self.current * 2).min(self.max);
        delay
    }

    pub fn reset(&mut self) {
        self.current = self.initial;
    }
}

/// Sleep for `duration` unless shutdown happens first. Returns `true` when
/// the full duration elapsed, `false` when shutdown interrupted it.
pub async fn sleep_or_shutdown(duration: Duration, shutdown: &mut ShutdownSignal) -> bool {
    tokio::select! {
        _ = tokio::time::sleep(duration) => true,
        _ = shutdown.wait() => false,
    }
}

/// Run `make(signal)` as a task and restart it when it panics.
///
/// Returns when the task finishes normally or shutdown has been requested.
pub async fn supervise<F, Fut>(name: &'static str, lifecycle: Arc<Lifecycle>, mut make: F)
where
    F: FnMut(ShutdownSignal) -> Fut,
    Fut: Future<Output = ()> + Send + 'static,
{
    let mut restart_backoff = Backoff::new(Duration::from_secs(1), Duration::from_secs(30));
    loop {
        if lifecycle.is_shutting_down() {
            return;
        }
        let mut task = tokio::spawn(make(lifecycle.signal()));
        match (&mut task).await {
            Ok(()) => {
                info!(service = name, "background service stopped");
                return;
            }
            Err(join_error) if join_error.is_panic() => {
                metrics::counter!("background_task_restarts_total", "task" => name).increment(1);
                error!(service = name, "background service panicked, restarting");
                let mut signal = lifecycle.signal();
                if !sleep_or_shutdown(restart_backoff.next_delay(), &mut signal).await {
                    return;
                }
            }
            Err(_) => return, // cancelled
        }
    }
}

/// Wake `wake` whenever Postgres delivers a NOTIFY on `channel`.
///
/// Notifications are only a latency optimisation (services also poll), so
/// connection problems are retried quietly with backoff.
pub fn spawn_notify_listener(
    pool: onedev_db::DbPool,
    channel: &'static str,
    wake: Arc<Notify>,
    mut shutdown: ShutdownSignal,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        let mut backoff = Backoff::new(Duration::from_secs(1), Duration::from_secs(30));
        'connect: loop {
            if shutdown.is_set() {
                return;
            }
            let mut listener = match PgListener::connect_with(&pool).await {
                Ok(listener) => listener,
                Err(error) => {
                    warn!(%error, channel, "cannot open LISTEN connection");
                    if !sleep_or_shutdown(backoff.next_delay(), &mut shutdown).await {
                        return;
                    }
                    continue 'connect;
                }
            };
            if let Err(error) = listener.listen(channel).await {
                warn!(%error, channel, "LISTEN failed");
                if !sleep_or_shutdown(backoff.next_delay(), &mut shutdown).await {
                    return;
                }
                continue 'connect;
            }
            backoff.reset();
            loop {
                tokio::select! {
                    received = listener.recv() => match received {
                        Ok(_) => wake.notify_one(),
                        Err(error) => {
                            warn!(%error, channel, "LISTEN connection lost, reconnecting");
                            continue 'connect;
                        }
                    },
                    _ = shutdown.wait() => return,
                }
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[test]
    fn backoff_doubles_until_the_cap_and_resets() {
        let mut backoff = Backoff::new(Duration::from_secs(1), Duration::from_secs(5));
        let delays: Vec<u64> = (0..5).map(|_| backoff.next_delay().as_secs()).collect();
        assert_eq!(delays, vec![1, 2, 4, 5, 5]);
        backoff.reset();
        assert_eq!(backoff.next_delay(), Duration::from_secs(1));
    }

    #[tokio::test]
    async fn supervisor_restarts_a_panicking_task_and_stops_on_shutdown() {
        let lifecycle = Arc::new(Lifecycle::new());
        let runs = Arc::new(AtomicUsize::new(0));
        let supervised = {
            let lifecycle = lifecycle.clone();
            let runs = runs.clone();
            tokio::spawn(async move {
                supervise("test", lifecycle, move |mut signal| {
                    let runs = runs.clone();
                    async move {
                        let attempt = runs.fetch_add(1, Ordering::SeqCst);
                        if attempt == 0 {
                            panic!("first run crashes");
                        }
                        signal.wait().await;
                    }
                })
                .await;
            })
        };
        // first attempt panics, supervisor waits ~1s, second attempt runs until shutdown
        tokio::time::timeout(Duration::from_secs(5), async {
            while runs.load(Ordering::SeqCst) < 2 {
                tokio::time::sleep(Duration::from_millis(50)).await;
            }
        })
        .await
        .expect("task must be restarted after the panic");
        lifecycle.begin_shutdown();
        tokio::time::timeout(Duration::from_secs(5), supervised)
            .await
            .expect("supervisor must stop on shutdown")
            .unwrap();
        assert_eq!(runs.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn sleep_or_shutdown_is_interrupted_by_shutdown() {
        let lifecycle = Arc::new(Lifecycle::new());
        let mut signal = lifecycle.signal();
        let started = std::time::Instant::now();
        let sleeper = tokio::spawn(async move { sleep_or_shutdown(Duration::from_secs(30), &mut signal).await });
        tokio::time::sleep(Duration::from_millis(50)).await;
        lifecycle.begin_shutdown();
        assert!(!sleeper.await.unwrap());
        assert!(started.elapsed() < Duration::from_secs(5));
    }
}
