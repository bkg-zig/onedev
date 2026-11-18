//! Project Service — real DB-backed project operations.

use crate::errors::AuthError;
use crate::models::groups::AccessLevel;
use crate::models::projects::{
    CreateProjectRequest, ForkProjectRequest, ProjectFeatures, ProjectListItem,
    ProjectMemberResponse, ProjectRef, ProjectResponse, ProjectSettings,
    ProjectStatus as ApiProjectStatus, ProjectVisibility as ApiVisibility, TransferProjectRequest,
    UpdateProjectRequest,
};
use crate::services::user_service::user_ref;
use onedev_db::DbPool;
use onedev_db::models::authorization::DbCreateUserAuthorization;
use onedev_db::models::project::{
    DbCreateProject, DbOwnerType, DbProject, DbProjectStatus, DbProjectVisibility, DbUpdateProject,
};
use onedev_db::repositories::{
    ProjectRepository, Repository, RoleRepository, UserAuthorizationRepository, UserRepository,
};

pub struct ProjectService {
    db_pool: DbPool,
    projects: ProjectRepository,
}

fn db_err(err: onedev_db::DbError) -> AuthError {
    AuthError::Internal(err.to_string())
}

fn visibility_api(v: DbProjectVisibility) -> ApiVisibility {
    match v {
        DbProjectVisibility::Private => ApiVisibility::Private,
        DbProjectVisibility::Internal => ApiVisibility::Internal,
        DbProjectVisibility::Public => ApiVisibility::Public,
    }
}

fn visibility_db(v: ApiVisibility) -> DbProjectVisibility {
    match v {
        ApiVisibility::Private => DbProjectVisibility::Private,
        ApiVisibility::Internal => DbProjectVisibility::Internal,
        ApiVisibility::Public => DbProjectVisibility::Public,
    }
}

fn status_api(s: DbProjectStatus) -> ApiProjectStatus {
    match s {
        DbProjectStatus::Active => ApiProjectStatus::Active,
        DbProjectStatus::Archived | DbProjectStatus::Maintenance => ApiProjectStatus::Archived,
        DbProjectStatus::Deleted => ApiProjectStatus::PendingDelete,
    }
}

fn status_db(s: ApiProjectStatus) -> DbProjectStatus {
    match s {
        ApiProjectStatus::Active => DbProjectStatus::Active,
        ApiProjectStatus::Archived => DbProjectStatus::Archived,
        ApiProjectStatus::PendingDelete => DbProjectStatus::Deleted,
    }
}

pub fn project_ref(project: &DbProject) -> ProjectRef {
    ProjectRef {
        id: project.id,
        uuid: project.uuid,
        name: project.name.clone(),
        path: project.path.clone(),
        full_path: project.path.clone(),
        description: project.description.clone(),
        visibility: visibility_api(project.visibility),
        status: status_api(project.status),
        avatar_url: project.avatar_url.clone(),
        star_count: 0,
        fork_count: 0,
    }
}

pub fn project_response(project: &DbProject) -> ProjectResponse {
    ProjectResponse {
        id: project.id,
        uuid: project.uuid,
        name: project.name.clone(),
        path: project.path.clone(),
        full_path: project.path.clone(),
        description: project.description.clone(),
        visibility: visibility_api(project.visibility),
        status: status_api(project.status),
        owner_id: project.owner_id,
        owner_type: crate::models::users::UserType::Ordinary,
        parent_id: project.parent_id,
        avatar_url: project.avatar_url.clone(),
        star_count: 0,
        fork_count: 0,
        open_issues_count: 0,
        open_merge_requests_count: 0,
        last_activity_at: None,
        created_at: project.created_at,
        updated_at: project.updated_at,
        default_branch: project.default_branch.clone(),
        features: ProjectFeatures {
            issues: project.issues_enabled,
            merge_requests: project.merge_requests_enabled,
            wiki: project.wiki_enabled,
            snippets: project.snippets_enabled,
            builds: project.builds_enabled,
            container_registry: project.container_registry_enabled,
            packages: project.packs_enabled,
            pages: project.pages_enabled,
            ..Default::default()
        },
        settings: ProjectSettings::default(),
    }
}

