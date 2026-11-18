//! Session Management for Web UI

use axum::extract::FromRequestParts;
use axum::http::HeaderMap;
use axum_extra::extract::cookie::{Cookie, CookieJar, Key, SameSite};
use serde::{Deserialize, Serialize};
use time::Duration as TimeDuration;
use uuid::Uuid;

/// Extract cookie value by name from the Cookie header
fn get_cookie_value(headers: &HeaderMap, name: &str) -> Option<String> {
    headers
        .get(axum::http::header::COOKIE)
        .and_then(|value| value.to_str().ok())
        .and_then(|cookie_str| {
            cookie_str.split(';').find_map(|part| {
                let part = part.trim();
                let mut iter = part.splitn(2, '=');
                let cookie_name = iter.next()?;
                if cookie_name == name {
                    iter.next().map(|s| s.to_owned())
                } else {
                    None
                }
            })
        })
}

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
        // secret is used for deriving key material in production; kept for API compatibility
        debug_assert!(!secret.is_empty());
        let key = Key::from(secret);
        Self {
            key,
            cookie_name: cookie_name.to_string(),
            secure,
            same_site: SameSite::Lax,
            max_age: TimeDuration::hours(24), // 24 hours
        }
    }

    /// Create a new session cookie
    pub fn create_session(&self, data: SessionData) -> anyhow::Result<Cookie<'static>> {
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

    /// Extract session from cookie value
    pub fn extract_session(&self, jar: &CookieJar) -> Option<SessionData> {
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

/// Extractor for session data using cookies
impl<S> FromRequestParts<S> for SessionData
where
    S: Send + Sync,
{
    type Rejection = axum::http::StatusCode;

    async fn from_request_parts(
        parts: &mut axum::http::request::Parts,
        _state: &S,
    ) -> Result<Self, Self::Rejection> {
        if let Some(session_value) = get_cookie_value(&parts.headers, "session")
            && let Ok(session_data) = serde_json::from_str(&session_value)
        {
            return Ok(session_data);
        }
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

    async fn from_request_parts(
        parts: &mut axum::http::request::Parts,
        _state: &S,
    ) -> Result<Self, Self::Rejection> {
        if let Some(session_value) = get_cookie_value(&parts.headers, "session") {
            let session = serde_json::from_str(&session_value).ok();
            Ok(OptionalSession(session))
        } else {
            Ok(OptionalSession(None))
        }
    }
}

/// CSRF protection
#[derive(Default)]
pub struct CsrfToken(pub String);

impl CsrfToken {
    pub fn new() -> Self {
        Self(Uuid::new_v4().to_string())
    }

    pub fn validate(&self, token: &str) -> bool {
        self.0 == token
    }
}
