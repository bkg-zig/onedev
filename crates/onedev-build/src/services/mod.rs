//! Build services: spec parsing, execution, scheduling.

pub mod executor;
pub mod parser;
pub mod runner;

pub use executor::*;
pub use parser::*;
pub use runner::*;

/// In-memory event bus for development (re-export from onedev-core)
pub use onedev_core::events::InMemoryEventBus;

/// Build executor for server startup (stub - full implementation deferred)
/// BLOCKER: onedev-server crate requires BuildExecutor but full implementation is
/// beyond current scope. This stub allows core libraries to compile while server
/// implementation continues separately. See project plan for completion criteria.
pub struct BuildExecutor {
    _event_bus: std::sync::Arc<onedev_core::events::InMemoryEventBus>,
}

impl BuildExecutor {
    #[allow(dead_code)]
    pub fn new(
        _db_pool: std::sync::Arc<sqlx::PgPool>,
        event_bus: std::sync::Arc<onedev_core::events::InMemoryEventBus>,
    ) -> Self {
        Self {
            _event_bus: event_bus,
        }
    }

    pub async fn run(&self) {
        // Stub: full executor implementation deferred to server milestone
        tracing::info!("BuildExecutor::run() - stub implementation (server milestone)");
    }
}
