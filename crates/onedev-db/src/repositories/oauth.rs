//! OAuth Repository

use crate::{
    models::oauth::{DbOAuthApplication, DbCreateOAuthApplication, DbUpdateOAuthApplication, DbOAuthGrant, DbCreateOAuthGrant, DbUpdateOAuthGrant, OAuthApplicationQueryBuilder, OAuthGrantQueryBuilder},
    repositories::Repository,
    DbPool, DbResult,
};
use async_trait::async_trait;

pub struct OAuthApplicationRepository;

impl OAuthApplicationRepository {
    pub fn new() -> Self {
        Self
    }

    /// Find by client ID
    pub async fn find_by_client_id(&self, pool: &DbPool, client_id: &str) -> DbResult<Option<DbOAuthApplication>> {
        let row = sqlx::query(OAuthApplicationQueryBuilder::select_by_client_id())
            .bind(client_id)
            .fetch_optional(pool)
            .await?;

        Ok(row.map(|r| DbOAuthApplication::from_row(&r)).transpose()?)
    }

    /// Find by owner
    pub async fn find_by_owner(&self, pool: &DbPool, owner_id: i64, owner_type: crate::models::oauth::DbOAuthOwnerType) -> DbResult<Vec<DbOAuthApplication>> {
        let rows = sqlx::query(OAuthApplicationQueryBuilder::select_by_owner())
            .bind(owner_id)
            .bind(owner_type)
            .fetch_all(pool)
            .await?;

        let apps = rows.into_iter().map(|r| DbOAuthApplication::from_row(&r)).collect::<Result<Vec<_>, _>>()?;
        Ok(apps)
    }
}

#[async_trait]
impl Repository<DbOAuthApplication, DbCreateOAuthApplication, DbUpdateOAuthApplication> for OAuthApplicationRepository {
    async fn create(&self, pool: &DbPool, create: DbCreateOAuthApplication) -> DbResult<DbOAuthApplication> {
        let uuid = uuid::Uuid::new_v4();
        let now = chrono::Utc::now();

        let id: i64 = sqlx::query_scalar(OAuthApplicationQueryBuilder::insert())
            .bind(uuid)
            .bind(create.owner_id)
            .bind(create.owner_type)
            .bind(&create.name)
            .bind(&create.client_id)
            .bind(&create.client_secret_hash)
            .bind(&create.redirect_uris)
            .bind(&create.scopes)
            .bind(create.confidential)
            .bind(now)
            .bind(now)
            .fetch_one(pool)
            .await?;

        self.find_by_id(pool, id).await?.ok_or_else(|| crate::DbError::NotFound {
            entity_type: "OAuthApplication".to_string(),
            id: id.to_string(),
        })
    }

    async fn find_by_id(&self, pool: &DbPool, id: i64) -> DbResult<Option<DbOAuthApplication>> {
        let row = sqlx::query(OAuthApplicationQueryBuilder::select_by_id())
            .bind(id)
            .fetch_optional(pool)
            .await?;

        Ok(row.map(|r| DbOAuthApplication::from_row(&r)).transpose()?)
    }

    async fn update(&self, pool: &DbPool, id: i64, update: DbUpdateOAuthApplication) -> DbResult<DbOAuthApplication> {
        let row = sqlx::query(OAuthApplicationQueryBuilder::update())
            .bind(id)
            .bind(&update.name)
            .bind(update.redirect_uris.as_ref().map(|v| v.as_slice()))
            .bind(update.scopes.as_ref().map(|v| v.as_slice()))
            .bind(update.confidential)
            .fetch_one(pool)
            .await?;

        DbOAuthApplication::from_row(&row)
    }

    async fn delete(&self, pool: &DbPool, id: i64) -> DbResult<()> {
        sqlx::query(OAuthApplicationQueryBuilder::delete())
            .bind(id)
            .execute(pool)
            .await?;
        Ok(())
    }

    async fn list(&self, pool: &DbPool, limit: i64, offset: i64) -> DbResult<Vec<DbOAuthApplication>> {
        let rows = sqlx::query(
            "SELECT * FROM oauth_applications ORDER BY created_at DESC LIMIT $1 OFFSET $2"
        )
        .bind(limit)
        .bind(offset)
        .fetch_all(pool)
        .await?;

        let apps = rows.into_iter().map(|r| DbOAuthApplication::from_row(&r)).collect::<Result<Vec<_>, _>>()?;
        Ok(apps)
    }

    async fn count(&self, pool: &DbPool) -> DbResult<i64> {
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM oauth_applications")
            .fetch_one(pool)
            .await?;
        Ok(count)
    }
}

pub struct OAuthGrantRepository;

impl OAuthGrantRepository {
    pub fn new() -> Self {
        Self
    }

