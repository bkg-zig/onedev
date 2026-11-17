//! Authorization Service - Permission checking and access control

use crate::models::subject::Subject;
use crate::models::principal::{UserPrincipal, AccessTokenPrincipal};
use onedev_core::models::user::UserRef;
use onedev_core::models::project::ProjectRef;
use onedev_core::models::role::{Role, PermissionSet};
use onedev_core::models::group::GroupRef;
use onedev_core::permissions::{Permission, ProjectPermissionType, SystemPermission, BasePermission};
use onedev_db::repositories::{UserRepository, ProjectRepository, RoleRepository, MembershipRepository, GroupRepository, UserAuthorizationRepository, Repository};
use onedev_db::DbPool;
use onedev_db::DbError;
use dashmap::DashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use thiserror::Error;

/// Authorization errors
#[derive(Debug, Error)]
pub enum AuthzError {
    #[error("Permission denied: {0}")]
    PermissionDenied(String),
    #[error("Resource not found")]
    NotFound,
    #[error("Database error: {0}")]
    Database(#[from] DbError),
    #[error("Internal error: {0}")]
    Internal(String),
}

/// Permission check result
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PermissionCheck {
    Granted,
    Denied(String),
}

/// Cached authorization info
#[derive(Debug, Clone)]
struct CachedAuthzInfo {
    permissions: Vec<ProjectPermissionType>,
    roles: Vec<Role>,
    expires_at: std::time::Instant,
}

/// Authorization service with caching
pub struct AuthorizationService {
    user_repo: UserRepository,
    project_repo: ProjectRepository,
    role_repo: RoleRepository,
    membership_repo: MembershipRepository,
    group_repo: GroupRepository,
    db_pool: DbPool,
    cache: Arc<DashMap<String, CachedAuthzInfo>>,
    cache_ttl: Duration,
}

impl AuthorizationService {
    /// Create a new authorization service
    pub fn new(db_pool: DbPool) -> Self {
        Self {
            user_repo: UserRepository::new(),
            project_repo: ProjectRepository::new(),
            role_repo: RoleRepository::new(),
            membership_repo: MembershipRepository::new(),
            group_repo: GroupRepository::new(),
            db_pool,
            cache: Arc::new(DashMap::new()),
            cache_ttl: Duration::from_secs(5 * 60), // 5 minutes
        }
    }

    /// Check if subject has a project permission
    pub async fn check_project_permission(
        &self,
        subject: &Subject,
        project_id: i64,
        permission: ProjectPermissionType,
    ) -> Result<PermissionCheck, AuthzError> {
        let permissions = self.get_subject_project_permissions(subject, project_id).await?;
        
        if permissions.iter().any(|p| p.implies(&permission)) {
            Ok(PermissionCheck::Granted)
        } else {
            Ok(PermissionCheck::Denied(format!("Missing permission: {}", permission.name())))
        }
    }

    /// Check if subject has a system permission
    pub async fn check_system_permission(
        &self,
        subject: &Subject,
        permission: SystemPermission,
    ) -> Result<PermissionCheck, AuthzError> {
        // System permissions only for users (not tokens)
        let user_id = match subject {
            Subject::User(p) => p.user_id,
            Subject::AccessToken(p) if p.can_impersonate() => p.effective_user_id(),
            _ => return Ok(PermissionCheck::Denied("System permissions require user authentication".to_string())),
        };

        // Check if user is admin
        let is_admin = self.is_system_admin(user_id).await?;
        
        if is_admin || permission == SystemPermission::ViewAllProjects || permission == SystemPermission::ViewAllGroups {
            Ok(PermissionCheck::Granted)
        } else {
            Ok(PermissionCheck::Denied(format!("Missing system permission: {}", permission.name())))
        }
    }

