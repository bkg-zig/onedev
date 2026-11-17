//! SSO Account Repository

use crate::{
    models::sso_account::{DbSsoAccount, DbCreateSsoAccount, DbUpdateSsoAccount, SsoAccountQueryBuilder},
    repositories::Repository,
    DbPool, DbResult,
};
use async_trait::async_trait;
use sqlx::FromRow;

pub struct SsoAccountRepository;

impl SsoAccountRepository {
    pub fn new() -> Self {
        Self
    }

    /// Find by user ID
    pub async fn find_by_user_id(&self, pool: &DbPool, user_id: i64) -> DbResult<Vec<DbSsoAccount>> {
        let rows = sqlx::query(SsoAccountQueryBuilder::select_by_user_id())
            .bind(user_id)
            .fetch_all(pool)
            .await?;

        let accounts = rows.into_iter().map(|r| DbSsoAccount::from_row(&r)).collect::<Result<Vec<_>, _>>()?;
        Ok(accounts)
    }

    /// Find by provider and provider user ID
    pub async fn find_by_provider_and_user_id(&self, pool: &DbPool, provider: &str, provider_user_id: &str) -> DbResult<Option<DbSsoAccount>> {
        let row = sqlx::query(SsoAccountQueryBuilder::select_by_provider_and_user_id())
            .bind(provider)
            .bind(provider_user_id)
            .fetch_optional(pool)
            .await?;

        Ok(row.map(|r| DbSsoAccount::from_row(&r)).transpose()?)
    }
}

#[async_trait]
impl Repository<DbSsoAccount, DbCreateSsoAccount, DbUpdateSsoAccount> for SsoAccountRepository {
    async fn create(&self, pool: &DbPool, create: DbCreateSsoAccount) -> DbResult<DbSsoAccount> {
        let uuid = uuid::Uuid::new_v4();
        let now = chrono::Utc::now();

        let id: i64 = sqlx::query_scalar(SsoAccountQueryBuilder::insert())
            .bind(uuid)
            .bind(create.user_id)
            .bind(&create.provider)
            .bind(&create.provider_user_id)
            .bind(&create.provider_username)
            .bind(&create.provider_email)
            .bind(&create.access_token)
            .bind(&create.refresh_token)
            .bind(create.token_expires_at)
            .bind(create.raw_data)
            .bind(now)
            .bind(now)
            .fetch_one(pool)
            .await?;

        self.find_by_id(pool, id).await?.ok_or_else(|| crate::DbError::NotFound {
            entity_type: "SsoAccount".to_string(),
            id: id.to_string(),
        })
    }

    async fn find_by_id(&self, pool: &DbPool, id: i64) -> DbResult<Option<DbSsoAccount>> {
        let row = sqlx::query(SsoAccountQueryBuilder::select_by_id())
            .bind(id)
            .fetch_optional(pool)
            .await?;

        Ok(row.map(|r| DbSsoAccount::from_row(&r)).transpose()?)
    }

    async fn update(&self, pool: &DbPool, id: i64, update: DbUpdateSsoAccount) -> DbResult<DbSsoAccount> {
        let row = sqlx::query(SsoAccountQueryBuilder::update())
            .bind(id)
            .bind(&update.provider_username)
            .bind(&update.provider_email)
            .bind(&update.access_token)
            .bind(&update.refresh_token)
            .bind(update.token_expires_at)
            .bind(update.raw_data)
            .fetch_one(pool)
            .await?;

        Ok(DbSsoAccount::from_row(&row)?)
    }

    async fn delete(&self, pool: &DbPool, id: i64) -> DbResult<()> {
        sqlx::query(SsoAccountQueryBuilder::delete())
            .bind(id)
            .execute(pool)
            .await?;
        Ok(())
    }

    async fn list(&self, pool: &DbPool, limit: i64, offset: i64) -> DbResult<Vec<DbSsoAccount>> {
        let rows = sqlx::query(
            "SELECT * FROM sso_accounts ORDER BY created_at DESC LIMIT $1 OFFSET $2"
        )
        .bind(limit)
        .bind(offset)
        .fetch_all(pool)
        .await?;

        let accounts = rows.into_iter().map(|r| DbSsoAccount::from_row(&r)).collect::<Result<Vec<_>, _>>()?;
        Ok(accounts)
    }

    async fn count(&self, pool: &DbPool) -> DbResult<i64> {
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM sso_accounts")
            .fetch_one(pool)
            .await?;
        Ok(count)
    }
}