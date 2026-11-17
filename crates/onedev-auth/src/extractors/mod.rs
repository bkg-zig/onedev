//! Custom Extractors for Authentication & Authorization

use axum::{
    extract::{FromRequestParts, Request, State},
    http::{header, HeaderMap, StatusCode, request::Parts},
    async_trait,
};
use axum_extra::extract::CookieJar;
use crate::models::subject::{Subject, CURRENT_SUBJECT};
use crate::models::principal::{UserPrincipal, AccessTokenPrincipal};
use crate::models::token::{JwtService, Claims, TokenType};
use crate::services::AuthService;
use crate::errors::AuthError;
use std::sync::Arc;
use tracing::debug;

/// Extractor for validated JWT claims
#[derive(Debug, Clone)]
pub struct JwtClaims(pub Claims);

#[async_trait]
impl<S> FromRequestParts<S> for JwtClaims
where
    S: Send + Sync,
{
    type Rejection = AuthError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        // Try to get from extensions first
        if let Some(claims) = parts.extensions.get::<Claims>() {
            return Ok(JwtClaims(claims.clone()));
        }

        // Try to extract from state (if AuthService is in state)
        // This is a fallback - normally the middleware sets this
        Err(AuthError::Unauthorized("No JWT claims found".to_string()))
    }
}

/// Extractor that requires a valid access token (JWT or PAT)
#[derive(Debug, Clone)]
pub struct RequireAuth(pub Subject);

#[async_trait]
impl<S> FromRequestParts<S> for RequireAuth
where
    S: Send + Sync,
{
    type Rejection = AuthError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        if let Some(subject) = parts.extensions.get::<Subject>() {
            match subject {
                Subject::User(_) | Subject::AccessToken(_) => {
                    return Ok(RequireAuth(subject.clone()));
                }
                Subject::Anonymous => {}
            }
        }
        Err(AuthError::Unauthorized("Authentication required".to_string()))
    }
}

/// Extractor that requires a user (not access token)
#[derive(Debug, Clone)]
pub struct RequireUser(pub UserPrincipal);

#[async_trait]
impl<S> FromRequestParts<S> for RequireUser
where
    S: Send + Sync,
{
    type Rejection = AuthError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        if let Some(subject) = parts.extensions.get::<Subject>() {
            if let Subject::User(p) = subject {
                return Ok(RequireUser(p.clone()));
            }
        }
        Err(AuthError::Unauthorized("User authentication required".to_string()))
    }
}

/// Extractor that requires admin/root access
#[derive(Debug, Clone)]
pub struct RequireAdmin(pub Subject);

#[async_trait]
impl<S> FromRequestParts<S> for RequireAdmin
where
    S: Send + Sync,
{
    type Rejection = AuthError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        if let Some(subject) = parts.extensions.get::<Subject>() {
            if subject.is_elevated() {
                return Ok(RequireAdmin(subject.clone()));
            }
        }
        Err(AuthError::Forbidden("Admin access required".to_string()))
    }
}

/// Extractor for project permission
#[derive(Debug, Clone)]
pub struct RequireProjectPermission {
    pub permission: crate::models::principal::ProjectPermissionType,
    pub project_id: i64,
    pub subject: Subject,
}

impl RequireProjectPermission {
    pub fn new(permission: crate::models::principal::ProjectPermissionType, project_id: i64) -> Self {
        Self {
            permission,
            project_id,
            subject: Subject::Anonymous,
        }
    }
}

#[async_trait]
impl<S> FromRequestParts<S> for RequireProjectPermission
where
    S: Send + Sync,
{
    type Rejection = AuthError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let subject = parts.extensions.get::<Subject>().cloned()
            .unwrap_or(Subject::Anonymous);
        
        // Note: Actual permission check would require AuthorizationService from state
        // This extractor mainly provides the subject for the handler to check
        
        Ok(RequireProjectPermission {
            permission: crate::models::principal::ProjectPermissionType::ViewProject,
            project_id: 0,
            subject,
        })
    }
}

/// Extractor for optional authentication (doesn't reject)
#[derive(Debug, Clone)]
pub struct OptionalAuth(pub Option<Subject>);

#[async_trait]
impl<S> FromRequestParts<S> for OptionalAuth
where
    S: Send + Sync,
{
    type Rejection = AuthError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        Ok(OptionalAuth(parts.extensions.get::<Subject>().cloned()))
    }
}

/// Extractor for current user ID (convenience)
#[derive(Debug, Clone)]
pub struct CurrentUserId(pub Option<i64>);

#[async_trait]
impl<S> FromRequestParts<S> for CurrentUserId
where
    S: Send + Sync,
{
    type Rejection = AuthError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        if let Some(subject) = parts.extensions.get::<Subject>() {
            Ok(CurrentUserId(subject.effective_user_id()))
        } else {
            Ok(CurrentUserId(None))
        }
    }
}

/// Macro for easy permission checking in handlers
#[macro_export]
macro_rules! require_permission {
    ($subject:expr, $authz:expr, $project_id:expr, $perm:expr) => {{
        use $crate::services::AuthorizationService;
        use $crate::models::principal::ProjectPermissionType;
        
        let result = $authz.check_project_permission(&$subject, $project_id, $perm).await?;
        if result != $crate::services::authorization_service::PermissionCheck::Granted {
            return Err($crate::errors::AuthError::InsufficientPermissions(
                format!("Missing permission: {}", $perm.name())
            ));
        }
    }};
}

/// Macro for requiring user authentication
#[macro_export]
macro_rules! require_user {
    ($subject:expr) => {{
        match &$subject {
            $crate::models::subject::Subject::User(p) => p.clone(),
            _ => return Err($crate::errors::AuthError::Unauthorized("User authentication required".to_string())),
        }
    }};
}

/// Macro for requiring admin access
#[macro_export]
macro_rules! require_admin {
    ($subject:expr) => {{
        if !$subject.is_elevated() {
            return Err($crate::errors::AuthError::Forbidden("Admin access required".to_string()));
        }
    }};
}