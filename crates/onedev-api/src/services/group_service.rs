//! Group Service — real DB-backed group operations.

use crate::errors::AuthError;
use crate::models::groups::{
    AccessLevel, AddGroupMemberRequest, CreateGroupRequest, GroupListItem, GroupMemberResponse,
    GroupQuery, GroupRef, GroupResponse, ProjectCreationLevel, UpdateGroupRequest,
};
use crate::services::user_service::user_ref;
use onedev_db::models::group::{
    DbCreateGroup, DbGroup, DbGroupVisibility, DbProjectCreationLevel, DbSubgroupCreationLevel,
    DbUpdateGroup,
};
use onedev_db::models::membership::{DbCreateMembership, DbMembership};
use onedev_db::repositories::{
    GroupRepository, MembershipRepository, Repository, UserRepository,
};
use onedev_db::DbPool;

fn db_err(err: onedev_db::DbError) -> AuthError {
    AuthError::Internal(err.to_string())
}

fn visibility_db(v: crate::models::groups::GroupVisibility) -> DbGroupVisibility {
    match v {
        crate::models::groups::GroupVisibility::Private => DbGroupVisibility::Private,
        crate::models::groups::GroupVisibility::Internal => DbGroupVisibility::Internal,
        crate::models::groups::GroupVisibility::Public => DbGroupVisibility::Public,
    }
}

fn visibility_api(v: DbGroupVisibility) -> crate::models::groups::GroupVisibility {
    match v {
        DbGroupVisibility::Private => crate::models::groups::GroupVisibility::Private,
        DbGroupVisibility::Internal => crate::models::groups::GroupVisibility::Internal,
        DbGroupVisibility::Public => crate::models::groups::GroupVisibility::Public,
    }
}

fn creation_level_db(v: ProjectCreationLevel) -> DbProjectCreationLevel {
    match v {
        ProjectCreationLevel::NoOne => DbProjectCreationLevel::Noone,
        ProjectCreationLevel::Maintainer => DbProjectCreationLevel::Maintainer,
        ProjectCreationLevel::Developer => DbProjectCreationLevel::Developer,
        ProjectCreationLevel::Owner => DbProjectCreationLevel::Owner,
    }
}

fn creation_level_api(v: DbProjectCreationLevel) -> ProjectCreationLevel {
    match v {
        DbProjectCreationLevel::Noone => ProjectCreationLevel::NoOne,
        DbProjectCreationLevel::Maintainer => ProjectCreationLevel::Maintainer,
        DbProjectCreationLevel::Developer => ProjectCreationLevel::Developer,
        DbProjectCreationLevel::Owner => ProjectCreationLevel::Owner,
    }
}

fn subgroup_level_api(v: DbSubgroupCreationLevel) -> crate::models::groups::SubgroupCreationLevel {
    match v {
        DbSubgroupCreationLevel::Owner => crate::models::groups::SubgroupCreationLevel::Owner,
        DbSubgroupCreationLevel::Maintainer => {
            crate::models::groups::SubgroupCreationLevel::Maintainer
        }
    }
}

fn access_level_api(level: onedev_db::models::membership::DbGroupRole) -> AccessLevel {
    match level {
        onedev_db::models::membership::DbGroupRole::Guest => AccessLevel::Guest,
        onedev_db::models::membership::DbGroupRole::Reporter => AccessLevel::Reporter,
        onedev_db::models::membership::DbGroupRole::Developer => AccessLevel::Developer,
        onedev_db::models::membership::DbGroupRole::Maintainer => AccessLevel::Maintainer,
        onedev_db::models::membership::DbGroupRole::Owner => AccessLevel::Owner,
    }
}

fn access_level_db(level: AccessLevel) -> onedev_db::models::membership::DbGroupRole {
    use onedev_db::models::membership::DbGroupRole;
    match level {
        AccessLevel::Guest => DbGroupRole::Guest,
        AccessLevel::Reporter => DbGroupRole::Reporter,
        AccessLevel::Developer => DbGroupRole::Developer,
        AccessLevel::Maintainer => DbGroupRole::Maintainer,
        AccessLevel::Owner => DbGroupRole::Owner,
    }
}

fn group_ref(group: &DbGroup) -> GroupRef {
    GroupRef {
        id: group.id,
        uuid: group.uuid,
        name: group.name.clone(),
        path: group.path.clone(),
        full_path: group.path.clone(),
        description: group.description.clone(),
        visibility: visibility_api(group.visibility),
        avatar_url: group.avatar_url.clone(),
        project_count: 0,
        subgroup_count: 0,
    }
}

fn group_response(group: &DbGroup) -> GroupResponse {
    GroupResponse {
        id: group.id,
        uuid: group.uuid,
        name: group.name.clone(),
        path: group.path.clone(),
        full_path: group.path.clone(),
        description: group.description.clone(),
        visibility: visibility_api(group.visibility),
        parent_id: group.parent_id,
        owner_id: 1,
        avatar_url: group.avatar_url.clone(),
        project_count: 0,
        subgroup_count: 0,
        require_two_factor: group.require_two_factor,
        two_factor_grace_period: group.two_factor_grace_period as u32,
        project_creation_level: creation_level_api(group.project_creation_level),
        subgroup_creation_level: subgroup_level_api(group.subgroup_creation_level),
        visibility_level: crate::models::groups::VisibilityLevel::Private,
        shared_runners_minutes: None,
        extra_shared_runners_minutes: None,
        created_at: group.created_at,
        updated_at: group.updated_at,
    }
}

