//! Subject - The authenticated principal in the system
//!
//! A Subject represents who is making a request. It can be:
//! - A regular user (UserPrincipal)
//! - An access token (AccessTokenPrincipal)
//! - An anonymous/unauthenticated request (Anonymous)

use crate::models::principal::{UserPrincipal, AccessTokenPrincipal};
use onedev_core::models::user::{UserType, UserRef};
use serde::{Deserialize, Serialize};
use std::fmt;

/// The authenticated subject making a request
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Subject {
    /// Authenticated user
    User(UserPrincipal),
    /// Authenticated via access token
    AccessToken(AccessTokenPrincipal),
    /// Unauthenticated/anonymous request
    Anonymous,
}

impl Subject {
    /// Check if subject is anonymous
    pub fn is_anonymous(&self) -> bool {
        matches!(self, Subject::Anonymous)
    }

    /// Check if subject is a user
    pub fn is_user(&self) -> bool {
        matches!(self, Subject::User(_))
    }

    /// Check if subject is an access token
    pub fn is_access_token(&self) -> bool {
        matches!(self, Subject::AccessToken(_))
    }

    /// Get user ID if subject is a user
    pub fn user_id(&self) -> Option<i64> {
        match self {
            Subject::User(p) => Some(p.user_id),
            Subject::AccessToken(p) if p.impersonated_user_id.is_some() => p.impersonated_user_id,
            _ => None,
        }
    }

    /// Get the principal as a string (for logging, caching, etc.)
    pub fn principal_string(&self) -> String {
        match self {
            Subject::User(p) => format!("u:{}", p.user_id),
            Subject::AccessToken(p) => format!("a:{}", p.token_id),
            Subject::Anonymous => "anonymous".to_string(),
        }
    }

    /// Get the effective user ID (handles impersonation)
    pub fn effective_user_id(&self) -> Option<i64> {
        match self {
            Subject::User(p) => Some(p.user_id),
            Subject::AccessToken(p) => p.impersonated_user_id.or(Some(p.owner_id)),
            Subject::Anonymous => None,
        }
    }

    /// Check if subject can impersonate (has sudo scope)
    pub fn can_impersonate(&self) -> bool {
        match self {
            Subject::AccessToken(p) => p.scopes.contains(crate::models::token::TokenScope::SUDO),
            _ => false,
        }
    }

    /// Check if subject is a service account
    pub fn is_service_account(&self) -> bool {
        match self {
            Subject::User(p) => p.user_type == UserType::Service,
            Subject::AccessToken(p) => p.token_type == crate::models::token::AccessTokenType::ServiceAccount,
            Subject::Anonymous => false,
        }
    }

    /// Check if subject is system user
    pub fn is_system(&self) -> bool {
        match self {
            Subject::User(p) => p.user_id == onedev_core::models::user::SYSTEM_USER_ID,
            _ => false,
        }
    }

    /// Check if subject is root user
    pub fn is_root(&self) -> bool {
        match self {
            Subject::User(p) => p.user_id == onedev_core::models::user::ROOT_USER_ID,
            _ => false,
        }
    }

    /// Check if subject has elevated privileges (root or system)
    pub fn is_elevated(&self) -> bool {
        self.is_root() || self.is_system()
    }
}

impl fmt::Display for Subject {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.principal_string())
    }
}

impl Default for Subject {
    fn default() -> Self {
        Subject::Anonymous
    }
}

/// Thread-local storage for the current subject
///
/// Uses tokio's task-local storage to maintain subject across async boundaries
tokio::task_local! {
    pub static CURRENT_SUBJECT: Subject;
}

/// Get the current subject from task-local storage
pub fn current_subject() -> Subject {
    CURRENT_SUBJECT.try_with(|s| s.clone()).unwrap_or(Subject::Anonymous)
}

/// Set the current subject for the current task
pub fn set_current_subject(subject: Subject) {
    CURRENT_SUBJECT.scope(subject, || {});
}

/// Run a closure with a specific subject set
pub async fn with_subject<F, R>(subject: Subject, f: F) -> R
where
    F: std::future::Future<Output = R>,
{
    CURRENT_SUBJECT.scope(subject, f).await
}