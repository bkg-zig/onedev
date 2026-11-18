//! Label Repository

use crate::{
    DbPool, DbResult,
    models::label::{DbCreateLabel, DbLabel, DbUpdateLabel, LabelQueryBuilder},
    repositories::Repository,
};
use async_trait::async_trait;
use sqlx::FromRow;

pub struct LabelRepository;

impl LabelRepository {
    pub fn new() -> Self {
        Self
    }

    /// Find by project ID
    pub async fn find_by_project_id(
        &self,
        pool: &DbPool,
        project_id: i64,
    ) -> DbResult<Vec<DbLabel>> {
        let rows = sqlx::query(LabelQueryBuilder::select_by_project_id())
            .bind(project_id)
            .fetch_all(pool)
            .await?;

        let labels = rows
            .into_iter()
            .map(|r| DbLabel::from_row(&r))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(labels)
    }

    /// Find by project and name
    pub async fn find_by_project_and_name(
        &self,
        pool: &DbPool,
        project_id: i64,
        name: &str,
    ) -> DbResult<Option<DbLabel>> {
        let row = sqlx::query(LabelQueryBuilder::select_by_project_and_name())
            .bind(project_id)
            .bind(name)
            .fetch_optional(pool)
            .await?;

        Ok(row.map(|r| DbLabel::from_row(&r)).transpose()?)
    }
}

#[async_trait]
impl Repository<DbLabel, DbCreateLabel, DbUpdateLabel> for LabelRepository {
    async fn create(&self, pool: &DbPool, create: DbCreateLabel) -> DbResult<DbLabel> {
        let uuid = uuid::Uuid::new_v4();
        let now = chrono::Utc::now();

        let id: i64 = sqlx::query_scalar(LabelQueryBuilder::insert())
            .bind(uuid)
            .bind(create.project_id)
            .bind(&create.name)
            .bind(&create.color)
            .bind(&create.description)
            .bind(now)
            .bind(now)
            .fetch_one(pool)
            .await?;

        self.find_by_id(pool, id)
            .await?
            .ok_or_else(|| crate::DbError::NotFound {
                entity_type: "Label".to_string(),
                id: id.to_string(),
            })
    }

    async fn find_by_id(&self, pool: &DbPool, id: i64) -> DbResult<Option<DbLabel>> {
        let row = sqlx::query(LabelQueryBuilder::select_by_id())
            .bind(id)
            .fetch_optional(pool)
            .await?;

        Ok(row.map(|r| DbLabel::from_row(&r)).transpose()?)
    }

    async fn update(&self, pool: &DbPool, id: i64, update: DbUpdateLabel) -> DbResult<DbLabel> {
        let row = sqlx::query(LabelQueryBuilder::update())
            .bind(id)
            .bind(&update.name)
            .bind(&update.color)
            .bind(&update.description)
            .fetch_one(pool)
            .await?;

        Ok(DbLabel::from_row(&row)?)
    }

    async fn delete(&self, pool: &DbPool, id: i64) -> DbResult<()> {
        sqlx::query(LabelQueryBuilder::delete())
            .bind(id)
            .execute(pool)
            .await?;
        Ok(())
    }

    async fn list(&self, pool: &DbPool, limit: i64, offset: i64) -> DbResult<Vec<DbLabel>> {
        let rows = sqlx::query("SELECT * FROM labels ORDER BY name ASC LIMIT $1 OFFSET $2")
            .bind(limit)
            .bind(offset)
            .fetch_all(pool)
            .await?;

        let labels = rows
            .into_iter()
            .map(|r| DbLabel::from_row(&r))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(labels)
    }

    async fn count(&self, pool: &DbPool) -> DbResult<i64> {
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM labels")
            .fetch_one(pool)
            .await?;
        Ok(count)
    }
}
