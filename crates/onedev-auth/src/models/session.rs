//! Session management

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// User session
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Session {
    pub id: Uuid,
    pub user_id: i64,
    pub username: String,
    pub user_agent: Option<String>,
    pub ip_address: Option<String>,
    pub created_at: DateTime<Utc>,
    pub last_accessed_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub is_remember_me: bool,
    pub two_factor_verified: bool,
}

/// Session creation input
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateSession {
    pub user_id: i64,
    pub username: String,
    pub user_agent: Option<String>,
    pub ip_address: Option<String>,
    pub remember_me: bool,
    pub two_factor_verified: bool,
}

/// Session query params
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SessionQuery {
    pub user_id: Option<i64>,
    pub active_only: bool,
    pub page: u32,
    pub per_page: u32,
}

impl Session {
    /// Default session TTL (24 hours)
    pub const DEFAULT_TTL_SECONDS: i64 = 24 * 60 * 60;

    /// Remember me session TTL (30 days)
    pub const REMEMBER_ME_TTL_SECONDS: i64 = 30 * 24 * 60 * 60;

    /// Create a new session
    pub fn new(create: CreateSession) -> Self {
        let now = Utc::now();
        let ttl = if create.remember_me {
            Self::REMEMBER_ME_TTL_SECONDS
        } else {
            Self::DEFAULT_TTL_SECONDS
        };

        Self {
            id: Uuid::new_v4(),
            user_id: create.user_id,
            username: create.username,
            user_agent: create.user_agent,
            ip_address: create.ip_address,
            created_at: now,
            last_accessed_at: now,
            expires_at: now + chrono::Duration::seconds(ttl),
            is_remember_me: create.remember_me,
            two_factor_verified: create.two_factor_verified,
        }
    }

    /// Check if session is expired
    pub fn is_expired(&self) -> bool {
        Utc::now() > self.expires_at
    }

    /// Check if session is valid (not expired)
    pub fn is_valid(&self) -> bool {
        !self.is_expired()
    }

    /// Update last accessed time
    pub fn touch(&mut self) {
        self.last_accessed_at = Utc::now();
    }

    /// Extend session expiry
    pub fn extend(&mut self, additional_seconds: i64) {
        self.expires_at = self.expires_at + chrono::Duration::seconds(additional_seconds);
    }
}
