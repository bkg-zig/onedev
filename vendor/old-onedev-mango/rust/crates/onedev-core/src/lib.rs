//! Framework-independent domain model for the Rust rewrite of OneDev.
//!
//! The Java implementation remains the behavioural reference until every
//! feature listed in `rust/PARITY.md` has reached parity. This crate only
//! contains stable domain contracts and does not depend on HTTP or database
//! frameworks.

pub mod auth;
pub mod build;
pub mod error;
pub mod issue;
pub mod pack;
pub mod project;
pub mod pull_request;
pub mod query;
pub mod user;

pub use error::{CoreError, CoreResult};