pub struct GroupService {
    db_pool: DbPool,
    groups: GroupRepository,
}

impl GroupService {
    pub fn new(db_pool: DbPool) -> Self {
        Self {
            db_pool,
            groups: GroupRepository::new(),
        }
    }

    pub async fn list(&self, query: &GroupQuery) -> Result<Vec<GroupListItem>, AuthError> {
        let limit = query.pagination.per_page.max(1) as i64;
        let offset = query.pagination.page as i64 * limit;
        let users = UserRepository::new();
        let rows = self.groups.list(&self.db_pool, limit, offset).await.map_err(db_err)?;
        let mut items = Vec::with_capacity(rows.len());
        for group in &rows {
            let owner = users
                .find_by_id(&self.db_pool, 1)
                .await
                .map_err(db_err)?
                .map(|user| user_ref(&user))
                .unwrap_or(crate::models::users::UserRef {
                    id: 1,
                    uuid: uuid::Uuid::nil(),
                    name: String::new(),
                    full_name: None,
                    avatar_url: None,
                    user_type: crate::models::users::UserType::Ordinary,
                });
            items.push(GroupListItem {
                group: group_ref(group),
                owner,
            });
        }
        Ok(items)
    }

    pub async fn count(&self) -> Result<i64, AuthError> {
        self.groups.count(&self.db_pool).await.map_err(db_err)
    }

    pub async fn get(&self, id: i64) -> Result<GroupResponse, AuthError> {
        let group = self
            .groups
            .find_by_id(&self.db_pool, id)
            .await
            .map_err(db_err)?
            .ok_or_else(|| AuthError::Internal("group not found".to_owned()))?;
        Ok(group_response(&group))
    }

    pub async fn create(&self, request: CreateGroupRequest) -> Result<GroupResponse, AuthError> {
        let created = self
            .groups
            .create(
                &self.db_pool,
                DbCreateGroup {
                    name: request.name,
                    path: request.path,
                    description: request.description,
                    parent_id: request.parent_id,
                    avatar_url: None,
                    visibility: visibility_db(request.visibility),
                    require_two_factor: false,
                    two_factor_grace_period: 0,
                    project_creation_level: DbProjectCreationLevel::Maintainer,
                    subgroup_creation_level: DbSubgroupCreationLevel::Owner,
                },
            )
            .await
            .map_err(db_err)?;
        Ok(group_response(&created))
    }

    pub async fn update(&self, id: i64, request: UpdateGroupRequest) -> Result<GroupResponse, AuthError> {
        let updated = self
            .groups
            .update(
                &self.db_pool,
                id,
                DbUpdateGroup {
                    name: request.name,
                    description: request.description,
                    avatar_url: request.avatar_url,
                    visibility: request.visibility.map(visibility_db),
                    require_two_factor: request.require_two_factor,
                    two_factor_grace_period: request.two_factor_grace_period.map(|v| v as i32),
                    project_creation_level: request.project_creation_level.map(creation_level_db),
                    subgroup_creation_level: None,
                    settings: None,
                },
            )
            .await
            .map_err(db_err)?;
        Ok(group_response(&updated))
    }

    pub async fn delete(&self, id: i64) -> Result<(), AuthError> {
        self.groups.delete(&self.db_pool, id).await.map_err(db_err)
    }

    pub async fn members(&self, group_id: i64) -> Result<Vec<GroupMemberResponse>, AuthError> {
        let memberships = MembershipRepository::new();
        let users = UserRepository::new();
        let mut out = Vec::new();
        for member in memberships
            .find_by_group_id(&self.db_pool, group_id)
            .await
            .map_err(db_err)?
        {
            let Some(user) = users.find_by_id(&self.db_pool, member.user_id).await.map_err(db_err)?
            else {
                continue;
            };
            out.push(GroupMemberResponse {
                user: user_ref(&user),
                access_level: access_level_api(member.role),
                expires_at: None,
                created_at: member.created_at,
            });
        }
        Ok(out)
    }

    pub async fn add_member(
        &self,
        group_id: i64,
        request: AddGroupMemberRequest,
    ) -> Result<GroupMemberResponse, AuthError> {
        let memberships = MembershipRepository::new();
        let created: DbMembership = memberships
            .create(
                &self.db_pool,
                DbCreateMembership {
                    user_id: request.user_id,
                    group_id,
                    role: access_level_db(request.access_level),
                },
            )
            .await
            .map_err(db_err)?;
        let user = UserRepository::new()
            .find_by_id(&self.db_pool, created.user_id)
            .await
            .map_err(db_err)?
            .ok_or_else(|| AuthError::Internal("user not found".to_owned()))?;
        Ok(GroupMemberResponse {
            user: user_ref(&user),
            access_level: request.access_level,
            expires_at: None,
            created_at: created.created_at,
        })
    }

    pub async fn remove_member(&self, group_id: i64, user_id: i64) -> Result<(), AuthError> {
        let memberships = MembershipRepository::new();
        if let Some(member) = memberships
            .find_by_user_and_group(&self.db_pool, user_id, group_id)
            .await
            .map_err(db_err)?
        {
            memberships.delete(&self.db_pool, member.id).await.map_err(db_err)?;
        }
        Ok(())
    }
}
