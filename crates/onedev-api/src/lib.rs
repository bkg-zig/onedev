//! OneDev HTTP API + OpenAPI (utoipa)

pub mod models;
pub mod services;
pub mod handlers;
pub mod errors;
pub mod utils;
pub mod routes;
pub mod openapi;

pub use models::*;
pub use services::*;
pub use handlers::*;
pub use errors::*;
pub use utils::*;
pub use routes::*;

/// Re-export commonly used types
pub use uuid::Uuid;
pub use chrono::{DateTime, Utc};
pub use serde::{Serialize, Deserialize};
pub use anyhow::Result;
pub use thiserror::Error;