//! Authentication Service Wrapper

use onedev_auth::services::auth_service::{
    AuthConfig, AuthError as InnerAuthError, AuthService as InnerAuthService,
};
use onedev_db::DbPool;
use std::sync::Arc;

/// Wrapper around onedev-auth's AuthService
pub struct AuthService {
    inner: Arc<InnerAuthService>,
}

impl AuthService {
    pub fn new(config: AuthConfig, db_pool: DbPool) -> Result<Self, InnerAuthError> {
        let inner = InnerAuthService::new(config, db_pool)?;
        Ok(Self {
            inner: Arc::new(inner),
        })
    }

    pub fn inner(&self) -> &InnerAuthService {
        &self.inner
    }

    pub fn clone_inner(&self) -> Arc<InnerAuthService> {
        self.inner.clone()
    }
}

// Re-export types
pub use onedev_auth::models::principal::{AccessTokenPrincipal, UserPrincipal};
pub use onedev_auth::models::subject::Subject;
pub use onedev_auth::services::auth_service::AuthResult;
