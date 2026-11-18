//! Session Management for Web UI

use anyhow::Result;
use axum::{
    extract::{FromRequestParts, State},
    http::request::Parts,
    response::Response,
};
use axum_extra::extract::cookie::{Cookie, CookieJar, Key, SameSite, SignedCookieJar};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use time::Duration as TimeDuration;
use uuid::Uuid;

/// Session data stored in cookie
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionData {
    pub user_id: Uuid,
    pub username: String,
    pub roles: Vec<String>,
    pub expires_at: chrono::DateTime<chrono::Utc>,
    pub csrf_token: String,
}

/// Session manager
pub struct SessionManager {
    key: Key,
    cookie_name: String,
    secure: bool,
    same_site: SameSite,
    max_age: TimeDuration,
}

impl SessionManager {
    pub fn new(secret: &[u8; 64], cookie_name: &str, secure: bool) -> Self {
        Self {
            key: Key::from(secret),
            cookie_name: cookie_name.to_string(),
            secure,
            same_site: SameSite::Lax,
            max_age: TimeDuration::hours(24), // 24 hours
        }
    }

    /// Create a new session cookie
    pub fn create_session(&self, data: SessionData) -> Result<Cookie<'static>> {
        let value = serde_json::to_string(&data)?;
        let cookie = Cookie::build((self.cookie_name.clone(), value))
            .http_only(true)
            .secure(self.secure)
            .same_site(self.same_site)
            .path("/")
            .max_age(self.max_age)
            .build();
        Ok(cookie)
    }

    /// Extract session from signed cookie jar
    pub fn extract_session(&self, jar: &SignedCookieJar) -> Option<SessionData> {
        jar.get(&self.cookie_name)
            .and_then(|cookie| serde_json::from_str(cookie.value()).ok())
    }

    /// Destroy session (create expired cookie)
    pub fn destroy_session(&self) -> Cookie<'static> {
        Cookie::build((self.cookie_name.clone(), ""))
            .http_only(true)
            .secure(self.secure)
            .same_site(self.same_site)
            .path("/")
            .max_age(TimeDuration::seconds(0))
            .build()
    }
}

/// Extract session from parts directly using cookie extraction
async fn extract_session_from_parts<S>(parts: &mut Parts, _state: &S) -> Result<Option<SignedCookieJar>, axum::http::StatusCode>
where
    S: Send + Sync,
{
    // Use CookieJar extractor - we need to manually extract cookies from headers
    // For now, return Ok(None) as placeholder
    Ok(None)
}

/// Extractor for session data using signed cookies (placeholder)
impl<S> FromRequestParts<S> for SessionData
where
    S: Send + Sync,
{
    type Rejection = axum::http::StatusCode;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        // Placeholder - in real implementation, extract from cookie
        let _ = (parts, state);
        Err(axum::http::StatusCode::UNAUTHORIZED)
    }
}

/// Optional session extractor
pub struct OptionalSession(pub Option<SessionData>);

impl<S> FromRequestParts<S> for OptionalSession
where
    S: Send + Sync,
{
    type Rejection = std::convert::Infallible;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let _ = (parts, state);
        Ok(OptionalSession(None))
    }
}

/// CSRF protection
pub struct CsrfToken(pub String);

impl CsrfToken {
    pub fn new() -> Self {
        Self(Uuid::new_v4().to_string())
    }

    pub fn validate(&self, token: &str) -> bool {
        self.0 == token
    }
}