    /// Find by code hash
    pub async fn find_by_code_hash(&self, pool: &DbPool, code_hash: &str) -> DbResult<Option<DbOAuthGrant>> {
        let row = sqlx::query(OAuthGrantQueryBuilder::select_by_code_hash())
            .bind(code_hash)
            .fetch_optional(pool)
            .await?;

        Ok(row.map(|r| DbOAuthGrant::from_row(&r)).transpose()?)
    }

    /// Find by access token hash
    pub async fn find_by_access_token_hash(&self, pool: &DbPool, access_token_hash: &str) -> DbResult<Option<DbOAuthGrant>> {
        let row = sqlx::query(OAuthGrantQueryBuilder::select_by_access_token_hash())
            .bind(access_token_hash)
            .fetch_optional(pool)
            .await?;

        Ok(row.map(|r| DbOAuthGrant::from_row(&r)).transpose()?)
    }

    /// Find by refresh token hash
    pub async fn find_by_refresh_token_hash(&self, pool: &DbPool, refresh_token_hash: &str) -> DbResult<Option<DbOAuthGrant>> {
        let row = sqlx::query(OAuthGrantQueryBuilder::select_by_refresh_token_hash())
            .bind(refresh_token_hash)
            .fetch_optional(pool)
            .await?;

        Ok(row.map(|r| DbOAuthGrant::from_row(&r)).transpose()?)
    }

    /// Find by application and user
    pub async fn find_by_application_and_user(&self, pool: &DbPool, application_id: i64, user_id: i64) -> DbResult<Vec<DbOAuthGrant>> {
        let rows = sqlx::query(OAuthGrantQueryBuilder::select_by_application_and_user())
            .bind(application_id)
            .bind(user_id)
            .fetch_all(pool)
            .await?;

        let grants = rows.into_iter().map(|r| DbOAuthGrant::from_row(&r)).collect::<Result<Vec<_>, _>>()?;
        Ok(grants)
    }

    /// Delete expired grants
    pub async fn delete_expired(&self, pool: &DbPool) -> DbResult<u64> {
        let result = sqlx::query(
            "DELETE FROM oauth_grants WHERE code_expires_at < NOW() AND access_token_expires_at < NOW()"
        )
        .execute(pool)
        .await?;
        Ok(result.rows_affected())
    }
}

#[async_trait]
impl Repository<DbOAuthGrant, DbCreateOAuthGrant, DbUpdateOAuthGrant> for OAuthGrantRepository {
    async fn create(&self, pool: &DbPool, create: DbCreateOAuthGrant) -> DbResult<DbOAuthGrant> {
        let uuid = uuid::Uuid::new_v4();
        let now = chrono::Utc::now();

        let id: i64 = sqlx::query_scalar(OAuthGrantQueryBuilder::insert())
            .bind(uuid)
            .bind(create.application_id)
            .bind(create.user_id)
            .bind(&create.scopes)
            .bind(&create.redirect_uri)
            .bind(&create.code_hash)
            .bind(create.code_expires_at)
            .bind(now)
            .bind(now)
            .fetch_one(pool)
            .await?;

        self.find_by_id(pool, id).await?.ok_or_else(|| crate::DbError::NotFound {
            entity_type: "OAuthGrant".to_string(),
            id: id.to_string(),
        })
    }

    async fn find_by_id(&self, pool: &DbPool, id: i64) -> DbResult<Option<DbOAuthGrant>> {
        let row = sqlx::query(OAuthGrantQueryBuilder::select_by_id())
            .bind(id)
            .fetch_optional(pool)
            .await?;

        Ok(row.map(|r| DbOAuthGrant::from_row(&r)).transpose()?)
    }

    async fn update(&self, pool: &DbPool, id: i64, update: DbUpdateOAuthGrant) -> DbResult<DbOAuthGrant> {
        let row = sqlx::query(OAuthGrantQueryBuilder::update())
            .bind(id)
            .bind(&update.code)
            .bind(&update.code_hash)
            .bind(&update.access_token_hash)
            .bind(&update.refresh_token_hash)
            .bind(update.access_token_expires_at)
            .fetch_one(pool)
            .await?;

        DbOAuthGrant::from_row(&row)
    }

    async fn delete(&self, pool: &DbPool, id: i64) -> DbResult<()> {
        sqlx::query(OAuthGrantQueryBuilder::delete())
            .bind(id)
            .execute(pool)
            .await?;
        Ok(())
    }

    async fn list(&self, pool: &DbPool, limit: i64, offset: i64) -> DbResult<Vec<DbOAuthGrant>> {
        let rows = sqlx::query(
            "SELECT * FROM oauth_grants ORDER BY created_at DESC LIMIT $1 OFFSET $2"
        )
        .bind(limit)
        .bind(offset)
        .fetch_all(pool)
        .await?;

        let grants = rows.into_iter().map(|r| DbOAuthGrant::from_row(&r)).collect::<Result<Vec<_>, _>>()?;
        Ok(grants)
    }

    async fn count(&self, pool: &DbPool) -> DbResult<i64> {
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM oauth_grants")
            .fetch_one(pool)
            .await?;
        Ok(count)
    }
}