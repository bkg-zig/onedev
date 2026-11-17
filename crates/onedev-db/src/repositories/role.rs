//! Role Repository

use crate::{
    models::role::{DbRole, DbCreateRole, DbUpdateRole, DbRolePermission, RoleQueryBuilder, RolePermissionQueryBuilder},
    repositories::Repository,
    DbPool, DbResult,
};
use async_trait::async_trait;

pub struct RoleRepository;

impl RoleRepository {
    pub fn new() -> Self {
        Self
    }

    /// Find by name
    pub async fn find_by_name(&self, pool: &DbPool, name: &str) -> DbResult<Option<DbRole>> {
        let row = sqlx::query(RoleQueryBuilder::select_by_name())
            .bind(name)
            .fetch_optional(pool)
            .await?;

        Ok(row.map(|r| DbRole::from_row(&r)).transpose()?)
    }

    /// Find with permissions
    pub async fn find_with_permissions(&self, pool: &DbPool, id: i64) -> DbResult<Option<(DbRole, Vec<String>)>> {
        let rows = sqlx::query(RoleQueryBuilder::select_with_permissions())
            .bind(id)
            .fetch_all(pool)
            .await?;

        if rows.is_empty() {
            return Ok(None);
        }

        let mut role: Option<DbRole> = None;
        let mut permissions = Vec::new();

        for row in rows {
            if role.is_none() {
                role = Some(DbRole::from_row(&row)?);
            }
            let permission: Option<String> = row.get("permission");
            if let Some(p) = permission {
                permissions.push(p);
            }
        }

        Ok(role.map(|r| (r, permissions)))
    }
}

#[async_trait]
impl Repository<DbRole, DbCreateRole, DbUpdateRole> for RoleRepository {
    async fn create(&self, pool: &DbPool, create: DbCreateRole) -> DbResult<DbRole> {
        let uuid = uuid::Uuid::new_v4();
        let now = chrono::Utc::now();

        let id: i64 = sqlx::query_scalar(RoleQueryBuilder::insert())
            .bind(uuid)
            .bind(&create.name)
            .bind(&create.description)
            .bind(create.is_system)
            .bind(now)
            .bind(now)
            .fetch_one(pool)
            .await?;

        self.find_by_id(pool, id).await?.ok_or_else(|| crate::DbError::NotFound {
            entity_type: "Role".to_string(),
            id: id.to_string(),
        })
    }

    async fn find_by_id(&self, pool: &DbPool, id: i64) -> DbResult<Option<DbRole>> {
        let row = sqlx::query(RoleQueryBuilder::select_by_id())
            .bind(id)
            .fetch_optional(pool)
            .await?;

        Ok(row.map(|r| DbRole::from_row(&r)).transpose()?)
    }

    async fn update(&self, pool: &DbPool, id: i64, update: DbUpdateRole) -> DbResult<DbRole> {
        let row = sqlx::query(RoleQueryBuilder::update())
            .bind(id)
            .bind(&update.name)
            .bind(&update.description)
            .fetch_one(pool)
            .await?;

        DbRole::from_row(&row)
    }

    async fn delete(&self, pool: &DbPool, id: i64) -> DbResult<()> {
        sqlx::query(RoleQueryBuilder::delete())
            .bind(id)
            .execute(pool)
            .await?;
        Ok(())
    }

    async fn list(&self, pool: &DbPool, limit: i64, offset: i64) -> DbResult<Vec<DbRole>> {
        let rows = sqlx::query(
            "SELECT * FROM roles ORDER BY created_at DESC LIMIT $1 OFFSET $2"
        )
        .bind(limit)
        .bind(offset)
        .fetch_all(pool)
        .await?;

        let roles = rows.into_iter().map(|r| DbRole::from_row(&r)).collect::<Result<Vec<_>, _>>()?;
        Ok(roles)
    }

    async fn count(&self, pool: &DbPool) -> DbResult<i64> {
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM roles")
            .fetch_one(pool)
            .await?;
        Ok(count)
    }
}

pub struct RolePermissionRepository;

impl RolePermissionRepository {
    pub fn new() -> Self {
        Self
    }

