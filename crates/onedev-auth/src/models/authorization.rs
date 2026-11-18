//! AuthorizationInfoCache - Thread-local cache for authorization info
//!
//! This is a port of OneDev's `AuthorizationInfoCache` from Java ThreadLocal
//! to Rust's `tokio::task_local!`. It caches the current subject's roles,
//! permissions, and other authorization data within the current async task.

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

use super::principal::{AccessTokenPrincipal, UserPrincipal};
use super::subject::Subject;
use chrono::{DateTime, Utc};
use onedev_core::models::role::Role;
use onedev_core::permissions::{Privilege, ProjectPrivilege};

/// Cached authorization information (port of Java AuthorizationInfo)
#[derive(Debug, Clone)]
pub struct AuthorizationInfo {
    /// The current subject
    pub subject: Subject,
    /// Cached roles on the current project
    pub project_roles: HashMap<i64, Vec<Role>>,
    /// Cached privileges for the subject
    pub privileges: Vec<Privilege>,
    /// Last cache update time
    pub last_updated: DateTime<Utc>,
    /// Whether the cache is expired
    pub expired: bool,
}

impl AuthorizationInfo {
    /// Create new authorization info for a subject
    pub fn new(subject: Subject) -> Self {
        Self {
            subject,
            project_roles: HashMap::new(),
            privileges: Vec::new(),
            last_updated: Utc::now(),
            expired: false,
        }
    }

    /// Check if cache is expired (configurable TTL)
    pub fn is_expired(&self, ttl_seconds: i64) -> bool {
        Utc::now()
            .signed_duration_since(self.last_updated)
            .num_seconds()
            > ttl_seconds
            || self.expired
    }

    /// Invalidate the cache
    pub fn invalidate(&mut self) {
        self.privileges.clear();
        self.project_roles.clear();
        self.expired = true;
        self.last_updated = Utc::now();
    }

    /// Update cache with roles for a project
    pub fn update_project_roles(&mut self, project_id: i64, roles: Vec<Role>) {
        let entry = self.project_roles.entry(project_id).or_default();
        *entry = roles;
        self.last_updated = Utc::now();
    }

    /// Get roles for a project
    pub fn get_project_roles(&self, project_id: i64) -> Option<&Vec<Role>> {
        self.project_roles.get(&project_id)
    }

    /// Check if subject has a privilege on a project
    pub fn has_privilege(&self, project_id: i64, privilege: &Privilege) -> bool {
        if let Some(roles) = self.project_roles.get(&project_id) {
            roles.iter().any(|role| role.implies(privilege))
        } else {
            false
        }
    }

    /// Get all privileges for a project
    pub fn get_project_privileges(&self, project_id: i64) -> Vec<Privilege> {
        self.project_roles
            .get(&project_id)
            .map(|roles| roles.iter().flat_map(|role| role.permissions()).collect())
            .unwrap_or_default()
    }
}

/// Thread-local storage for authorization info
/// (Port of Java ThreadLocal<AuthorizationInfo>)
tokio::task_local! {
    /// The current authorization info in the active task
    pub static AUTH_INFO: RwLock<Option<AuthorizationInfo>>;
}

/// Get the current authorization info from thread-local storage
pub async fn current_auth_info() -> Option<AuthorizationInfo> {
    AUTH_INFO
        .try_with(|info| info.blocking_read().clone())
        .unwrap_or(None)
}

/// Set the authorization info for the current task
pub async fn set_auth_info(subject: Subject) -> Result<(), AuthCacheError> {
    AUTH_INFO
        .scope(RwLock::new(Some(AuthorizationInfo::new(subject))), async {
            Ok(())
        })
        .await
}

/// With subject - run closure with a specific subject set
pub async fn with_subject<F, R>(subject: Subject, f: F) -> R
where
    F: std::future::Future<Output = R>,
{
    AUTH_INFO
        .scope(RwLock::new(Some(AuthorizationInfo::new(subject))), f)
        .await
}

/// Clear the authorization cache for the current task
pub async fn clear_auth_info() {
    let _ = AUTH_INFO.scope(RwLock::new(None), async {}).await;
}

/// Authorization cache error
#[derive(Debug, thiserror::Error)]
pub enum AuthCacheError {
    #[error("Authorization info not found in thread-local context")]
    NotFound,
    #[error("Authorization info expired")]
    Expired,
    #[error("Database error: {0}")]
    Database(#[from] onedev_db::DbError),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_authorization_info_creation() {
        let subject = Subject::Anonymous;
        let auth_info = AuthorizationInfo::new(subject);

        assert!(matches!(auth_info.subject, Subject::Anonymous));
        assert!(auth_info.privileges.is_empty());
        assert!(!auth_info.expired);
    }

    #[tokio::test]
    async fn test_authorization_info_invalidation() {
        let subject = Subject::Anonymous;
        let mut auth_info = AuthorizationInfo::new(subject);
        auth_info.expired = false;
        auth_info.privileges.push(Privilege::AccessProject);

        auth_info.invalidate();

        assert!(auth_info.expired);
        assert!(auth_info.privileges.is_empty());
        assert!(auth_info.project_roles.is_empty());
    }
}
