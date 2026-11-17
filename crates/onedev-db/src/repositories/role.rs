//! Role Repository — Vanilla free-form roles.

use crate::{
    models::role::{DbCreateRole, DbRole, DbUpdateRole, RoleQueryBuilder},
    repositories::Repository,
    DbPool, DbResult,
};
use async_trait::async_trait;
use sqlx::FromRow;

pub struct RoleRepository;

impl RoleRepository {
    pub fn new() -> Self {
        Self
    }

    /// Find by name
    pub async fn find_by_name(&self, pool: &DbPool, name: &str) -> DbResult<Option<DbRole>> {
        let row = sqlx::query(&RoleQueryBuilder::select_by_name())
            .bind(name)
            .fetch_optional(pool)
            .await?;

        Ok(row.map(|r| DbRole::from_row(&r)).transpose()?)
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
            .bind(create.manage_project)
            .bind(create.create_children)
            .bind(create.manage_pull_requests)
            .bind(create.manage_code_comments)
            .bind(create.code_privilege_str())
            .bind(create.pack_privilege_str())
            .bind(create.manage_issues)
            .bind(create.access_confidential_issues)
            .bind(create.access_time_tracking)
            .bind(create.schedule_issues)
            .bind(serde_json::to_value(&create.editable_issue_fields)?)
            .bind(create.can_edit_fields_of_other_issues)
            .bind(serde_json::to_value(&create.editable_issue_links)?)
            .bind(create.manage_builds)
            .bind(create.upload_cache)
            .bind(serde_json::to_value(&create.job_privileges)?)
            .bind(&create.accessible_sites)
            .bind(create.manage_workspaces)
            .bind(create.create_workspaces)
            .bind(create.maintenance)
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
        let row = sqlx::query(&RoleQueryBuilder::select_by_id())
            .bind(id)
            .fetch_optional(pool)
            .await?;

        Ok(row.map(|r| DbRole::from_row(&r)).transpose()?)
    }

    async fn update(&self, pool: &DbPool, id: i64, update: DbUpdateRole) -> DbResult<DbRole> {
        let editable_issue_fields = update
            .editable_issue_fields
            .as_ref()
            .map(serde_json::to_value)
            .transpose()?;
        let editable_issue_links = update
            .editable_issue_links
            .as_ref()
            .map(serde_json::to_value)
            .transpose()?;
        let job_privileges = update
            .job_privileges
            .as_ref()
            .map(serde_json::to_value)
            .transpose()?;

        let row = sqlx::query(&RoleQueryBuilder::update())
            .bind(id)
            .bind(&update.name)
            .bind(&update.description)
            .bind(update.manage_project)
            .bind(update.create_children)
            .bind(update.manage_pull_requests)
            .bind(update.manage_code_comments)
            .bind(update.code_privilege_str())
            .bind(update.pack_privilege_str())
            .bind(update.manage_issues)
            .bind(update.access_confidential_issues)
            .bind(update.access_time_tracking)
            .bind(update.schedule_issues)
            .bind(editable_issue_fields)
            .bind(update.can_edit_fields_of_other_issues)
            .bind(editable_issue_links)
            .bind(update.manage_builds)
            .bind(update.upload_cache)
            .bind(job_privileges)
            .bind(&update.accessible_sites)
            .bind(update.manage_workspaces)
            .bind(update.create_workspaces)
            .bind(update.maintenance)
            .fetch_one(pool)
            .await?;

        Ok(DbRole::from_row(&row)?)
    }

    async fn delete(&self, pool: &DbPool, id: i64) -> DbResult<()> {
        sqlx::query(RoleQueryBuilder::delete())
            .bind(id)
            .execute(pool)
            .await?;
        Ok(())
    }

    async fn list(&self, pool: &DbPool, limit: i64, offset: i64) -> DbResult<Vec<DbRole>> {
        let rows = sqlx::query(&format!(
            "{} ORDER BY created_at DESC LIMIT $1 OFFSET $2",
            RoleQueryBuilder::select_all()
        ))
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
