//! Audit Log Repository

use crate::{
    models::audit::{DbAuditLog, DbCreateAuditLog, AuditLogQueryBuilder},
    repositories::Repository,
    DbPool, DbResult,
};
use async_trait::async_trait;
use sqlx::FromRow;

pub struct AuditLogRepository;

impl AuditLogRepository {
    pub fn new() -> Self {
        Self
    }

    /// Find by user ID
    pub async fn find_by_user_id(&self, pool: &DbPool, user_id: i64, limit: i64, offset: i64) -> DbResult<Vec<DbAuditLog>> {
        let rows = sqlx::query(AuditLogQueryBuilder::select_by_user_id())
            .bind(user_id)
            .bind(limit)
            .bind(offset)
            .fetch_all(pool)
            .await?;

        let logs = rows.into_iter().map(|r| DbAuditLog::from_row(&r)).collect::<Result<Vec<_>, _>>()?;
        Ok(logs)
    }

    /// Find by entity
    pub async fn find_by_entity(&self, pool: &DbPool, entity_type: &str, entity_id: i64, limit: i64, offset: i64) -> DbResult<Vec<DbAuditLog>> {
        let rows = sqlx::query(AuditLogQueryBuilder::select_by_entity())
            .bind(entity_type)
            .bind(entity_id)
            .bind(limit)
            .bind(offset)
            .fetch_all(pool)
            .await?;

        let logs = rows.into_iter().map(|r| DbAuditLog::from_row(&r)).collect::<Result<Vec<_>, _>>()?;
        Ok(logs)
    }

    /// Find by action
    pub async fn find_by_action(&self, pool: &DbPool, action: &str, limit: i64, offset: i64) -> DbResult<Vec<DbAuditLog>> {
        let rows = sqlx::query(AuditLogQueryBuilder::select_by_action())
            .bind(action)
            .bind(limit)
            .bind(offset)
            .fetch_all(pool)
            .await?;

        let logs = rows.into_iter().map(|r| DbAuditLog::from_row(&r)).collect::<Result<Vec<_>, _>>()?;
        Ok(logs)
    }

    /// Find by date range
    pub async fn find_by_date_range(&self, pool: &DbPool, start: chrono::DateTime<chrono::Utc>, end: chrono::DateTime<chrono::Utc>, limit: i64, offset: i64) -> DbResult<Vec<DbAuditLog>> {
        let rows = sqlx::query(AuditLogQueryBuilder::select_by_date_range())
            .bind(start)
            .bind(end)
            .bind(limit)
            .bind(offset)
            .fetch_all(pool)
            .await?;

        let logs = rows.into_iter().map(|r| DbAuditLog::from_row(&r)).collect::<Result<Vec<_>, _>>()?;
        Ok(logs)
    }

    /// Delete old logs
    pub async fn delete_old(&self, pool: &DbPool, before: chrono::DateTime<chrono::Utc>) -> DbResult<u64> {
        let result = sqlx::query(AuditLogQueryBuilder::delete_old())
            .bind(before)
            .execute(pool)
            .await?;
        Ok(result.rows_affected())
    }
}

#[async_trait]
impl Repository<DbAuditLog, DbCreateAuditLog, DbAuditLog> for AuditLogRepository {
    async fn create(&self, pool: &DbPool, create: DbCreateAuditLog) -> DbResult<DbAuditLog> {
        let uuid = uuid::Uuid::new_v4();
        let now = chrono::Utc::now();

        let id: i64 = sqlx::query_scalar(AuditLogQueryBuilder::insert())
            .bind(uuid)
            .bind(create.user_id)
            .bind(&create.action)
            .bind(&create.entity_type)
            .bind(create.entity_id)
            .bind(&create.entity_ref)
            .bind(&create.old_value)
            .bind(&create.new_value)
            .bind(&create.ip_address)
            .bind(&create.user_agent)
            .bind(create.request_id)
            .bind(now)
            .fetch_one(pool)
            .await?;

        self.find_by_id(pool, id).await?.ok_or_else(|| crate::DbError::NotFound {
            entity_type: "AuditLog".to_string(),
            id: id.to_string(),
        })
    }

    async fn find_by_id(&self, pool: &DbPool, id: i64) -> DbResult<Option<DbAuditLog>> {
        let row = sqlx::query(AuditLogQueryBuilder::select_by_id())
            .bind(id)
            .fetch_optional(pool)
            .await?;

        Ok(row.map(|r| DbAuditLog::from_row(&r)).transpose()?)
    }

    async fn update(&self, pool: &DbPool, id: i64, update: DbAuditLog) -> DbResult<DbAuditLog> {
        // Audit logs are immutable
        self.find_by_id(pool, id).await?.ok_or_else(|| crate::DbError::NotFound {
            entity_type: "AuditLog".to_string(),
            id: id.to_string(),
        })
    }

    async fn delete(&self, pool: &DbPool, id: i64) -> DbResult<()> {
        sqlx::query("DELETE FROM audit_logs WHERE id = $1")
            .bind(id)
            .execute(pool)
            .await?;
        Ok(())
    }

    async fn list(&self, pool: &DbPool, limit: i64, offset: i64) -> DbResult<Vec<DbAuditLog>> {
        let rows = sqlx::query(
            "SELECT * FROM audit_logs ORDER BY created_at DESC LIMIT $1 OFFSET $2"
        )
        .bind(limit)
        .bind(offset)
        .fetch_all(pool)
        .await?;

        let logs = rows.into_iter().map(|r| DbAuditLog::from_row(&r)).collect::<Result<Vec<_>, _>>()?;
        Ok(logs)
    }

    async fn count(&self, pool: &DbPool) -> DbResult<i64> {
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM audit_logs")
            .fetch_one(pool)
            .await?;
        Ok(count)
    }
}