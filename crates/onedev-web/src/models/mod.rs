//! Web UI Models

pub mod auth;
pub mod dashboard;
pub mod git;
pub mod issues;
pub mod merge_requests;
pub mod projects;
pub mod builds;
pub mod profile;
pub mod admin;

pub use auth::*;
pub use dashboard::*;
pub use git::*;
pub use issues::*;
pub use merge_requests::*;
pub use projects::*;
pub use builds::*;
pub use profile::*;
pub use admin::*;