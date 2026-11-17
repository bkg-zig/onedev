//! SSH Key Repository

use crate::{
    models::ssh_key::{DbSshKey, DbCreateSshKey, DbUpdateSshKey, SshKeyQueryBuilder},
    repositories::Repository,
    DbPool, DbResult,
};
use async_trait::async_trait;
use sqlx::FromRow;

pub struct SshKeyRepository;

impl SshKeyRepository {
    pub fn new() -> Self {
        Self
    }

    /// Find by user ID
    pub async fn find_by_user_id(&self, pool: &DbPool, user_id: i64) -> DbResult<Vec<DbSshKey>> {
        let rows = sqlx::query(SshKeyQueryBuilder::select_by_user_id())
            .bind(user_id)
            .fetch_all(pool)
            .await?;

        let keys = rows.into_iter().map(|r| DbSshKey::from_row(&r)).collect::<Result<Vec<_>, _>>()?;
        Ok(keys)
    }

    /// Find by fingerprint
    pub async fn find_by_fingerprint(&self, pool: &DbPool, fingerprint: &str) -> DbResult<Option<DbSshKey>> {
        let row = sqlx::query(SshKeyQueryBuilder::select_by_fingerprint())
            .bind(fingerprint)
            .fetch_optional(pool)
            .await?;

        Ok(row.map(|r| DbSshKey::from_row(&r)).transpose()?)
    }

    /// Update last used
    pub async fn update_last_used(&self, pool: &DbPool, id: i64) -> DbResult<()> {
        sqlx::query(SshKeyQueryBuilder::update_last_used())
            .bind(id)
            .execute(pool)
            .await?;
        Ok(())
    }
}

#[async_trait]
impl Repository<DbSshKey, DbCreateSshKey, DbUpdateSshKey> for SshKeyRepository {
    async fn create(&self, pool: &DbPool, create: DbCreateSshKey) -> DbResult<DbSshKey> {
        let uuid = uuid::Uuid::new_v4();
        let now = chrono::Utc::now();

        let id: i64 = sqlx::query_scalar(SshKeyQueryBuilder::insert())
            .bind(uuid)
            .bind(create.user_id)
            .bind(&create.title)
            .bind(&create.public_key)
            .bind(&create.fingerprint_sha256)
            .bind(&create.fingerprint_md5)
            .bind(create.key_type)
            .bind(create.bits)
            .bind(create.expires_at)
            .bind(now)
            .bind(now)
            .fetch_one(pool)
            .await?;

        self.find_by_id(pool, id).await?.ok_or_else(|| crate::DbError::NotFound {
            entity_type: "SshKey".to_string(),
            id: id.to_string(),
        })
    }

    async fn find_by_id(&self, pool: &DbPool, id: i64) -> DbResult<Option<DbSshKey>> {
        let row = sqlx::query(SshKeyQueryBuilder::select_by_id())
            .bind(id)
            .fetch_optional(pool)
            .await?;

        Ok(row.map(|r| DbSshKey::from_row(&r)).transpose()?)
    }

    async fn update(&self, pool: &DbPool, id: i64, update: DbUpdateSshKey) -> DbResult<DbSshKey> {
        let row = sqlx::query(SshKeyQueryBuilder::update())
            .bind(id)
            .bind(&update.title)
            .bind(update.expires_at)
            .fetch_one(pool)
            .await?;

        Ok(DbSshKey::from_row(&row)?)
    }

    async fn delete(&self, pool: &DbPool, id: i64) -> DbResult<()> {
        sqlx::query(SshKeyQueryBuilder::delete())
            .bind(id)
            .execute(pool)
            .await?;
        Ok(())
    }

    async fn list(&self, pool: &DbPool, limit: i64, offset: i64) -> DbResult<Vec<DbSshKey>> {
        let rows = sqlx::query(
            "SELECT * FROM ssh_keys ORDER BY created_at DESC LIMIT $1 OFFSET $2"
        )
        .bind(limit)
        .bind(offset)
        .fetch_all(pool)
        .await?;

        let keys = rows.into_iter().map(|r| DbSshKey::from_row(&r)).collect::<Result<Vec<_>, _>>()?;
        Ok(keys)
    }

    async fn count(&self, pool: &DbPool) -> DbResult<i64> {
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM ssh_keys")
            .fetch_one(pool)
            .await?;
        Ok(count)
    }
}