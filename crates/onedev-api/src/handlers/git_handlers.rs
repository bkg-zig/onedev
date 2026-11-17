//! Git Smart HTTP Handlers

use axum::{
    extract::{State, Path, Query},
    http::{HeaderMap, StatusCode, header},
    response::{IntoResponse, Response},
    body::{Body, Bytes},
};
use crate::errors::AuthError;
use crate::AppState;
use std::sync::Arc;

/// Git service errors
#[derive(Debug, thiserror::Error)]
pub enum GitHttpError {
    #[error("Repository not found")]
    NotFound,
    #[error("Authentication required")]
    Unauthorized,
    #[error("Permission denied: {0}")]
    Forbidden(String),
    #[error("Invalid request: {0}")]
    BadRequest(String),
    #[error("Internal error: {0}")]
    Internal(String),
}

impl IntoResponse for GitHttpError {
    fn into_response(self) -> Response {
        let (status, message) = match self {
            GitHttpError::NotFound => (StatusCode::NOT_FOUND, "Repository not found".to_string()),
            GitHttpError::Unauthorized => (StatusCode::UNAUTHORIZED, "Authentication required".to_string()),
            GitHttpError::Forbidden(msg) => (StatusCode::FORBIDDEN, msg),
            GitHttpError::BadRequest(msg) => (StatusCode::BAD_REQUEST, msg),
            GitHttpError::Internal(msg) => (StatusCode::INTERNAL_SERVER_ERROR, msg),
        };
        
        let mut response = Response::builder()
            .status(status)
            .header(header::CONTENT_TYPE, "text/plain")
            .body(Body::from(message))
            .unwrap();
        
        // Add Git HTTP headers
        response.headers_mut().insert(
            header::CACHE_CONTROL,
            "no-cache".parse().unwrap(),
        );
        
        response
    }
}

/// GET /api/git/{project_path}/info/refs
/// 
/// Git Smart HTTP - Advertise refs for upload-pack (fetch/clone) or receive-pack (push)
pub async fn info_refs(
    State(_state): State<crate::AppState>,
    Path(_project_path): Path<String>,
    Query(_params): Query<InfoRefsQuery>,
    headers: HeaderMap,
) -> Result<Response, GitHttpError> {
    // TODO: Implement Git Smart HTTP info/refs
    Err(GitHttpError::Internal("Not implemented".to_string()))
}

/// POST /api/git/{project_path}/git-upload-pack
/// 
/// Git Smart HTTP - Fetch/clone operation
pub async fn git_upload_pack(
    State(_state): State<crate::AppState>,
    Path(_project_path): Path<String>,
    headers: HeaderMap,
    body: axum::body::Bytes,
) -> Result<Response, GitHttpError> {
    // TODO: Implement Git Smart HTTP upload-pack
    Err(GitHttpError::Internal("Not implemented".to_string()))
}

/// POST /api/git/{project_path}/git-receive-pack
/// 
/// Git Smart HTTP - Push operation
pub async fn git_receive_pack(
    State(_state): State<crate::AppState>,
    Path(_project_path): Path<String>,
    headers: HeaderMap,
    body: axum::body::Bytes,
) -> Result<Response, GitHttpError> {
    // TODO: Implement Git Smart HTTP receive-pack
    Err(GitHttpError::Internal("Not implemented".to_string()))
}

/// Info/refs query parameters
#[derive(Debug, serde::Deserialize)]
pub struct InfoRefsQuery {
    pub service: String, // "git-upload-pack" or "git-receive-pack"
}