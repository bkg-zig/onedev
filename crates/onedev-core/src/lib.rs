//! OneDev Core Domain Models & Shared Types
//!
//! This crate contains all domain entities, value objects, enums, and shared types
//! used across the OneDev Rust rewrite.

pub mod models;
pub mod permissions;
pub mod search;
pub mod events;
pub mod errors;
pub mod utils;

pub use models::*;
pub use permissions::*;
pub use search::*;
pub use events::*;
pub use errors::*;
pub use utils::*;

/// Re-export commonly used types
pub use uuid::Uuid;
pub use chrono::{DateTime, Utc, NaiveDateTime};
pub use serde::{Serialize, Deserialize};
pub use anyhow::Result;
pub use thiserror::Error;