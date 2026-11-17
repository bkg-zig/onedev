//! Email Address Repository

use crate::{
    models::email::{DbEmailAddress, DbCreateEmailAddress, DbUpdateEmailAddress, EmailAddressQueryBuilder},
    repositories::Repository,
    DbPool, DbResult,
};
use async_trait::async_trait;
use sqlx::FromRow;

pub struct EmailAddressRepository;

impl EmailAddressRepository {
    pub fn new() -> Self {
        Self
    }

    /// Find by user ID
    pub async fn find_by_user_id(&self, pool: &DbPool, user_id: i64) -> DbResult<Vec<DbEmailAddress>> {
        let rows = sqlx::query(EmailAddressQueryBuilder::select_by_user_id())
            .bind(user_id)
            .fetch_all(pool)
            .await?;

        let emails = rows.into_iter().map(|r| DbEmailAddress::from_row(&r)).collect::<Result<Vec<_>, _>>()?;
        Ok(emails)
    }

    /// Find by value
    pub async fn find_by_value(&self, pool: &DbPool, value: &str) -> DbResult<Option<DbEmailAddress>> {
        let row = sqlx::query(EmailAddressQueryBuilder::select_by_value())
            .bind(value)
            .fetch_optional(pool)
            .await?;

        Ok(row.map(|r| DbEmailAddress::from_row(&r)).transpose()?)
    }

    /// Find primary by user ID
    pub async fn find_primary_by_user_id(&self, pool: &DbPool, user_id: i64) -> DbResult<Option<DbEmailAddress>> {
        let row = sqlx::query(EmailAddressQueryBuilder::select_primary_by_user_id())
            .bind(user_id)
            .fetch_optional(pool)
            .await?;

        Ok(row.map(|r| DbEmailAddress::from_row(&r)).transpose()?)
    }

    /// Set primary email
    pub async fn set_primary(&self, pool: &DbPool, user_id: i64, email_id: i64) -> DbResult<()> {
        let mut tx = pool.begin().await?;

        // Unset current primary
        sqlx::query(r#"UPDATE email_addresses SET "primary" = FALSE, updated_at = NOW() WHERE user_id = $1 AND "primary" = TRUE"#)
            .bind(user_id)
            .execute(&mut *tx)
            .await?;

        // Set new primary
        sqlx::query(r#"UPDATE email_addresses SET "primary" = TRUE, updated_at = NOW() WHERE id = $1 AND user_id = $2"#)
            .bind(email_id)
            .bind(user_id)
            .execute(&mut *tx)
            .await?;

        tx.commit().await?;
        Ok(())
    }

    /// Verify email
    pub async fn verify(&self, pool: &DbPool, id: i64) -> DbResult<()> {
        sqlx::query("UPDATE email_addresses SET verified = TRUE, verification_code = NULL, updated_at = NOW() WHERE id = $1")
            .bind(id)
            .execute(pool)
            .await?;
        Ok(())
    }
}

#[async_trait]
impl Repository<DbEmailAddress, DbCreateEmailAddress, DbUpdateEmailAddress> for EmailAddressRepository {
    async fn create(&self, pool: &DbPool, create: DbCreateEmailAddress) -> DbResult<DbEmailAddress> {
        let uuid = uuid::Uuid::new_v4();
        let now = chrono::Utc::now();

        let id: i64 = sqlx::query_scalar(EmailAddressQueryBuilder::insert())
            .bind(uuid)
            .bind(create.user_id)
            .bind(&create.value)
            .bind(false) // verified
            .bind(create.primary)
            .bind(&create.verification_code)
            .bind(Option::<chrono::DateTime<chrono::Utc>>::None) // verification_sent_at
            .bind(now)
            .bind(now)
            .fetch_one(pool)
            .await?;

        self.find_by_id(pool, id).await?.ok_or_else(|| crate::DbError::NotFound {
            entity_type: "EmailAddress".to_string(),
            id: id.to_string(),
        })
    }

    async fn find_by_id(&self, pool: &DbPool, id: i64) -> DbResult<Option<DbEmailAddress>> {
        let row = sqlx::query(EmailAddressQueryBuilder::select_by_id())
            .bind(id)
            .fetch_optional(pool)
            .await?;

        Ok(row.map(|r| DbEmailAddress::from_row(&r)).transpose()?)
    }

    async fn update(&self, pool: &DbPool, id: i64, update: DbUpdateEmailAddress) -> DbResult<DbEmailAddress> {
        let row = sqlx::query(EmailAddressQueryBuilder::update())
            .bind(id)
            .bind(update.verified)
            .bind(update.primary)
            .bind(&update.verification_code)
            .bind(update.verification_sent_at)
            .fetch_one(pool)
            .await?;

        Ok(DbEmailAddress::from_row(&row)?)
    }

    async fn delete(&self, pool: &DbPool, id: i64) -> DbResult<()> {
        sqlx::query(EmailAddressQueryBuilder::delete())
            .bind(id)
            .execute(pool)
            .await?;
        Ok(())
    }

    async fn list(&self, pool: &DbPool, limit: i64, offset: i64) -> DbResult<Vec<DbEmailAddress>> {
        let rows = sqlx::query(
            "SELECT * FROM email_addresses ORDER BY created_at DESC LIMIT $1 OFFSET $2"
        )
        .bind(limit)
        .bind(offset)
        .fetch_all(pool)
        .await?;

        let emails = rows.into_iter().map(|r| DbEmailAddress::from_row(&r)).collect::<Result<Vec<_>, _>>()?;
        Ok(emails)
    }

    async fn count(&self, pool: &DbPool) -> DbResult<i64> {
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM email_addresses")
            .fetch_one(pool)
            .await?;
        Ok(count)
    }
}