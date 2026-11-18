//! Background Services
//!
//! This module contains all background services that run as part of the OneDev server.

pub mod build_scheduler;
pub mod cleanup;
pub mod git_gc;
pub mod search_indexer;
pub mod webhook_dispatcher;

pub use build_scheduler::*;
pub use cleanup::*;
pub use git_gc::*;
pub use search_indexer::*;
pub use webhook_dispatcher::*;