//! Authorization Service Wrapper

use onedev_auth::{
    AuthorizationService as InnerAuthzService,
    AuthzError as InnerAuthzError,
    PermissionCheck,
};
use onedev_db::DbPool;
use std::sync::Arc;
use onedev_auth::models::principal::ProjectPermissionType;

/// Wrapper around onedev-auth's AuthorizationService
pub struct AuthorizationService {
    inner: Arc<InnerAuthzService>,
}

impl AuthorizationService {
    pub fn new(db_pool: DbPool) -> Self {
        Self {
            inner: Arc::new(InnerAuthzService::new(db_pool)),
        }
    }

    pub fn inner(&self) -> &InnerAuthzService {
        &self.inner
    }

    pub fn clone_inner(&self) -> Arc<InnerAuthzService> {
        self.inner.clone()
    }

    /// Check project permission
    pub async fn check_project_permission(
        &self,
        subject: &onedev_auth::Subject,
        project_id: i64,
        permission: ProjectPermissionType,
    ) -> Result<PermissionCheck, InnerAuthzError> {
        self.inner.check_project_permission(subject, project_id, permission).await
    }

    /// Check system permission
    pub async fn check_system_permission(
        &self,
        subject: &onedev_auth::Subject,
        permission: onedev_auth::models::principal::SystemPermission,
    ) -> Result<PermissionCheck, InnerAuthzError> {
        self.inner.check_system_permission(subject, permission).await
    }
}

// Re-export types
pub use onedev_auth::{
    AuthorizationService as InnerAuthorizationService,
    AuthzError,
    PermissionCheck,
};