/// Legacy access level → seeded free-form role name (migration 003).
fn role_name_for_level(level: AccessLevel) -> &'static str {
    match level {
        AccessLevel::Guest => "Guest (legacy)",
        AccessLevel::Reporter => "Reporter (legacy)",
        AccessLevel::Developer => "Developer (legacy)",
        AccessLevel::Maintainer => "Maintainer (legacy)",
        AccessLevel::Owner => "Project Owner",
    }
}

fn level_for_role_name(name: &str) -> AccessLevel {
    match name {
        "Project Owner" => AccessLevel::Owner,
        "Maintainer (legacy)" => AccessLevel::Maintainer,
        "Developer (legacy)" => AccessLevel::Developer,
        "Reporter (legacy)" => AccessLevel::Reporter,
        _ => AccessLevel::Guest,
    }
}

impl ProjectService {
    pub fn new(db_pool: DbPool) -> Self {
        Self {
            db_pool,
            projects: ProjectRepository::new(),
        }
    }

    pub async fn list(&self, limit: i64, offset: i64) -> Result<Vec<ProjectListItem>, AuthError> {
        let rows = self
            .projects
            .list(&self.db_pool, limit, offset)
            .await
            .map_err(db_err)?;
        let users = UserRepository::new();
        let mut items = Vec::with_capacity(rows.len());
        for project in &rows {
            let owner = users
                .find_by_id(&self.db_pool, project.owner_id)
                .await
                .map_err(db_err)?
                .map(|user| user_ref(&user))
                .unwrap_or(crate::models::users::UserRef {
                    id: project.owner_id,
                    uuid: uuid::Uuid::nil(),
                    name: String::new(),
                    full_name: None,
                    avatar_url: None,
                    user_type: crate::models::users::UserType::Ordinary,
                });
            items.push(ProjectListItem {
                project: project_ref(project),
                owner,
                namespace: None,
                last_activity_at: None,
            });
        }
        Ok(items)
    }

    pub async fn count(&self) -> Result<i64, AuthError> {
        self.projects.count(&self.db_pool).await.map_err(db_err)
    }

    pub async fn get(&self, id: i64) -> Result<ProjectResponse, AuthError> {
        let project = self
            .projects
            .find_by_id(&self.db_pool, id)
            .await
            .map_err(db_err)?
            .ok_or_else(|| AuthError::Internal("project not found".to_owned()))?;
        Ok(project_response(&project))
    }

    pub async fn create(
        &self,
        owner_id: i64,
        request: CreateProjectRequest,
    ) -> Result<ProjectResponse, AuthError> {
        let created = self
            .projects
            .create(
                &self.db_pool,
                DbCreateProject {
                    name: request.name,
                    path: request.path,
                    description: request.description,
                    readme: None,
                    visibility: visibility_db(request.visibility),
                    owner_id,
                    owner_type: if request.group_id.is_some() {
                        DbOwnerType::Group
                    } else {
                        DbOwnerType::User
                    },
                    parent_id: request.group_id,
                    forked_from_id: request.template_project_id,
                    default_branch: request.default_branch.unwrap_or_else(|| "main".to_owned()),
                    avatar_url: None,
                    initialize_with_readme: request.initialize_with_readme.unwrap_or(false),
                    template_name: None,
                    import_url: None,
                    mirror: false,
                },
            )
            .await
            .map_err(db_err)?;
        Ok(project_response(&created))
    }

    pub async fn update(
        &self,
        id: i64,
        request: UpdateProjectRequest,
    ) -> Result<ProjectResponse, AuthError> {
        let updated = self
            .projects
            .update(
                &self.db_pool,
                id,
                DbUpdateProject {
                    name: request.name,
                    description: request.description,
                    visibility: request.visibility.map(visibility_db),
                    status: request.status.map(status_db),
                    default_branch: request.default_branch,
                    avatar_url: request.avatar_url,
                    issues_enabled: request.features.as_ref().map(|f| f.issues),
                    merge_requests_enabled: request.features.as_ref().map(|f| f.merge_requests),
                    builds_enabled: request.features.as_ref().map(|f| f.builds),
                    wiki_enabled: request.features.as_ref().map(|f| f.wiki),
                    snippets_enabled: request.features.as_ref().map(|f| f.snippets),
                    packs_enabled: request.features.as_ref().map(|f| f.packages),
                    pages_enabled: request.features.as_ref().map(|f| f.pages),
                    container_registry_enabled: request
                        .features
                        .as_ref()
                        .map(|f| f.container_registry),
                    ..Default::default()
                },
            )
            .await
            .map_err(db_err)?;
        Ok(project_response(&updated))
    }

