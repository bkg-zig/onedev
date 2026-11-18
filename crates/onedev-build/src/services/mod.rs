//! Build services: spec parsing, execution, scheduling.

pub mod executor;
pub mod parser;
pub mod runner;

pub use executor::*;
pub use parser::*;
pub use runner::*;

/// In-memory event bus for development (re-export from onedev-core)
pub use onedev_core::events::InMemoryEventBus;