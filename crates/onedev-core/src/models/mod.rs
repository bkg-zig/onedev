//! Domain Models - Core Entities

pub mod user;
pub mod project;
pub mod issue;
pub mod pull_request;
pub mod build;
pub mod workspace;
pub mod pack;
pub mod group;
pub mod role;
pub mod setting;
pub mod access_token;
pub mod email;
pub mod ssh_key;
pub mod gpg_key;
pub mod oauth;
pub mod audit;
pub mod dashboard;
pub mod code_comment;
pub mod webhook;

// Re-export commonly used types for submodules
pub use uuid::Uuid;
pub use chrono::{DateTime, Utc, NaiveDateTime};
pub use serde::{Serialize, Deserialize};
pub use anyhow::Result;
pub use thiserror::Error;
pub(crate) use std::collections::HashMap;

pub use user::*;
pub use project::*;
pub use issue::*;
pub use pull_request::*;
pub use build::*;
pub use workspace::*;
pub use pack::*;
pub use group::*;
pub use role::*;
pub use setting::*;
pub use access_token::*;
pub use email::*;
pub use ssh_key::*;
pub use gpg_key::*;
pub use oauth::*;
pub use audit::*;
pub use dashboard::*;
pub use code_comment::*;
pub use webhook::*;