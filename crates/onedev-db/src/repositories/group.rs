//! Group Repository

use crate::{
    DbPool, DbResult,
    models::group::{DbCreateGroup, DbGroup, DbUpdateGroup, GroupQueryBuilder},
    repositories::Repository,
};
use async_trait::async_trait;
use sqlx::FromRow;

pub struct GroupRepository;

impl GroupRepository {
    pub fn new() -> Self {
        Self
    }

    /// Find group by path (and optional parent)
    pub async fn find_by_path(
        &self,
        pool: &DbPool,
        path: &str,
        parent_id: Option<i64>,
    ) -> DbResult<Option<DbGroup>> {
        let row = sqlx::query(GroupQueryBuilder::select_by_path())
            .bind(path)
            .bind(parent_id)
            .fetch_optional(pool)
            .await?;

        Ok(row.map(|r| DbGroup::from_row(&r)).transpose()?)
    }

    /// Find group by UUID
    pub async fn find_by_uuid(&self, pool: &DbPool, uuid: uuid::Uuid) -> DbResult<Option<DbGroup>> {
        let row = sqlx::query(GroupQueryBuilder::select_by_uuid())
            .bind(uuid)
            .fetch_optional(pool)
            .await?;

        Ok(row.map(|r| DbGroup::from_row(&r)).transpose()?)
    }

    /// List groups with filters
    pub async fn list_filtered(
        &self,
        pool: &DbPool,
        term: Option<&str>,
        visibility: Option<crate::models::group::DbGroupVisibility>,
        parent_id: Option<i64>,
        page: u32,
        per_page: u32,
    ) -> DbResult<Vec<DbGroup>> {
        let mut query = String::from(GroupQueryBuilder::select_all());
        query.push_str(" WHERE deleted_at IS NULL");

        let mut conditions = Vec::new();
        let mut param_idx = 1;

        if let Some(_term) = term {
            conditions.push(format!(
                "(name ILIKE ${} OR path ILIKE ${} OR description ILIKE ${})",
                param_idx, param_idx, param_idx
            ));
            param_idx += 1;
        }

        if visibility.is_some() {
            conditions.push(format!("visibility = ${}", param_idx));
            param_idx += 1;
        }

        if parent_id.is_some() {
            conditions.push(format!("parent_id = ${}", param_idx));
            param_idx += 1;
        }

        if !conditions.is_empty() {
            query.push_str(" AND ");
            query.push_str(&conditions.join(" AND "));
        }

        query.push_str(" ORDER BY created_at DESC LIMIT $");
        query.push_str(&param_idx.to_string());
        param_idx += 1;
        query.push_str(" OFFSET $");
        query.push_str(&param_idx.to_string());

        let mut q = sqlx::query(&query);

        if let Some(term) = term {
            let pattern = format!("%{}%", term);
            q = q.bind(pattern);
        }

        if let Some(v) = visibility {
            q = q.bind(v);
        }

        if let Some(pid) = parent_id {
            q = q.bind(pid);
        }

        q = q.bind(per_page as i64);
        q = q.bind((page * per_page) as i64);

        let rows = q.fetch_all(pool).await?;
        let groups = rows
            .into_iter()
            .map(|r| DbGroup::from_row(&r))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(groups)
    }
}

#[async_trait]
impl Repository<DbGroup, DbCreateGroup, DbUpdateGroup> for GroupRepository {
    async fn create(&self, pool: &DbPool, create: DbCreateGroup) -> DbResult<DbGroup> {
        let uuid = uuid::Uuid::new_v4();
        let now = chrono::Utc::now();

        let id: i64 = sqlx::query_scalar(GroupQueryBuilder::insert())
            .bind(uuid)
            .bind(&create.name)
            .bind(&create.path)
            .bind(&create.description)
            .bind(create.parent_id)
            .bind(&create.avatar_url)
            .bind(create.visibility)
            .bind(now)
            .bind(now)
            .bind(create.require_two_factor)
            .bind(create.two_factor_grace_period)
            .bind(create.project_creation_level)
            .bind(create.subgroup_creation_level)
            .bind(Option::<serde_json::Value>::None) // settings
            .fetch_one(pool)
            .await?;

        self.find_by_id(pool, id)
            .await?
            .ok_or_else(|| crate::DbError::NotFound {
                entity_type: "Group".to_string(),
                id: id.to_string(),
            })
    }

    async fn find_by_id(&self, pool: &DbPool, id: i64) -> DbResult<Option<DbGroup>> {
        let row = sqlx::query(GroupQueryBuilder::select_by_id())
            .bind(id)
            .fetch_optional(pool)
            .await?;

        Ok(row.map(|r| DbGroup::from_row(&r)).transpose()?)
    }

    async fn update(&self, pool: &DbPool, id: i64, update: DbUpdateGroup) -> DbResult<DbGroup> {
        let row = sqlx::query(GroupQueryBuilder::update())
            .bind(id)
            .bind(&update.name)
            .bind(&update.description)
            .bind(&update.avatar_url)
            .bind(update.visibility)
            .bind(update.require_two_factor)
            .bind(update.two_factor_grace_period)
            .bind(update.project_creation_level)
            .bind(update.subgroup_creation_level)
            .bind(update.settings.map(serde_json::to_value).transpose()?)
            .fetch_one(pool)
            .await?;

        Ok(DbGroup::from_row(&row)?)
    }

    async fn delete(&self, pool: &DbPool, id: i64) -> DbResult<()> {
        sqlx::query(GroupQueryBuilder::delete())
            .bind(id)
            .execute(pool)
            .await?;
        Ok(())
    }

    async fn list(&self, pool: &DbPool, limit: i64, offset: i64) -> DbResult<Vec<DbGroup>> {
        let rows = sqlx::query(
            "SELECT * FROM groups WHERE deleted_at IS NULL ORDER BY created_at DESC LIMIT $1 OFFSET $2"
        )
        .bind(limit)
        .bind(offset)
        .fetch_all(pool)
        .await?;

        let groups = rows
            .into_iter()
            .map(|r| DbGroup::from_row(&r))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(groups)
    }

    async fn count(&self, pool: &DbPool) -> DbResult<i64> {
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM groups WHERE deleted_at IS NULL")
            .fetch_one(pool)
            .await?;
        Ok(count)
    }
}
