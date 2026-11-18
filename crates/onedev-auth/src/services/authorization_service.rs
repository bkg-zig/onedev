//! Authorization Service - Permission checking and access control

use crate::models::principal::{AccessTokenPrincipal, UserPrincipal};
use crate::models::subject::Subject;
use dashmap::DashMap;
use onedev_core::models::group::GroupRef;
use onedev_core::models::project::ProjectRef;
use onedev_core::models::role::{CodePrivilege, Role};
use onedev_core::models::user::UserRef;
use onedev_core::permissions::{
    BasePermission, Permission, ProjectPermissionType, SystemPermission,
};
use onedev_db::DbError;
use onedev_db::DbPool;
use onedev_db::repositories::{
    GroupRepository, MembershipRepository, ProjectRepository, Repository, RoleRepository,
    UserAuthorizationRepository, UserRepository,
};
use std::sync::Arc;
use std::time::Duration;
use thiserror::Error;
use tokio::sync::RwLock;

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
        // Root/system users pass every project check (vanilla admin chain).
        if let Subject::User(principal) = subject {
            if self.is_system_admin(principal.user_id).await? {
                return Ok(PermissionCheck::Granted);
            }
        }

        let roles = self.subject_project_roles(subject, project_id).await?;
        let granted: Vec<(i64, &Role)> = roles.iter().map(|role| (project_id, role)).collect();
        let required =
            onedev_core::permissions::ProjectPrivilege::new(project_id, permission.to_privilege());

        if onedev_core::permissions::PermissionChecker::check_roles(&granted, &required, &[]) {
            Ok(PermissionCheck::Granted)
        } else {
            Ok(PermissionCheck::Denied(format!(
                "Missing permission: {}",
                permission.name()
            )))
        }
    }

    /// Roles a subject holds on a project: direct user authorizations,
    /// access-token authorizations, and base authorizations (everyone).
    async fn subject_project_roles(
        &self,
        subject: &Subject,
        project_id: i64,
    ) -> Result<Vec<Role>, AuthzError> {
        let repo = UserAuthorizationRepository::new();
        let mut role_ids: Vec<i64> = Vec::new();

        match subject {
            Subject::User(principal) => {
                for authz in repo
                    .find_by_user_and_project(&self.db_pool, principal.user_id, project_id)
                    .await?
                {
                    role_ids.push(authz.role_id);
                }
            }
            Subject::AccessToken(principal) => {
                for authz in repo
                    .find_access_token_authorizations(&self.db_pool, principal.token_id, project_id)
                    .await?
                {
                    role_ids.push(authz.role_id);
                }
            }
            Subject::Anonymous => {}
        }
        for authz in repo
            .find_base_authorizations(&self.db_pool, project_id)
            .await?
        {
            role_ids.push(authz.role_id);
        }

        let mut roles = Vec::with_capacity(role_ids.len());
        for role_id in role_ids {
            if let Some(db_role) = self.role_repo.find_by_id(&self.db_pool, role_id).await? {
                roles.push(db_role.to_core());
            }
        }
        Ok(roles)
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
            _ => {
                return Ok(PermissionCheck::Denied(
                    "System permissions require user authentication".to_string(),
                ));
            }
        };

        // Check if user is admin
        let is_admin = self.is_system_admin(user_id).await?;

        if is_admin
            || permission == SystemPermission::ViewAllProjects
            || permission == SystemPermission::ViewAllGroups
        {
            Ok(PermissionCheck::Granted)
        } else {
            Ok(PermissionCheck::Denied(format!(
                "Missing system permission: {}",
                permission.name()
            )))
        }
    }

    /// Check multiple project permissions at once
    pub async fn check_project_permissions(
        &self,
        subject: &Subject,
        project_id: i64,
        permissions: &[ProjectPermissionType],
    ) -> Result<PermissionCheck, AuthzError> {
        for required in permissions {
            if self
                .check_project_permission(subject, project_id, *required)
                .await?
                != PermissionCheck::Granted
            {
                return Ok(PermissionCheck::Denied(format!(
                    "Missing permission: {}",
                    required.name()
                )));
            }
        }

        Ok(PermissionCheck::Granted)
    }

    /// Check if subject can access a project (any permission)
    pub async fn can_access_project(
        &self,
        subject: &Subject,
        project_id: i64,
    ) -> Result<bool, AuthzError> {
        let result = self
            .check_project_permission(subject, project_id, ProjectPermissionType::ViewProject)
            .await?;
        Ok(result == PermissionCheck::Granted)
    }

    /// Check if subject can read code in a project
    pub async fn can_read_code(
        &self,
        subject: &Subject,
        project_id: i64,
    ) -> Result<bool, AuthzError> {
        let result = self
            .check_project_permission(subject, project_id, ProjectPermissionType::ReadCode)
            .await?;
        Ok(result == PermissionCheck::Granted)
    }

    /// Check if subject can write code in a project
    pub async fn can_write_code(
        &self,
        subject: &Subject,
        project_id: i64,
    ) -> Result<bool, AuthzError> {
        let result = self
            .check_project_permission(subject, project_id, ProjectPermissionType::WriteCode)
            .await?;
        Ok(result == PermissionCheck::Granted)
    }

    /// Check if subject can manage a project
    pub async fn can_manage_project(
        &self,
        subject: &Subject,
        project_id: i64,
    ) -> Result<bool, AuthzError> {
        let result = self
            .check_project_permission(subject, project_id, ProjectPermissionType::ManageProject)
            .await?;
        Ok(result == PermissionCheck::Granted)
    }

    /// Check if subject is project admin
    pub async fn is_project_admin(
        &self,
        subject: &Subject,
        project_id: i64,
    ) -> Result<bool, AuthzError> {
        let result = self
            .check_project_permission(subject, project_id, ProjectPermissionType::AdminProject)
            .await?;
        Ok(result == PermissionCheck::Granted)
    }

    /// Get all project permissions for a subject: the legacy permission
    /// types implied by the subject's Vanilla roles on the project.
    async fn get_subject_project_permissions(
        &self,
        subject: &Subject,
        project_id: i64,
    ) -> Result<Vec<ProjectPermissionType>, AuthzError> {
        let cache_key = format!("{}:{}", subject.principal_string(), project_id);

        if let Some(cached) = self.cache.get(&cache_key) {
            if cached.expires_at > std::time::Instant::now() {
                return Ok(cached.permissions.clone());
            }
        }

        // Candidate set: every legacy type is tested against the roles.
        const CANDIDATES: &[ProjectPermissionType] = &[
            ProjectPermissionType::ViewProject,
            ProjectPermissionType::ManageProject,
            ProjectPermissionType::AdminProject,
            ProjectPermissionType::ReadCode,
            ProjectPermissionType::WriteCode,
            ProjectPermissionType::ViewConfidentialIssues,
            ProjectPermissionType::ViewBuildLogs,
            ProjectPermissionType::TriggerBuild,
            ProjectPermissionType::ViewPackages,
            ProjectPermissionType::UploadPackage,
            ProjectPermissionType::CreateWorkspace,
        ];

        let roles = self.subject_project_roles(subject, project_id).await?;
        let granted: Vec<(i64, &Role)> = roles.iter().map(|role| (project_id, role)).collect();
        let mut permissions = Vec::new();
        for candidate in CANDIDATES {
            let required = onedev_core::permissions::ProjectPrivilege::new(
                project_id,
                candidate.to_privilege(),
            );
            if onedev_core::permissions::PermissionChecker::check_roles(&granted, &required, &[]) {
                permissions.push(*candidate);
            }
        }

        self.cache.insert(
            cache_key,
            CachedAuthzInfo {
                permissions: permissions.clone(),
                roles,
                expires_at: std::time::Instant::now() + self.cache_ttl,
            },
        );

        Ok(permissions)
    }

    /// Check if user is system admin
    async fn is_system_admin(&self, user_id: i64) -> Result<bool, AuthzError> {
        let user = self.user_repo.find_by_id(&self.db_pool, user_id).await?;
        Ok(user
            .map(|u| {
                u.id == onedev_core::models::user::ROOT_USER_ID
                    || u.id == onedev_core::models::user::SYSTEM_USER_ID
            })
            .unwrap_or(false))
    }

    /// Get a user's roles in a project (vanilla: a user can hold
    /// several roles on the same project).
    pub async fn get_user_project_roles(
        &self,
        user_id: i64,
        project_id: i64,
    ) -> Result<Vec<Role>, AuthzError> {
        let repo = UserAuthorizationRepository::new();
        let authz = repo
            .find_by_user_and_project(&self.db_pool, user_id, project_id)
            .await
            .map_err(|e| AuthzError::Internal(e.to_string()))?;

        let mut roles = Vec::with_capacity(authz.len());
        for row in authz {
            if let Some(db_role) = self
                .role_repo
                .find_by_id(&self.db_pool, row.role_id)
                .await?
            {
                roles.push(db_role.to_core());
            }
        }
        Ok(roles)
    }

    /// Get effective permissions for a subject on a project (for UI)
    pub async fn get_effective_permissions(
        &self,
        subject: &Subject,
        project_id: i64,
    ) -> Result<Vec<ProjectPermissionType>, AuthzError> {
        self.get_subject_project_permissions(subject, project_id)
            .await
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
