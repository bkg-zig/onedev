//! Web UI Routes

use axum::Router;
use crate::WebState;

pub fn create_web_router(state: WebState) -> Router<WebState> {
    // This is defined in lib.rs to avoid circular dependencies
    // but we keep this module for potential future expansion
    crate::create_web_router(state)
}