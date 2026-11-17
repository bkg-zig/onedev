//! Watch/Subscription Repository

use crate::{
    models::watch::{DbWatch, DbCreateWatch, DbQuerySubscription, WatchQueryBuilder, QuerySubscriptionQueryBuilder},
    repositories::Repository,
    DbPool, DbResult,
};
use async_trait::async_trait;
use sqlx::FromRow;

pub struct WatchRepository;

impl WatchRepository {
    pub fn new() -> Self {
        Self
    }

    /// Find by user ID
    pub async fn find_by_user_id(&self, pool: &DbPool, user_id: i64) -> DbResult<Vec<DbWatch>> {
        let rows = sqlx::query(WatchQueryBuilder::select_by_user_id())
            .bind(user_id)
            .fetch_all(pool)
            .await?;

        let watches = rows.into_iter().map(|r| DbWatch::from_row(&r)).collect::<Result<Vec<_>, _>>()?;
        Ok(watches)
    }

    /// Find by watchable
    pub async fn find_by_watchable(&self, pool: &DbPool, watchable_type: crate::models::watch::DbWatchableType, watchable_id: i64) -> DbResult<Vec<DbWatch>> {
        let rows = sqlx::query(WatchQueryBuilder::select_by_watchable())
            .bind(watchable_type)
            .bind(watchable_id)
            .fetch_all(pool)
            .await?;

        let watches = rows.into_iter().map(|r| DbWatch::from_row(&r)).collect::<Result<Vec<_>, _>>()?;
        Ok(watches)
    }

    /// Find by user and watchable
    pub async fn find_by_user_and_watchable(&self, pool: &DbPool, user_id: i64, watchable_type: crate::models::watch::DbWatchableType, watchable_id: i64) -> DbResult<Option<DbWatch>> {
        let row = sqlx::query(WatchQueryBuilder::select_by_user_and_watchable())
            .bind(user_id)
            .bind(watchable_type)
            .bind(watchable_id)
            .fetch_optional(pool)
            .await?;

        Ok(row.map(|r| DbWatch::from_row(&r)).transpose()?)
    }

    /// Check if user is watching
    pub async fn is_watching(&self, pool: &DbPool, user_id: i64, watchable_type: crate::models::watch::DbWatchableType, watchable_id: i64) -> DbResult<bool> {
        let count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM watches WHERE user_id = $1 AND watchable_type = $2 AND watchable_id = $3"
        )
        .bind(user_id)
        .bind(watchable_type)
        .bind(watchable_id)
        .fetch_one(pool)
        .await?;

        Ok(count > 0)
    }

    /// Delete by user and watchable
    pub async fn delete_by_user_and_watchable(&self, pool: &DbPool, user_id: i64, watchable_type: crate::models::watch::DbWatchableType, watchable_id: i64) -> DbResult<()> {
        sqlx::query(WatchQueryBuilder::delete_by_user_and_watchable())
            .bind(user_id)
            .bind(watchable_type)
            .bind(watchable_id)
            .execute(pool)
            .await?;
        Ok(())
    }
}

#[async_trait]
impl Repository<DbWatch, DbCreateWatch, DbWatch> for WatchRepository {
    async fn create(&self, pool: &DbPool, create: DbCreateWatch) -> DbResult<DbWatch> {
        let uuid = uuid::Uuid::new_v4();
        let now = chrono::Utc::now();

        let id: i64 = sqlx::query_scalar(WatchQueryBuilder::insert())
            .bind(uuid)
            .bind(create.user_id)
            .bind(create.watchable_type)
            .bind(create.watchable_id)
            .bind(now)
            .fetch_one(pool)
            .await?;

        self.find_by_id(pool, id).await?.ok_or_else(|| crate::DbError::NotFound {
            entity_type: "Watch".to_string(),
            id: id.to_string(),
        })
    }

    async fn find_by_id(&self, pool: &DbPool, id: i64) -> DbResult<Option<DbWatch>> {
        let row = sqlx::query(WatchQueryBuilder::select_by_id())
            .bind(id)
            .fetch_optional(pool)
            .await?;

        Ok(row.map(|r| DbWatch::from_row(&r)).transpose()?)
    }

    async fn update(&self, pool: &DbPool, id: i64, _update: DbWatch) -> DbResult<DbWatch> {
        // Watches are immutable, just return existing
        self.find_by_id(pool, id).await?.ok_or_else(|| crate::DbError::NotFound {
            entity_type: "Watch".to_string(),
            id: id.to_string(),
        })
    }

