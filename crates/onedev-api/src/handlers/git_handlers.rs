//! Git Smart HTTP Handlers — backed by `onedev-git`.

use crate::utils::extractors::OptionalSubject;
use axum::{
    body::{Body, Bytes},
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode, header},
    response::{IntoResponse, Response},
};

/// Git service errors
#[derive(Debug, thiserror::Error)]
pub enum GitHttpError {
    #[error("Repository not found")]
    NotFound,
    #[error("Authentication required")]
    Unauthorized,
    #[error("Push rejected: {0}")]
    Rejected(String),
    #[error("Permission denied: {0}")]
    Forbidden(String),
    #[error("Invalid request: {0}")]
    BadRequest(String),
    #[error("Internal error: {0}")]
    Internal(String),
}

impl From<onedev_git::GitError> for GitHttpError {
    fn from(err: onedev_git::GitError) -> Self {
        use onedev_git::GitError;
        match err {
            GitError::NotFound(_) => GitHttpError::NotFound,
            GitError::PermissionDenied(message) => GitHttpError::Forbidden(message),
            GitError::InvalidProjectPath(_)
            | GitError::InvalidRef(_)
            | GitError::InvalidRevision(_)
            | GitError::Protocol(_) => GitHttpError::BadRequest(err.to_string()),
            GitError::HookRejected(message) => GitHttpError::Rejected(message),
            _ => GitHttpError::Internal(err.to_string()),
        }
    }
}

fn git_response(response: onedev_git::GitHttpResponse) -> Response {
    Response::builder()
        .status(response.status)
        .header(header::CONTENT_TYPE, response.content_type)
        .header(header::CACHE_CONTROL, "no-cache")
        .body(Body::from(response.body))
        .unwrap()
}

impl IntoResponse for GitHttpError {
    fn into_response(self) -> Response {
        let (status, message) = match &self {
            GitHttpError::NotFound => (StatusCode::NOT_FOUND, "Repository not found".to_string()),
            GitHttpError::Unauthorized => {
                (StatusCode::UNAUTHORIZED, "Authentication required".to_string())
            }
            GitHttpError::Rejected(message) => (StatusCode::FORBIDDEN, message.clone()),
            GitHttpError::Forbidden(message) => (StatusCode::FORBIDDEN, message.clone()),
            GitHttpError::BadRequest(message) => (StatusCode::BAD_REQUEST, message.clone()),
            GitHttpError::Internal(message) => {
                (StatusCode::INTERNAL_SERVER_ERROR, message.clone())
            }
        };

        Response::builder()
            .status(status)
            .header(header::CONTENT_TYPE, "text/plain")
            .header(header::CACHE_CONTROL, "no-cache")
            .body(Body::from(message))
            .unwrap()
    }
}

/// GET /api/git/{project_path}/info/refs
///
/// Git Smart HTTP - Advertise refs for upload-pack (fetch/clone) or receive-pack (push)
pub async fn info_refs(
    State(state): State<crate::AppState>,
    Path(project_path): Path<String>,
    Query(params): Query<InfoRefsQuery>,
    _headers: HeaderMap,
) -> Result<Response, GitHttpError> {
    let response = state.git_service.info_refs(&project_path, &params.service)?;
    Ok(git_response(response))
}

/// POST /api/git/{project_path}/git-upload-pack
///
/// Git Smart HTTP - Fetch/clone operation
pub async fn git_upload_pack(
    State(state): State<crate::AppState>,
    Path(project_path): Path<String>,
    _headers: HeaderMap,
    body: Bytes,
) -> Result<Response, GitHttpError> {
    let response = state.git_service.upload_pack(&project_path, &body)?;
    Ok(git_response(response))
}

/// POST /api/git/{project_path}/git-receive-pack
///
/// Git Smart HTTP - Push operation (requires authentication so the push
/// can be attributed to an actor and checked against protection rules)
pub async fn git_receive_pack(
    State(state): State<crate::AppState>,
    Path(project_path): Path<String>,
    OptionalSubject(subject): OptionalSubject,
    _headers: HeaderMap,
    body: Bytes,
) -> Result<Response, GitHttpError> {
    let actor = subject
        .and_then(|subject| {
            let user_id = subject.effective_user_id()?;
            Some(onedev_git::models::GitActor {
                id: user_id.to_string(),
                name: subject.principal_string(),
            })
        })
        .ok_or(GitHttpError::Unauthorized)?;
    let response = state.git_service.receive_pack(&project_path, &body, actor)?;
    Ok(git_response(response))
}

/// Info/refs query parameters
#[derive(Debug, serde::Deserialize)]
pub struct InfoRefsQuery {
    pub service: String, // "git-upload-pack" or "git-receive-pack"
}
