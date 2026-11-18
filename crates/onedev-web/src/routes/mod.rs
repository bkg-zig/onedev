//! Web UI Routes

use crate::WebState;
use axum::Router;

pub fn create_web_router(state: WebState) -> Router {
    // This is defined in lib.rs to avoid circular dependencies
    // but we keep this module for potential future expansion
    crate::create_web_router(state)
}