    async fn delete(&self, pool: &DbPool, id: i64) -> DbResult<()> {
        sqlx::query(WatchQueryBuilder::delete())
            .bind(id)
            .execute(pool)
            .await?;
        Ok(())
    }

    async fn list(&self, pool: &DbPool, limit: i64, offset: i64) -> DbResult<Vec<DbWatch>> {
        let rows = sqlx::query(
            "SELECT * FROM watches ORDER BY created_at DESC LIMIT $1 OFFSET $2"
        )
        .bind(limit)
        .bind(offset)
        .fetch_all(pool)
        .await?;

        let watches = rows.into_iter().map(|r| DbWatch::from_row(&r)).collect::<Result<Vec<_>, _>>()?;
        Ok(watches)
    }

    async fn count(&self, pool: &DbPool) -> DbResult<i64> {
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM watches")
            .fetch_one(pool)
            .await?;
        Ok(count)
    }
}

pub struct QuerySubscriptionRepository;

impl QuerySubscriptionRepository {
    pub fn new() -> Self {
        Self
    }

    /// Find by user ID
    pub async fn find_by_user_id(&self, pool: &DbPool, user_id: i64) -> DbResult<Vec<DbQuerySubscription>> {
        let rows = sqlx::query(QuerySubscriptionQueryBuilder::select_by_user_id())
            .bind(user_id)
            .fetch_all(pool)
            .await?;

        let subs = rows.into_iter().map(|r| DbQuerySubscription::from_row(&r)).collect::<Result<Vec<_>, _>>()?;
        Ok(subs)
    }

    /// Find by user and query
    pub async fn find_by_user_and_query(&self, pool: &DbPool, user_id: i64, query_name: &str, query_type: crate::models::watch::DbQueryType) -> DbResult<Option<DbQuerySubscription>> {
        let row = sqlx::query(QuerySubscriptionQueryBuilder::select_by_user_and_query())
            .bind(user_id)
            .bind(query_name)
            .bind(query_type)
            .fetch_optional(pool)
            .await?;

        Ok(row.map(|r| DbQuerySubscription::from_row(&r)).transpose()?)
    }
}

#[async_trait]
impl Repository<DbQuerySubscription, DbQuerySubscription, DbQuerySubscription> for QuerySubscriptionRepository {
    async fn create(&self, pool: &DbPool, create: DbQuerySubscription) -> DbResult<DbQuerySubscription> {
        let uuid = uuid::Uuid::new_v4();
        let now = chrono::Utc::now();

        let id: i64 = sqlx::query_scalar(QuerySubscriptionQueryBuilder::insert())
            .bind(uuid)
            .bind(create.user_id)
            .bind(&create.query_name)
            .bind(create.query_type)
            .bind(create.project_id)
            .bind(now)
            .fetch_one(pool)
            .await?;

        self.find_by_id(pool, id).await?.ok_or_else(|| crate::DbError::NotFound {
            entity_type: "QuerySubscription".to_string(),
            id: id.to_string(),
        })
    }

    async fn find_by_id(&self, pool: &DbPool, id: i64) -> DbResult<Option<DbQuerySubscription>> {
        let row = sqlx::query("SELECT * FROM query_subscriptions WHERE id = $1")
            .bind(id)
            .fetch_optional(pool)
            .await?;

        Ok(row.map(|r| DbQuerySubscription::from_row(&r)).transpose()?)
    }

    async fn update(&self, pool: &DbPool, id: i64, _update: DbQuerySubscription) -> DbResult<DbQuerySubscription> {
        self.find_by_id(pool, id).await?.ok_or_else(|| crate::DbError::NotFound {
            entity_type: "QuerySubscription".to_string(),
            id: id.to_string(),
        })
    }

    async fn delete(&self, pool: &DbPool, id: i64) -> DbResult<()> {
        sqlx::query(QuerySubscriptionQueryBuilder::delete())
            .bind(id)
            .execute(pool)
            .await?;
        Ok(())
    }

    async fn list(&self, pool: &DbPool, limit: i64, offset: i64) -> DbResult<Vec<DbQuerySubscription>> {
        let rows = sqlx::query("SELECT * FROM query_subscriptions ORDER BY created_at DESC LIMIT $1 OFFSET $2")
            .bind(limit)
            .bind(offset)
            .fetch_all(pool)
            .await?;

        let subs = rows.into_iter().map(|r| DbQuerySubscription::from_row(&r)).collect::<Result<Vec<_>, _>>()?;
        Ok(subs)
    }

    async fn count(&self, pool: &DbPool) -> DbResult<i64> {
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM query_subscriptions")
            .fetch_one(pool)
            .await?;
        Ok(count)
    }
}