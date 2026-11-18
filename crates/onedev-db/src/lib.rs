//! OneDev Database Layer - SQLx Postgres Repositories & Migrations
//!
//! This crate provides the database abstraction layer for OneDev, including:
//! - SQLx models mapping to PostgreSQL tables
//! - Repository traits and implementations
//! - Database migrations
//! - Connection pool management

#![allow(clippy::new_without_default)]
#![allow(clippy::option_as_ref_deref)]
#![allow(clippy::needless_borrows_for_generic_args)]
#![allow(clippy::too_many_arguments)]
#![allow(unused_assignments)]

pub mod error;
pub mod migrations;
pub mod models;
pub mod pool;
pub mod repositories;

pub use error::*;
pub use migrations::*;
#[allow(ambiguous_glob_reexports)]
pub use models::*;
pub use pool::*;
#[allow(ambiguous_glob_reexports)]
pub use repositories::*;

pub use anyhow::Result;
pub use chrono::{DateTime, NaiveDateTime, Utc};
pub use sqlx::Row;
pub use sqlx::postgres::{PgPoolOptions, PgRow};
/// Re-export commonly used types
pub use sqlx::{Pool, Postgres, Transaction};
pub use thiserror::Error;
pub use uuid::Uuid;
