//! OneDev Database Layer - SQLx Postgres Repositories & Migrations
//!
//! This crate provides the database abstraction layer for OneDev, including:
//! - SQLx models mapping to PostgreSQL tables
//! - Repository traits and implementations
//! - Database migrations
//! - Connection pool management

pub mod models;
pub mod repositories;
pub mod migrations;
pub mod pool;
pub mod error;

pub use models::*;
pub use repositories::*;
pub use migrations::*;
pub use pool::*;
pub use error::*;

/// Re-export commonly used types
pub use sqlx::{Pool, Postgres, Transaction};
pub use sqlx::postgres::{PgPoolOptions, PgRow};
pub use sqlx::Row;
pub use uuid::Uuid;
pub use chrono::{DateTime, Utc, NaiveDateTime};
pub use anyhow::Result;
pub use thiserror::Error;