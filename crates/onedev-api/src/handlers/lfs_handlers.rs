//! Git LFS HTTP Handlers

use crate::AppState;
use crate::errors::AuthError;
use axum::{
    body::Body,
    extract::{Json, Multipart, Path, Query, State},
    http::{HeaderMap, StatusCode, header},
    response::{IntoResponse, Response},
};
use serde::{Deserialize, Serialize};

/// LFS errors
#[derive(Debug, thiserror::Error)]
pub enum LfsError {
    #[error("Object not found")]
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

impl IntoResponse for LfsError {
    fn into_response(self) -> Response {
        let (status, message) = match self {
            LfsError::NotFound => (StatusCode::NOT_FOUND, "Object not found".to_string()),
            LfsError::Unauthorized => (
                StatusCode::UNAUTHORIZED,
                "Authentication required".to_string(),
            ),
            LfsError::Forbidden(msg) => (StatusCode::FORBIDDEN, msg),
            LfsError::BadRequest(msg) => (StatusCode::BAD_REQUEST, msg),
            LfsError::Internal(msg) => (StatusCode::INTERNAL_SERVER_ERROR, msg),
        };

        let mut response = Response::builder()
            .status(status)
            .header(header::CONTENT_TYPE, "application/vnd.git-lfs+json")
            .body(Body::from(message))
            .unwrap();

        response
    }
}

/// LFS batch request
#[derive(Debug, Deserialize)]
pub struct LfsBatchRequest {
    pub operation: String, // "download" or "upload"
    pub transfers: Vec<String>,
    pub objects: Vec<LfsObjectRequest>,
    pub hash_algo: Option<String>,
}

/// LFS object request
#[derive(Debug, Deserialize, Serialize)]
pub struct LfsObjectRequest {
    pub oid: String,
    pub size: u64,
}

/// LFS batch response
#[derive(Debug, Serialize)]
pub struct LfsBatchResponse {
    pub transfer: String,
    pub objects: Vec<LfsObjectResponse>,
}

/// LFS object response
#[derive(Debug, Serialize)]
pub struct LfsObjectResponse {
    pub oid: String,
    pub size: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub actions: Option<LfsActions>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<LfsErrorResponse>,
}

/// LFS actions (download/upload URLs)
#[derive(Debug, Serialize)]
pub struct LfsActions {
    pub download: Option<LfsAction>,
    pub upload: Option<LfsAction>,
}

/// LFS action
#[derive(Debug, Serialize)]
pub struct LfsAction {
    pub href: String,
    pub header: Option<std::collections::HashMap<String, String>>,
    pub expires_at: Option<chrono::DateTime<chrono::Utc>>,
}

/// LFS error response
#[derive(Debug, Serialize)]
pub struct LfsErrorResponse {
    pub code: u16,
    pub message: String,
}

fn git_err(err: onedev_git::GitError) -> LfsError {
    use onedev_git::GitError;
    match err {
        GitError::NotFound(_) => LfsError::NotFound,
        GitError::PermissionDenied(message) => LfsError::Forbidden(message),
        GitError::InvalidProjectPath(_)
        | GitError::InvalidRef(_)
        | GitError::InvalidRevision(_)
        | GitError::Protocol(_) => LfsError::BadRequest(err.to_string()),
        _ => LfsError::Internal(err.to_string()),
    }
}

fn lock_response(lock: &onedev_git::models::LfsLock) -> LfsLockResponse {
    LfsLockResponse {
        id: lock.id.clone(),
        path: lock.path.clone(),
        owner: LfsLockOwner { name: lock.owner.clone() },
        locked_at: lock.locked_at,
    }
}

/// POST /api/git/{project_path}/info/lfs/objects/batch
pub async fn lfs_batch(
    State(state): State<crate::AppState>,
    Path(project_path): Path<String>,
    headers: HeaderMap,
    Json(request): Json<LfsBatchRequest>,
) -> Result<Json<LfsBatchResponse>, LfsError> {
    let objects = request
        .objects
        .iter()
        .map(|object| (object.oid.clone(), object.size))
        .collect();
    // Download/upload URLs point back at this server's LFS object endpoints.
    let base = headers
        .get("x-forwarded-prefix")
        .and_then(|value| value.to_str().ok())
        .unwrap_or("")
        .to_owned();
    let batch = state
        .lfs_service
        .batch(&project_path, &request.operation, objects, |project, oid, operation| {
            format!("{base}/api/git/{project}/info/lfs/objects/{oid}?operation={operation}")
        })
        .map_err(git_err)?;

    Ok(Json(LfsBatchResponse {
        transfer: batch.transfer.to_owned(),
        objects: batch
            .objects
            .into_iter()
            .map(|object| {
                let actions = object.actions.map(|actions| LfsActions {
                    download: actions.download.map(|action| LfsAction {
                        href: action.href,
                        header: None,
                        expires_at: None,
                    }),
                    upload: actions.upload.map(|action| LfsAction {
                        href: action.href,
                        header: None,
                        expires_at: None,
                    }),
                });
                LfsObjectResponse {
                    oid: object.oid,
                    size: object.size,
                    actions,
                    error: object.error.map(|error| LfsErrorResponse {
                        code: error.code,
                        message: error.message,
                    }),
                }
            })
            .collect(),
    }))
}

/// GET /api/git/{project_path}/info/lfs/objects/{oid}
pub async fn lfs_download(
    State(state): State<crate::AppState>,
    Path((project_path, oid)): Path<(String, String)>,
) -> Result<Response, LfsError> {
    let content = state.lfs_service.download(&project_path, &oid).map_err(git_err)?;
    Ok(Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "application/octet-stream")
        .body(Body::from(content))
        .unwrap())
}

