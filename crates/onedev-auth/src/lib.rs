//! OneDev Authentication & Authorization - Subject, Permissions, Sessions, Tokens

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
#![allow(ambiguous_glob_reexports)]

pub mod errors;
pub mod extractors;
pub mod handlers;
pub mod middleware;
pub mod models;
pub mod repositories;
pub mod router;
pub mod services;
pub mod utils;

pub use errors::*;
pub use extractors::*;
pub use handlers::*;
pub use middleware::*;
pub use models::*;
pub use router::*;
pub use services::*;
pub use utils::*;

pub use anyhow::Result;
pub use chrono::{DateTime, Utc};
pub use serde::{Deserialize, Serialize};
pub use thiserror::Error;
/// Re-export commonly used types
pub use uuid::Uuid;