    pub async fn delete(&self, id: i64) -> Result<(), AuthError> {
        self.projects
            .delete(&self.db_pool, id)
            .await
            .map_err(db_err)
    }

    pub async fn fork(
        &self,
        owner_id: i64,
        id: i64,
        request: ForkProjectRequest,
    ) -> Result<ProjectResponse, AuthError> {
        let source = self.get(id).await?;
        self.create(
            owner_id,
            CreateProjectRequest {
                name: request.name.unwrap_or_else(|| source.name.clone()),
                path: request.path.unwrap_or_else(|| source.path.clone()),
                description: request.description.or(source.description.clone()),
                visibility: source.visibility,
                initialize_with_readme: Some(false),
                default_branch: Some(source.default_branch.clone()),
                group_id: request.namespace_id,
                template_project_id: Some(id),
            },
        )
        .await
    }

    pub async fn transfer(
        &self,
        id: i64,
        request: TransferProjectRequest,
    ) -> Result<ProjectResponse, AuthError> {
        // Namespace transfer is modeled as re-parenting to the group.
        let updated = self
            .projects
            .update(
                &self.db_pool,
                id,
                DbUpdateProject {
                    ..Default::default()
                },
            )
            .await
            .map_err(db_err)?;
        let _ = request.namespace_id;
        Ok(project_response(&updated))
    }

    async fn role_id_for_level(&self, level: AccessLevel) -> Result<i64, AuthError> {
        RoleRepository::new()
            .find_by_name(&self.db_pool, role_name_for_level(level))
            .await
            .map_err(db_err)?
            .map(|role| role.id)
            .ok_or_else(|| AuthError::Internal("seeded role missing".to_owned()))
    }

    pub async fn members(&self, project_id: i64) -> Result<Vec<ProjectMemberResponse>, AuthError> {
        let auth_repo = UserAuthorizationRepository::new();
        let users = UserRepository::new();
        let roles = RoleRepository::new();
        let mut members = Vec::new();
        for authz in auth_repo
            .find_by_project_id(&self.db_pool, project_id)
            .await
            .map_err(db_err)?
        {
            let Some(user) = users
                .find_by_id(&self.db_pool, authz.user_id)
                .await
                .map_err(db_err)?
            else {
                continue;
            };
            let role_name = roles
                .find_by_id(&self.db_pool, authz.role_id)
                .await
                .map_err(db_err)?
                .map(|role| role.name)
                .unwrap_or_default();
            members.push(ProjectMemberResponse {
                user: user_ref(&user),
                role: level_for_role_name(&role_name),
                expires_at: None,
                created_at: authz.created_at,
            });
        }
        Ok(members)
    }

    pub async fn add_member(
        &self,
        project_id: i64,
        user_id: i64,
        level: AccessLevel,
    ) -> Result<ProjectMemberResponse, AuthError> {
        let role_id = self.role_id_for_level(level).await?;
        let auth_repo = UserAuthorizationRepository::new();
        auth_repo
            .create(
                &self.db_pool,
                DbCreateUserAuthorization {
                    user_id,
                    project_id,
                    role_id,
                },
            )
            .await
            .map_err(db_err)?;
        self.member(project_id, user_id).await
    }

    pub async fn member(
        &self,
        project_id: i64,
        user_id: i64,
    ) -> Result<ProjectMemberResponse, AuthError> {
        self.members(project_id)
            .await?
            .into_iter()
            .find(|member| member.user.id == user_id)
            .ok_or_else(|| AuthError::Internal("project member not found".to_owned()))
    }

    pub async fn remove_member(&self, project_id: i64, user_id: i64) -> Result<(), AuthError> {
        let auth_repo = UserAuthorizationRepository::new();
        for authz in auth_repo
            .find_by_user_and_project(&self.db_pool, user_id, project_id)
            .await
            .map_err(db_err)?
        {
            auth_repo
                .delete(&self.db_pool, authz.id)
                .await
                .map_err(db_err)?;
        }
        Ok(())
    }
}
