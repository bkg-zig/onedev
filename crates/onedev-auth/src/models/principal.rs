//! Principal types for authentication

use chrono::{DateTime, Utc};
use onedev_core::models::access_token::{AccessTokenType, TokenScope};
use onedev_core::models::user::{UserRef, UserType};
use serde::{Deserialize, Serialize};

/// User principal - represents an authenticated user
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct UserPrincipal {
    pub user_id: i64,
    pub username: String,
    pub user_type: UserType,
}

impl UserPrincipal {
    pub fn new(user_id: i64, username: String, user_type: UserType) -> Self {
        Self {
            user_id,
            username,
            user_type,
        }
    }

    /// Create from a UserRef
    pub fn principal_string(&self) -> String {
        format!("u:{}", self.user_id)
    }

    pub fn from_ref(user_ref: &UserRef) -> Self {
        Self {
            user_id: user_ref.id,
            username: user_ref.name.clone(),
            user_type: user_ref.user_type,
        }
    }
}

/// Access token principal - represents authentication via access token
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct AccessTokenPrincipal {
    pub token_id: i64,
    pub token_name: String,
    pub token_type: AccessTokenType,
    pub owner_id: i64,
    pub owner_username: String,
    pub scopes: TokenScope,
    pub impersonated_user_id: Option<i64>,
    pub impersonated_username: Option<String>,
    pub project_id: Option<i64>,
    pub group_id: Option<i64>,
    pub expires_at: Option<DateTime<Utc>>,
}

impl AccessTokenPrincipal {
    pub fn new(
        token_id: i64,
        token_name: String,
        token_type: AccessTokenType,
        owner_id: i64,
        owner_username: String,
        scopes: TokenScope,
    ) -> Self {
        Self {
            token_id,
            token_name,
            token_type,
            owner_id,
            owner_username,
            scopes,
            impersonated_user_id: None,
            impersonated_username: None,
            project_id: None,
            group_id: None,
            expires_at: None,
        }
    }

    /// Check if token is expired
    pub fn principal_string(&self) -> String {
        format!("a:{}", self.token_id)
    }

    pub fn is_expired(&self) -> bool {
        self.expires_at.is_some_and(|exp| exp < Utc::now())
    }

    /// Check if token is active (not expired)
    pub fn is_active(&self) -> bool {
        !self.is_expired()
    }

    /// Check if token has a specific scope
    pub fn has_scope(&self, scope: TokenScope) -> bool {
        self.scopes.contains(scope)
    }

    /// Check if token can impersonate (has SUDO scope)
    pub fn can_impersonate(&self) -> bool {
        self.has_scope(TokenScope::SUDO) && self.impersonated_user_id.is_some()
    }

    /// Get the effective user ID (impersonated or owner)
    pub fn effective_user_id(&self) -> i64 {
        self.impersonated_user_id.unwrap_or(self.owner_id)
    }
}

/// Principal trait for common operations
pub trait Principal {
    fn principal_string(&self) -> String;
    fn user_id(&self) -> Option<i64>;
    fn is_anonymous(&self) -> bool;
}

impl Principal for UserPrincipal {
    fn principal_string(&self) -> String {
        format!("u:{}", self.user_id)
    }

    fn user_id(&self) -> Option<i64> {
        Some(self.user_id)
    }

    fn is_anonymous(&self) -> bool {
        false
    }
}

impl Principal for AccessTokenPrincipal {
    fn principal_string(&self) -> String {
        format!("a:{}", self.token_id)
    }

    fn user_id(&self) -> Option<i64> {
        Some(self.effective_user_id())
    }

    fn is_anonymous(&self) -> bool {
        false
    }
}