    /// Check multiple project permissions at once
    pub async fn check_project_permissions(
        &self,
        subject: &Subject,
        project_id: i64,
        permissions: &[ProjectPermissionType],
    ) -> Result<PermissionCheck, AuthzError> {
        let subject_perms = self.get_subject_project_permissions(subject, project_id).await?;
        
        for required in permissions {
            if !subject_perms.iter().any(|p| p.implies(required)) {
                return Ok(PermissionCheck::Denied(format!("Missing permission: {}", required.name())));
            }
        }
        
        Ok(PermissionCheck::Granted)
    }

    /// Check if subject can access a project (any permission)
    pub async fn can_access_project(&self, subject: &Subject, project_id: i64) -> Result<bool, AuthzError> {
        let result = self.check_project_permission(subject, project_id, ProjectPermissionType::ViewProject).await?;
        Ok(result == PermissionCheck::Granted)
    }

    /// Check if subject can read code in a project
    pub async fn can_read_code(&self, subject: &Subject, project_id: i64) -> Result<bool, AuthzError> {
        let result = self.check_project_permission(subject, project_id, ProjectPermissionType::ReadCode).await?;
        Ok(result == PermissionCheck::Granted)
    }

    /// Check if subject can write code in a project
    pub async fn can_write_code(&self, subject: &Subject, project_id: i64) -> Result<bool, AuthzError> {
        let result = self.check_project_permission(subject, project_id, ProjectPermissionType::WriteCode).await?;
        Ok(result == PermissionCheck::Granted)
    }

    /// Check if subject can manage a project
    pub async fn can_manage_project(&self, subject: &Subject, project_id: i64) -> Result<bool, AuthzError> {
        let result = self.check_project_permission(subject, project_id, ProjectPermissionType::ManageProject).await?;
        Ok(result == PermissionCheck::Granted)
    }

    /// Check if subject is project admin
    pub async fn is_project_admin(&self, subject: &Subject, project_id: i64) -> Result<bool, AuthzError> {
        let result = self.check_project_permission(subject, project_id, ProjectPermissionType::AdminProject).await?;
        Ok(result == PermissionCheck::Granted)
    }

    /// Get all project permissions for a subject
    async fn get_subject_project_permissions(
        &self,
        subject: &Subject,
        project_id: i64,
    ) -> Result<Vec<ProjectPermissionType>, AuthzError> {
        let cache_key = format!("{}:{}", subject.principal_string(), project_id);
        
        // Check cache
        if let Some(cached) = self.cache.get(&cache_key) {
            if cached.expires_at > std::time::Instant::now() {
                return Ok(cached.permissions.clone());
            }
        }

        let mut permissions = Vec::new();

        match subject {
            Subject::User(principal) => {
                // TODO: Implement proper authorization lookup
                // For now, return empty permissions - this should be implemented
                // when the authorization repositories are complete
                let _ = principal;
            }
            Subject::AccessToken(principal) => {
                // TODO: Implement proper authorization lookup
                let _ = principal;
            }
            Subject::Anonymous => {
                // TODO: Implement anonymous access check
            }
        }

        // Cache the result
        self.cache.insert(cache_key, CachedAuthzInfo {
            permissions: permissions.clone(),
            roles: Vec::new(),
            expires_at: std::time::Instant::now() + self.cache_ttl,
        });

        Ok(permissions)
    }

    /// Convert role permission set to project permission types
    fn role_permissions_to_project(&self, perm_set: &PermissionSet) -> Vec<ProjectPermissionType> {
        let mut perms = Vec::new();
        
        // Map PermissionSet flags to ProjectPermissionType
        // This is a simplified mapping - in reality you'd have a complete mapping
        if perm_set.contains(PermissionSet::VIEW_PROJECT) {
            perms.push(ProjectPermissionType::ViewProject);
        }
        if perm_set.contains(PermissionSet::READ_CODE) {
            perms.push(ProjectPermissionType::ReadCode);
        }
        if perm_set.contains(PermissionSet::WRITE_CODE) {
            perms.push(ProjectPermissionType::WriteCode);
            perms.push(ProjectPermissionType::PushCode);
        }
        if perm_set.contains(PermissionSet::MANAGE_PROJECT) {
            perms.push(ProjectPermissionType::ManageProject);
        }
        if perm_set.contains(PermissionSet::ADMIN_PROJECT) {
            perms.push(ProjectPermissionType::AdminProject);
        }
        // ... add more mappings as needed
        
        perms
    }

