//! Package Repository

use crate::{
    DbPool, DbResult,
    models::pack::{DbCreatePack, DbPack, DbUpdatePack, PackQueryBuilder},
    repositories::Repository,
};
use async_trait::async_trait;
use sqlx::FromRow;

pub struct PackRepository;

impl PackRepository {
    pub fn new() -> Self {
        Self
    }

    /// Find by project and name
    pub async fn find_by_project_and_name(
        &self,
        pool: &DbPool,
        project_id: i64,
        name: &str,
    ) -> DbResult<Vec<DbPack>> {
        let rows = sqlx::query(PackQueryBuilder::select_by_project_and_name())
            .bind(project_id)
            .bind(name)
            .fetch_all(pool)
            .await?;

        let packs = rows
            .into_iter()
            .map(|r| DbPack::from_row(&r))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(packs)
    }

    /// Find latest version
    pub async fn find_latest_version(
        &self,
        pool: &DbPool,
        project_id: i64,
        name: &str,
    ) -> DbResult<Option<DbPack>> {
        let row = sqlx::query(
            "SELECT * FROM packs WHERE project_id = $1 AND name = $2 AND deleted_at IS NULL ORDER BY version DESC LIMIT 1"
        )
        .bind(project_id)
        .bind(name)
        .fetch_optional(pool)
        .await?;

        Ok(row.map(|r| DbPack::from_row(&r)).transpose()?)
    }
}

#[async_trait]
impl Repository<DbPack, DbCreatePack, DbUpdatePack> for PackRepository {
    async fn create(&self, pool: &DbPool, create: DbCreatePack) -> DbResult<DbPack> {
        let uuid = uuid::Uuid::new_v4();
        let now = chrono::Utc::now();

        let id: i64 = sqlx::query_scalar(PackQueryBuilder::insert())
            .bind(uuid)
            .bind(create.project_id)
            .bind(&create.name)
            .bind(&create.version)
            .bind(create.pack_type)
            .bind(&create.description)
            .bind(&create.readme)
            .bind(create.author_id)
            .bind(create.size)
            .bind(create.file_count)
            .bind(create.dependencies)
            .bind(create.metadata)
            .bind(now)
            .bind(now)
            .fetch_one(pool)
            .await?;

        self.find_by_id(pool, id)
            .await?
            .ok_or_else(|| crate::DbError::NotFound {
                entity_type: "Pack".to_string(),
                id: id.to_string(),
            })
    }

    async fn find_by_id(&self, pool: &DbPool, id: i64) -> DbResult<Option<DbPack>> {
        let row = sqlx::query(PackQueryBuilder::select_by_id())
            .bind(id)
            .fetch_optional(pool)
            .await?;

        Ok(row.map(|r| DbPack::from_row(&r)).transpose()?)
    }

    async fn update(&self, pool: &DbPool, id: i64, update: DbUpdatePack) -> DbResult<DbPack> {
        let row = sqlx::query(PackQueryBuilder::update())
            .bind(id)
            .bind(&update.description)
            .bind(&update.readme)
            .bind(update.metadata)
            .fetch_one(pool)
            .await?;

        Ok(DbPack::from_row(&row)?)
    }

    async fn delete(&self, pool: &DbPool, id: i64) -> DbResult<()> {
        sqlx::query(PackQueryBuilder::delete())
            .bind(id)
            .execute(pool)
            .await?;
        Ok(())
    }

    async fn list(&self, pool: &DbPool, limit: i64, offset: i64) -> DbResult<Vec<DbPack>> {
        let rows = sqlx::query(
            "SELECT * FROM packs WHERE deleted_at IS NULL ORDER BY created_at DESC LIMIT $1 OFFSET $2"
        )
        .bind(limit)
        .bind(offset)
        .fetch_all(pool)
        .await?;

        let packs = rows
            .into_iter()
            .map(|r| DbPack::from_row(&r))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(packs)
    }

    async fn count(&self, pool: &DbPool) -> DbResult<i64> {
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM packs WHERE deleted_at IS NULL")
            .fetch_one(pool)
            .await?;
        Ok(count)
    }
}
