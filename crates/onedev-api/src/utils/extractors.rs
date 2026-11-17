//! Custom Axum Extractors

use axum::{
    extract::{FromRequestParts, Request},
    http::request::Parts,
    async_trait,
};
use crate::errors::{AuthError, ApiError};
use onedev_auth::Subject;

/// Extract authenticated subject from request extensions
#[derive(Debug, Clone)]
pub struct CurrentSubject(pub Subject);

#[async_trait]
impl<S> FromRequestParts<S> for CurrentSubject
where
    S: Send + Sync,
{
    type Rejection = AuthError;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        parts.extensions
            .get::<Subject>()
            .cloned()
            .map(CurrentSubject)
            .ok_or(AuthError::Unauthorized("No subject in request".to_string()))
    }
}

/// Extract optional subject (for public endpoints)
#[derive(Debug, Clone)]
pub struct OptionalSubject(pub Option<Subject>);

#[async_trait]
impl<S> FromRequestParts<S> for OptionalSubject
where
    S: Send + Sync,
{
    type Rejection = AuthError;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        Ok(OptionalSubject(parts.extensions.get::<Subject>().cloned()))
    }
}

/// Require authenticated user (not access token)
pub async fn require_user(
    CurrentSubject(subject): CurrentSubject,
) -> Result<onedev_auth::UserPrincipal, AuthError> {
    match subject {
        Subject::User(p) => Ok(p),
        _ => Err(AuthError::Unauthorized("User authentication required".to_string())),
    }
}

/// Require admin access
pub async fn require_admin(
    CurrentSubject(subject): CurrentSubject,
) -> Result<Subject, AuthError> {
    if subject.is_elevated() {
        Ok(subject)
    } else {
        Err(AuthError::Forbidden("Admin access required".to_string()))
    }
}

/// Pagination extractor
#[derive(Debug, Clone)]
pub struct Pagination(pub crate::models::pagination::OffsetPagination);

#[async_trait]
impl<S> FromRequestParts<S> for Pagination
where
    S: Send + Sync,
{
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        // Query parameters are already parsed by axum's Query extractor
        // This is a placeholder - actual implementation would use Query<>
        Ok(Pagination(crate::models::pagination::OffsetPagination::default()))
    }
}