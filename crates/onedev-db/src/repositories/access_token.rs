//! Access Token Repository

use crate::{
    models::access_token::{DbAccessToken, DbCreateAccessToken, DbUpdateAccessToken, AccessTokenQueryBuilder},
    repositories::Repository,
    DbPool, DbResult,
};
use async_trait::async_trait;

pub struct AccessTokenRepository;

impl AccessTokenRepository {
    pub fn new() -> Self {
        Self
    }

    /// Find by token hash
    pub async fn find_by_token_hash(&self, pool: &DbPool, token_hash: &str) -> DbResult<Option<DbAccessToken>> {
        let row = sqlx::query(AccessTokenQueryBuilder::select_by_token_hash())
            .bind(token_hash)
            .fetch_optional(pool)
            .await?;

        Ok(row.map(|r| DbAccessToken::from_row(&r)).transpose()?)
    }

    /// Find by user ID
    pub async fn find_by_user_id(&self, pool: &DbPool, user_id: i64) -> DbResult<Vec<DbAccessToken>> {
        let rows = sqlx::query(AccessTokenQueryBuilder::select_by_user_id())
            .bind(user_id)
            .fetch_all(pool)
            .await?;

        let tokens = rows.into_iter().map(|r| DbAccessToken::from_row(&r)).collect::<Result<Vec<_>, _>>()?;
        Ok(tokens)
    }

    /// Update last used timestamp
    pub async fn update_last_used(&self, pool: &DbPool, id: i64) -> DbResult<()> {
        sqlx::query(AccessTokenQueryBuilder::update_last_used())
            .bind(id)
            .execute(pool)
            .await?;
        Ok(())
    }

    /// Revoke token
    pub async fn revoke(&self, pool: &DbPool, id: i64) -> DbResult<()> {
        sqlx::query("UPDATE access_tokens SET revoked_at = NOW(), updated_at = NOW() WHERE id = $1")
            .bind(id)
            .execute(pool)
            .await?;
        Ok(())
    }

    /// Clean up expired tokens
    pub async fn cleanup_expired(&self, pool: &DbPool) -> DbResult<u64> {
        let result = sqlx::query(
            "UPDATE access_tokens SET revoked_at = NOW(), updated_at = NOW() WHERE expires_at < NOW() AND revoked_at IS NULL"
        )
        .execute(pool)
        .await?;
        Ok(result.rows_affected())
    }
}

#[async_trait]
impl Repository<DbAccessToken, DbCreateAccessToken, DbUpdateAccessToken> for AccessTokenRepository {
    async fn create(&self, pool: &DbPool, create: DbCreateAccessToken) -> DbResult<DbAccessToken> {
        let uuid = uuid::Uuid::new_v4();
        let now = chrono::Utc::now();

        let id: i64 = sqlx::query_scalar(AccessTokenQueryBuilder::insert())
            .bind(uuid)
            .bind(create.user_id)
            .bind(&create.name)
            .bind(&create.token_hash)
            .bind(&create.scopes)
            .bind(create.expires_at)
            .bind(now)
            .bind(now)
            .fetch_one(pool)
            .await?;

        self.find_by_id(pool, id).await?.ok_or_else(|| crate::DbError::NotFound {
            entity_type: "AccessToken".to_string(),
            id: id.to_string(),
        })
    }

    async fn find_by_id(&self, pool: &DbPool, id: i64) -> DbResult<Option<DbAccessToken>> {
        let row = sqlx::query(AccessTokenQueryBuilder::select_by_id())
            .bind(id)
            .fetch_optional(pool)
            .await?;

        Ok(row.map(|r| DbAccessToken::from_row(&r)).transpose()?)
    }

    async fn update(&self, pool: &DbPool, id: i64, update: DbUpdateAccessToken) -> DbResult<DbAccessToken> {
        let row = sqlx::query(AccessTokenQueryBuilder::update())
            .bind(id)
            .bind(&update.name)
            .bind(update.scopes.as_ref().map(|v| v.as_slice()))
            .bind(update.expires_at)
            .bind(update.revoked_at)
            .fetch_one(pool)
            .await?;

        DbAccessToken::from_row(&row)
    }

    async fn delete(&self, pool: &DbPool, id: i64) -> DbResult<()> {
        sqlx::query(AccessTokenQueryBuilder::delete())
            .bind(id)
            .execute(pool)
            .await?;
        Ok(())
    }

    async fn list(&self, pool: &DbPool, limit: i64, offset: i64) -> DbResult<Vec<DbAccessToken>> {
        let rows = sqlx::query(
            "SELECT * FROM access_tokens ORDER BY created_at DESC LIMIT $1 OFFSET $2"
        )
        .bind(limit)
        .bind(offset)
        .fetch_all(pool)
        .await?;

        let tokens = rows.into_iter().map(|r| DbAccessToken::from_row(&r)).collect::<Result<Vec<_>, _>>()?;
        Ok(tokens)
    }

    async fn count(&self, pool: &DbPool) -> DbResult<i64> {
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM access_tokens")
            .fetch_one(pool)
            .await?;
        Ok(count)
    }
}