    /// Check if user is system admin
    async fn is_system_admin(&self, user_id: i64) -> Result<bool, AuthzError> {
        let user = self.user_repo.find_by_id(&self.db_pool, user_id).await?;
        Ok(user.map(|u| u.id == onedev_core::models::user::ROOT_USER_ID || u.id == onedev_core::models::user::SYSTEM_USER_ID).unwrap_or(false))
    }

    /// Get user's role in a project
    pub async fn get_user_project_role(&self, user_id: i64, project_id: i64) -> Result<Option<Role>, AuthzError> {
        let repo = UserAuthorizationRepository::new();
        let authz = repo
            .find_by_user_and_project(&self.db_pool, user_id, project_id)
            .await
            .map_err(|e| AuthzError::Internal(e.to_string()))?;

        Ok(authz.map(|row| {
            let permissions = match row.role {
                onedev_db::models::authorization::DbProjectRole::Guest => PermissionSet::VIEW_PROJECT,
                onedev_db::models::authorization::DbProjectRole::Reporter => {
                    PermissionSet::VIEW_PROJECT | PermissionSet::READ_CODE | PermissionSet::VIEW_ISSUES
                }
                onedev_db::models::authorization::DbProjectRole::Developer => {
                    PermissionSet::VIEW_PROJECT | PermissionSet::READ_CODE | PermissionSet::WRITE_CODE
                        | PermissionSet::VIEW_ISSUES | PermissionSet::CREATE_ISSUE
                }
                onedev_db::models::authorization::DbProjectRole::Maintainer => {
                    PermissionSet::VIEW_PROJECT | PermissionSet::READ_CODE | PermissionSet::WRITE_CODE
                        | PermissionSet::MANAGE_PROJECT | PermissionSet::VIEW_ISSUES
                }
                onedev_db::models::authorization::DbProjectRole::Owner => PermissionSet::all(),
            };
            Role {
                id: row.id,
                uuid: row.uuid,
                name: format!("{:?}", row.role),
                description: None,
                is_builtin: true,
                is_system: false,
                permissions,
                project_id: Some(row.project_id),
                group_id: None,
                base_authorizations: Vec::new(),
                created_at: row.created_at,
                updated_at: row.updated_at,
            }
        }))
    }

    /// Get effective permissions for a subject on a project (for UI)
    pub async fn get_effective_permissions(
        &self,
        subject: &Subject,
        project_id: i64,
    ) -> Result<Vec<ProjectPermissionType>, AuthzError> {
        self.get_subject_project_permissions(subject, project_id).await
    }

    /// Invalidate cache for a subject/project
    pub fn invalidate_cache(&self, subject: &Subject, project_id: i64) {
        let cache_key = format!("{}:{}", subject.principal_string(), project_id);
        self.cache.remove(&cache_key);
    }

    /// Invalidate all cache for a subject
    pub fn invalidate_subject_cache(&self, subject: &Subject) {
        let prefix = subject.principal_string();
        self.cache.retain(|k, _| !k.starts_with(&prefix));
    }
}

/// Base authorization service (for default roles)
pub struct BaseAuthorizationService {
    role_repo: RoleRepository,
    project_repo: ProjectRepository,
    db_pool: DbPool,
}

impl BaseAuthorizationService {
    pub fn new(db_pool: DbPool) -> Self {
        Self {
            role_repo: RoleRepository::new(),
            project_repo: ProjectRepository::new(),
            db_pool,
        }
    }

    /// Get all base authorizations (default roles for projects)
    pub async fn query(&self) -> Result<Vec<onedev_db::models::role::DbRole>, AuthzError> {
        // This would query the base_authorizations table
        // For now, return empty
        Ok(Vec::new())
    }
}