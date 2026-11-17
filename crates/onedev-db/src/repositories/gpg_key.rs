//! GPG Key Repository

use crate::{
    models::gpg_key::{DbGpgKey, DbCreateGpgKey, DbUpdateGpgKey, GpgKeyQueryBuilder},
    repositories::Repository,
    DbPool, DbResult,
};
use async_trait::async_trait;
use sqlx::FromRow;

pub struct GpgKeyRepository;

impl GpgKeyRepository {
    pub fn new() -> Self {
        Self
    }

    /// Find by user ID
    pub async fn find_by_user_id(&self, pool: &DbPool, user_id: i64) -> DbResult<Vec<DbGpgKey>> {
        let rows = sqlx::query(GpgKeyQueryBuilder::select_by_user_id())
            .bind(user_id)
            .fetch_all(pool)
            .await?;

        let keys = rows.into_iter().map(|r| DbGpgKey::from_row(&r)).collect::<Result<Vec<_>, _>>()?;
        Ok(keys)
    }

    /// Find by key ID
    pub async fn find_by_key_id(&self, pool: &DbPool, key_id: &str) -> DbResult<Option<DbGpgKey>> {
        let row = sqlx::query(GpgKeyQueryBuilder::select_by_key_id())
            .bind(key_id)
            .fetch_optional(pool)
            .await?;

        Ok(row.map(|r| DbGpgKey::from_row(&r)).transpose()?)
    }
}

#[async_trait]
impl Repository<DbGpgKey, DbCreateGpgKey, DbUpdateGpgKey> for GpgKeyRepository {
    async fn create(&self, pool: &DbPool, create: DbCreateGpgKey) -> DbResult<DbGpgKey> {
        let uuid = uuid::Uuid::new_v4();
        let now = chrono::Utc::now();

        let id: i64 = sqlx::query_scalar(GpgKeyQueryBuilder::insert())
            .bind(uuid)
            .bind(create.user_id)
            .bind(&create.key_id)
            .bind(&create.public_key)
            .bind(&create.fingerprint)
            .bind(&create.email)
            .bind(create.expires_at)
            .bind(now)
            .bind(now)
            .fetch_one(pool)
            .await?;

        self.find_by_id(pool, id).await?.ok_or_else(|| crate::DbError::NotFound {
            entity_type: "GpgKey".to_string(),
            id: id.to_string(),
        })
    }

    async fn find_by_id(&self, pool: &DbPool, id: i64) -> DbResult<Option<DbGpgKey>> {
        let row = sqlx::query(GpgKeyQueryBuilder::select_by_id())
            .bind(id)
            .fetch_optional(pool)
            .await?;

        Ok(row.map(|r| DbGpgKey::from_row(&r)).transpose()?)
    }

    async fn update(&self, pool: &DbPool, id: i64, update: DbUpdateGpgKey) -> DbResult<DbGpgKey> {
        let row = sqlx::query(GpgKeyQueryBuilder::update())
            .bind(id)
            .bind(update.expires_at)
            .fetch_one(pool)
            .await?;

        Ok(DbGpgKey::from_row(&row)?)
    }

    async fn delete(&self, pool: &DbPool, id: i64) -> DbResult<()> {
        sqlx::query(GpgKeyQueryBuilder::delete())
            .bind(id)
            .execute(pool)
            .await?;
        Ok(())
    }

    async fn list(&self, pool: &DbPool, limit: i64, offset: i64) -> DbResult<Vec<DbGpgKey>> {
        let rows = sqlx::query(
            "SELECT * FROM gpg_keys ORDER BY created_at DESC LIMIT $1 OFFSET $2"
        )
        .bind(limit)
        .bind(offset)
        .fetch_all(pool)
        .await?;

        let keys = rows.into_iter().map(|r| DbGpgKey::from_row(&r)).collect::<Result<Vec<_>, _>>()?;
        Ok(keys)
    }

    async fn count(&self, pool: &DbPool) -> DbResult<i64> {
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM gpg_keys")
            .fetch_one(pool)
            .await?;
        Ok(count)
    }
}