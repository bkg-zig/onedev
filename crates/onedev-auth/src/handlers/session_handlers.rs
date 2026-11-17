//! Session HTTP Handlers

use axum::{
    extract::{State, Path, Query, Json},
    http::StatusCode,
};
use serde::{Deserialize, Serialize};
use crate::services::SessionService;
use crate::models::session::{Session, SessionQuery};
use crate::errors::AuthError;
use std::sync::Arc;
use uuid::Uuid;

/// Session response
#[derive(Debug, Serialize)]
pub struct SessionResponse {
    pub id: String,
    pub user_id: i64,
    pub username: String,
    pub user_agent: Option<String>,
    pub ip_address: Option<String>,
    pub created_at: String,
    pub last_accessed_at: String,
    pub expires_at: String,
    pub is_remember_me: bool,
    pub two_factor_verified: bool,
}

impl From<Session> for SessionResponse {
    fn from(session: Session) -> Self {
        Self {
            id: session.id.to_string(),
            user_id: session.user_id,
            username: session.username,
            user_agent: session.user_agent,
            ip_address: session.ip_address,
            created_at: session.created_at.to_rfc3339(),
            last_accessed_at: session.last_accessed_at.to_rfc3339(),
            expires_at: session.expires_at.to_rfc3339(),
            is_remember_me: session.is_remember_me,
            two_factor_verified: session.two_factor_verified,
        }
    }
}

/// List sessions query
#[derive(Debug, Deserialize)]
pub struct ListSessionsQuery {
    pub page: Option<u32>,
    pub per_page: Option<u32>,
    pub active_only: Option<bool>,
}

/// GET /api/auth/sessions
pub async fn list_sessions(
    State(session_service): State<Arc<SessionService>>,
    subject: crate::models::subject::Subject,
    Query(query): Query<ListSessionsQuery>,
) -> Result<Json<Vec<SessionResponse>>, AuthError> {
    let user_id = match subject {
        crate::models::subject::Subject::User(p) => p.user_id,
        _ => return Err(AuthError::Unauthorized("User authentication required".to_string())),
    };

    let session_query = SessionQuery {
        user_id: Some(user_id),
        active_only: query.active_only.unwrap_or(true),
        page: query.page.unwrap_or(0),
        per_page: query.per_page.unwrap_or(20).min(100),
    };

    let sessions = session_service.list(session_query).await?;
    let response: Vec<SessionResponse> = sessions.into_iter().map(Into::into).collect();
    
    Ok(Json(response))
}

/// GET /api/auth/sessions/{id}
pub async fn get_session(
    State(session_service): State<Arc<SessionService>>,
    subject: crate::models::subject::Subject,
    Path(id): Path<String>,
) -> Result<Json<SessionResponse>, AuthError> {
    let session_uuid = id.parse::<Uuid>()
        .map_err(|_| AuthError::Internal("Invalid session ID".to_string()))?;

    let session = session_service.find_by_id(session_uuid).await?
        .ok_or(AuthError::SessionNotFound)?;

    // Verify ownership
    let user_id = match subject {
        crate::models::subject::Subject::User(p) => p.user_id,
        _ => return Err(AuthError::Unauthorized("User authentication required".to_string())),
    };

    if session.user_id != user_id && !subject.is_elevated() {
        return Err(AuthError::Forbidden("Not authorized to view this session".to_string()));
    }

    Ok(Json(session.into()))
}

/// DELETE /api/auth/sessions/{id}
pub async fn revoke_session(
    State(session_service): State<Arc<SessionService>>,
    subject: crate::models::subject::Subject,
    Path(id): Path<String>,
) -> Result<StatusCode, AuthError> {
    let session_uuid = id.parse::<Uuid>()
        .map_err(|_| AuthError::Internal("Invalid session ID".to_string()))?;

    let session = session_service.find_by_id(session_uuid).await?
        .ok_or(AuthError::SessionNotFound)?;

    // Verify ownership
    let user_id = match subject {
        crate::models::subject::Subject::User(p) => p.user_id,
        _ => return Err(AuthError::Unauthorized("User authentication required".to_string())),
    };

    if session.user_id != user_id && !subject.is_elevated() {
        return Err(AuthError::Forbidden("Not authorized to revoke this session".to_string()));
    }

    session_service.delete(session_uuid).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// DELETE /api/auth/sessions
pub async fn revoke_all_sessions(
    State(session_service): State<Arc<SessionService>>,
    subject: crate::models::subject::Subject,
) -> Result<StatusCode, AuthError> {
    let user_id = match subject {
        crate::models::subject::Subject::User(p) => p.user_id,
        _ => return Err(AuthError::Unauthorized("User authentication required".to_string())),
    };

    session_service.delete_by_user(user_id).await?;
    Ok(StatusCode::NO_CONTENT)
}