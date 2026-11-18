//! Webhook Repository

use crate::{
    DbPool, DbResult,
    models::webhook::{DbCreateWebhook, DbUpdateWebhook, DbWebhook, WebhookQueryBuilder},
    repositories::Repository,
};
use async_trait::async_trait;
use sqlx::FromRow;

pub struct WebhookRepository;

impl WebhookRepository {
    pub fn new() -> Self {
        Self
    }

    /// Find by project ID
    pub async fn find_by_project_id(
        &self,
        pool: &DbPool,
        project_id: i64,
    ) -> DbResult<Vec<DbWebhook>> {
        let rows = sqlx::query(WebhookQueryBuilder::select_by_project_id())
            .bind(project_id)
            .fetch_all(pool)
            .await?;

        let webhooks = rows
            .into_iter()
            .map(|r| DbWebhook::from_row(&r))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(webhooks)
    }

    /// Find system webhooks
    pub async fn find_system_webhooks(&self, pool: &DbPool) -> DbResult<Vec<DbWebhook>> {
        let rows = sqlx::query(WebhookQueryBuilder::select_system_webhooks())
            .fetch_all(pool)
            .await?;

        let webhooks = rows
            .into_iter()
            .map(|r| DbWebhook::from_row(&r))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(webhooks)
    }

    /// Update delivery status
    pub async fn update_delivery_status(
        &self,
        pool: &DbPool,
        id: i64,
        status: i32,
    ) -> DbResult<()> {
        sqlx::query(WebhookQueryBuilder::update_delivery_status())
            .bind(id)
            .bind(status)
            .execute(pool)
            .await?;
        Ok(())
    }
}

#[async_trait]
impl Repository<DbWebhook, DbCreateWebhook, DbUpdateWebhook> for WebhookRepository {
    async fn create(&self, pool: &DbPool, create: DbCreateWebhook) -> DbResult<DbWebhook> {
        let uuid = uuid::Uuid::new_v4();
        let now = chrono::Utc::now();

        let id: i64 = sqlx::query_scalar(WebhookQueryBuilder::insert())
            .bind(uuid)
            .bind(create.project_id)
            .bind(&create.url)
            .bind(&create.secret)
            .bind(sqlx::types::Json(&create.events)) // scopes (JSONB)
            .bind(&create.branch_filter)
            .bind(&create.tag_filter)
            .bind(create.ssl_verification)
            .bind(true) // active
            .bind(now)
            .bind(now)
            .fetch_one(pool)
            .await?;

        self.find_by_id(pool, id)
            .await?
            .ok_or_else(|| crate::DbError::NotFound {
                entity_type: "Webhook".to_string(),
                id: id.to_string(),
            })
    }

    async fn find_by_id(&self, pool: &DbPool, id: i64) -> DbResult<Option<DbWebhook>> {
        let row = sqlx::query(WebhookQueryBuilder::select_by_id())
            .bind(id)
            .fetch_optional(pool)
            .await?;

        Ok(row.map(|r| DbWebhook::from_row(&r)).transpose()?)
    }

    async fn update(&self, pool: &DbPool, id: i64, update: DbUpdateWebhook) -> DbResult<DbWebhook> {
        let row = sqlx::query(WebhookQueryBuilder::update())
            .bind(id)
            .bind(&update.url)
            .bind(&update.secret)
            .bind(update.events.as_ref().map(sqlx::types::Json)) // scopes (JSONB)
            .bind(&update.branch_filter)
            .bind(&update.tag_filter)
            .bind(update.ssl_verification)
            .bind(update.active)
            .fetch_one(pool)
            .await?;

        Ok(DbWebhook::from_row(&row)?)
    }

    async fn delete(&self, pool: &DbPool, id: i64) -> DbResult<()> {
        sqlx::query(WebhookQueryBuilder::delete())
            .bind(id)
            .execute(pool)
            .await?;
        Ok(())
    }

    async fn list(&self, pool: &DbPool, limit: i64, offset: i64) -> DbResult<Vec<DbWebhook>> {
        let rows =
            sqlx::query("SELECT * FROM webhooks ORDER BY created_at DESC LIMIT $1 OFFSET $2")
                .bind(limit)
                .bind(offset)
                .fetch_all(pool)
                .await?;

        let webhooks = rows
            .into_iter()
            .map(|r| DbWebhook::from_row(&r))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(webhooks)
    }

    async fn count(&self, pool: &DbPool) -> DbResult<i64> {
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM webhooks")
            .fetch_one(pool)
            .await?;
        Ok(count)
    }
}