    /// Find by role ID
    pub async fn find_by_role_id(&self, pool: &DbPool, role_id: i64) -> DbResult<Vec<DbRolePermission>> {
        let rows = sqlx::query(RolePermissionQueryBuilder::select_by_role_id())
            .bind(role_id)
            .fetch_all(pool)
            .await?;

        let perms = rows.into_iter().map(|r| DbRolePermission::from_row(&r)).collect::<Result<Vec<_>, _>>()?;
        Ok(perms)
    }

    /// Add permission
    pub async fn add_permission(&self, pool: &DbPool, role_id: i64, permission: &str, resource_type: Option<&str>, resource_id: Option<i64>) -> DbResult<DbRolePermission> {
        let uuid = uuid::Uuid::new_v4();
        let now = chrono::Utc::now();

        let id: i64 = sqlx::query_scalar(RolePermissionQueryBuilder::insert())
            .bind(uuid)
            .bind(role_id)
            .bind(permission)
            .bind(resource_type)
            .bind(resource_id)
            .bind(now)
            .fetch_one(pool)
            .await?;

        self.find_by_id(pool, id).await?.ok_or_else(|| crate::DbError::NotFound {
            entity_type: "RolePermission".to_string(),
            id: id.to_string(),
        })
    }

    /// Remove permission
    pub async fn remove_permission(&self, pool: &DbPool, role_id: i64, permission: &str) -> DbResult<()> {
        sqlx::query(RolePermissionQueryBuilder::delete_by_role_and_permission())
            .bind(role_id)
            .bind(permission)
            .execute(pool)
            .await?;
        Ok(())
    }

    /// Remove all permissions for role
    pub async fn remove_all_permissions(&self, pool: &DbPool, role_id: i64) -> DbResult<()> {
        sqlx::query(RolePermissionQueryBuilder::delete_by_role_id())
            .bind(role_id)
            .execute(pool)
            .await?;
        Ok(())
    }
}

#[async_trait]
impl Repository<DbRolePermission, DbRolePermission, DbRolePermission> for RolePermissionRepository {
    async fn create(&self, pool: &DbPool, create: DbRolePermission) -> DbResult<DbRolePermission> {
        let uuid = uuid::Uuid::new_v4();
        let now = chrono::Utc::now();

        let id: i64 = sqlx::query_scalar(RolePermissionQueryBuilder::insert())
            .bind(uuid)
            .bind(create.role_id)
            .bind(&create.permission)
            .bind(&create.resource_type)
            .bind(create.resource_id)
            .bind(now)
            .fetch_one(pool)
            .await?;

        self.find_by_id(pool, id).await?.ok_or_else(|| crate::DbError::NotFound {
            entity_type: "RolePermission".to_string(),
            id: id.to_string(),
        })
    }

    async fn find_by_id(&self, pool: &DbPool, id: i64) -> DbResult<Option<DbRolePermission>> {
        let row = sqlx::query("SELECT * FROM role_permissions WHERE id = $1")
            .bind(id)
            .fetch_optional(pool)
            .await?;

        Ok(row.map(|r| DbRolePermission::from_row(&r)).transpose()?)
    }

    async fn update(&self, pool: &DbPool, id: i64, update: DbRolePermission) -> DbResult<DbRolePermission> {
        let row = sqlx::query(
            "UPDATE role_permissions SET permission = $2, resource_type = $3, resource_id = $4 WHERE id = $1 RETURNING *"
        )
        .bind(id)
        .bind(&update.permission)
        .bind(&update.resource_type)
        .bind(update.resource_id)
        .fetch_one(pool)
        .await?;

        DbRolePermission::from_row(&row)
    }

    async fn delete(&self, pool: &DbPool, id: i64) -> DbResult<()> {
        sqlx::query("DELETE FROM role_permissions WHERE id = $1")
            .bind(id)
            .execute(pool)
            .await?;
        Ok(())
    }

    async fn list(&self, pool: &DbPool, limit: i64, offset: i64) -> DbResult<Vec<DbRolePermission>> {
        let rows = sqlx::query("SELECT * FROM role_permissions ORDER BY created_at DESC LIMIT $1 OFFSET $2")
            .bind(limit)
            .bind(offset)
            .fetch_all(pool)
            .await?;

        let perms = rows.into_iter().map(|r| DbRolePermission::from_row(&r)).collect::<Result<Vec<_>, _>>()?;
        Ok(perms)
    }

    async fn count(&self, pool: &DbPool) -> DbResult<i64> {
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM role_permissions")
            .fetch_one(pool)
            .await?;
        Ok(count)
    }
}