//! OneDev HTTP API + OpenAPI (utoipa)

#![allow(clippy::new_without_default)]
#![allow(clippy::option_as_ref_deref)]
#![allow(clippy::needless_borrows_for_generic_args)]
#![allow(clippy::too_many_arguments)]
#![allow(unused_assignments)]
#![allow(clippy::collapsible_if)]
#![allow(clippy::collapsible_match)]
#![allow(clippy::derivable_impls)]
#![allow(clippy::cloned_ref_to_slice_refs)]
#![allow(clippy::redundant_closure)]
#![allow(clippy::collapsible_str_replace)]
#![allow(clippy::type_complexity)]
#![allow(clippy::assign_op_pattern)]
#![allow(unused_imports)]
#![allow(unused_variables)]
#![allow(dead_code)]
#![allow(unused_doc_comments)]
#![allow(clippy::let_and_return)]
#![allow(clippy::manual_div_ceil)]
#![allow(unused_mut)]
#![allow(ambiguous_glob_reexports)]

pub mod errors;
pub mod handlers;
pub mod models;
pub mod openapi;
pub mod routes;
pub mod services;
pub mod utils;

pub use errors::*;
pub use handlers::*;
pub use models::*;
pub use routes::create_app_state;
pub use routes::*;
pub use services::*;
pub use utils::*;

pub use anyhow::Result;
pub use chrono::{DateTime, Utc};
pub use serde::{Deserialize, Serialize};
pub use thiserror::Error;
/// Re-export commonly used types
pub use uuid::Uuid;