/// PUT /api/git/{project_path}/info/lfs/objects/{oid}
pub async fn lfs_upload(
    State(state): State<crate::AppState>,
    Path((project_path, oid)): Path<(String, String)>,
    body: axum::body::Bytes,
) -> Result<StatusCode, LfsError> {
    state.lfs_service.upload(&project_path, &oid, &body).map_err(git_err)?;
    Ok(StatusCode::OK)
}

/// GET /api/git/{project_path}/info/lfs/locks
pub async fn lfs_locks(
    State(state): State<crate::AppState>,
    Path(project_path): Path<String>,
    Query(query): Query<LfsLocksQuery>,
) -> Result<Json<LfsLocksResponse>, LfsError> {
    let locks = state
        .lfs_service
        .locks(&project_path, query.path.as_deref())
        .map_err(git_err)?;
    Ok(Json(LfsLocksResponse {
        locks: locks.iter().map(lock_response).collect(),
        next_cursor: None,
    }))
}

/// POST /api/git/{project_path}/info/lfs/locks
pub async fn lfs_create_lock(
    State(state): State<crate::AppState>,
    Path(project_path): Path<String>,
    crate::utils::extractors::CurrentSubject(subject): crate::utils::extractors::CurrentSubject,
    Json(request): Json<LfsCreateLockRequest>,
) -> Result<Json<LfsLockResponse>, LfsError> {
    let owner = subject
        .effective_user_id()
        .map(|user_id| format!("u-{user_id}"))
        .unwrap_or_else(|| "anonymous".to_owned());
    let lock = state
        .lfs_service
        .create_lock(&project_path, &request.path, &owner)
        .map_err(git_err)?;
    Ok(Json(lock_response(&lock)))
}

/// DELETE /api/git/{project_path}/info/lfs/locks/{id}
pub async fn lfs_unlock(
    State(state): State<crate::AppState>,
    Path((project_path, id)): Path<(String, String)>,
    crate::utils::extractors::CurrentSubject(subject): crate::utils::extractors::CurrentSubject,
) -> Result<StatusCode, LfsError> {
    let owner = subject
        .effective_user_id()
        .map(|user_id| format!("u-{user_id}"))
        .unwrap_or_else(|| "anonymous".to_owned());
    // Only the lock owner (or an admin user id 1) may unlock.
    let effective_owner = if owner == "u-1" { "admin" } else { owner.as_str() };
    state.lfs_service.unlock(&project_path, &id, effective_owner).map_err(git_err)?;
    Ok(StatusCode::NO_CONTENT)
}

/// LFS locks query
#[derive(Debug, Deserialize)]
pub struct LfsLocksQuery {
    pub path: Option<String>,
    pub id: Option<String>,
    pub limit: Option<u32>,
    pub cursor: Option<String>,
}

/// LFS locks response
#[derive(Debug, Serialize)]
pub struct LfsLocksResponse {
    pub locks: Vec<LfsLockResponse>,
    pub next_cursor: Option<String>,
}

/// LFS lock response
#[derive(Debug, Serialize)]
pub struct LfsLockResponse {
    pub id: String,
    pub path: String,
    pub owner: LfsLockOwner,
    pub locked_at: chrono::DateTime<chrono::Utc>,
}

/// LFS lock owner
#[derive(Debug, Serialize)]
pub struct LfsLockOwner {
    pub name: String,
}

/// LFS create lock request
#[derive(Debug, Deserialize)]
pub struct LfsCreateLockRequest {
    pub path: String,
}
