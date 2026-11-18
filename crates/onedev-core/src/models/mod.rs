//! Domain Models - Core Entities

pub mod access_token;
pub mod audit;
pub mod build;
pub mod code_comment;
pub mod dashboard;
pub mod email;
pub mod gpg_key;
pub mod group;
pub mod issue;
pub mod oauth;
pub mod pack;
pub mod project;
pub mod pull_request;
pub mod role;
pub mod setting;
pub mod ssh_key;
pub mod user;
pub mod webhook;
pub mod workspace;

// Re-export commonly used types for submodules
pub use anyhow::Result;
pub use chrono::{DateTime, NaiveDateTime, Utc};
pub use serde::{Deserialize, Serialize};
pub(crate) use std::collections::HashMap;
pub use thiserror::Error;
pub use uuid::Uuid;

// Explicit re-exports for conflicting types to disambiguate
pub use crate::search::EntityType as SearchEntityType;
pub use access_token::AccessTokenAuthorization as AccessTokenAuth;
pub use audit::EntityType as AuditEntityType;
pub use issue::MergeRequestState as IssueMergeRequestState;
pub use pull_request::MergeRequestState as PullRequestMergeRequestState;
pub use role::AccessTokenAuthorization as RoleAccessTokenAuth;

// Glob re-exports (non-conflicting types) - allow ambiguous for known conflicts
#[allow(ambiguous_glob_reexports)]
pub use access_token::*;
pub use audit::*;
pub use build::*;
pub use code_comment::*;
pub use dashboard::*;
pub use email::*;
pub use gpg_key::*;
pub use group::*;
#[allow(ambiguous_glob_reexports)]
pub use issue::*;
pub use oauth::*;
pub use pack::*;
pub use project::*;
#[allow(ambiguous_glob_reexports)]
pub use pull_request::*;
#[allow(ambiguous_glob_reexports)]
pub use role::*;
pub use setting::*;
pub use ssh_key::*;
pub use user::*;
pub use webhook::*;
pub use workspace::*;
