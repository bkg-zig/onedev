//! OneDev Core Domain Models & Shared Types
//!
//! This crate contains all domain entities, value objects, enums, and shared types
//! used across the OneDev Rust rewrite.

pub mod errors;
pub mod events;
pub mod models;
pub mod permissions;
pub mod search;
pub mod utils;

pub use errors::*;
pub use events::*;
#[allow(ambiguous_glob_reexports)]
pub use models::*;
pub use permissions::*;
#[allow(ambiguous_glob_reexports)]
pub use search::*;
pub use utils::*;

pub use anyhow::Result;
pub use chrono::{DateTime, NaiveDateTime, Utc};
pub use serde::{Deserialize, Serialize};
pub use thiserror::Error;
/// Re-export commonly used types
pub use uuid::Uuid;
