//! Agent Repository

use crate::{
    models::agent::{DbAgent, DbCreateAgent, DbUpdateAgent, AgentQueryBuilder},
    repositories::Repository,
    DbPool, DbResult,
};
use async_trait::async_trait;
use sqlx::FromRow;

pub struct AgentRepository;

impl AgentRepository {
    pub fn new() -> Self {
        Self
    }

    /// Find by token hash
    pub async fn find_by_token_hash(&self, pool: &DbPool, token_hash: &str) -> DbResult<Option<DbAgent>> {
        let row = sqlx::query(AgentQueryBuilder::select_by_token_hash())
            .bind(token_hash)
            .fetch_optional(pool)
            .await?;

        Ok(row.map(|r| DbAgent::from_row(&r)).transpose()?)
    }

    /// Find online agents with capacity
    pub async fn find_online_with_capacity(&self, pool: &DbPool) -> DbResult<Vec<DbAgent>> {
        let rows = sqlx::query(AgentQueryBuilder::select_online())
            .fetch_all(pool)
            .await?;

        let agents = rows.into_iter().map(|r| DbAgent::from_row(&r)).collect::<Result<Vec<_>, _>>()?;
        Ok(agents)
    }

    /// Update agent status and current jobs
    pub async fn update_status(&self, pool: &DbPool, id: i64, status: crate::models::agent::DbAgentStatus, current_jobs: i32) -> DbResult<()> {
        sqlx::query(AgentQueryBuilder::update_status())
            .bind(id)
            .bind(status)
            .bind(current_jobs)
            .execute(pool)
            .await?;
        Ok(())
    }

    /// Increment current jobs
    pub async fn increment_jobs(&self, pool: &DbPool, id: i64) -> DbResult<()> {
        sqlx::query("UPDATE agents SET current_jobs = current_jobs + 1, updated_at = NOW() WHERE id = $1")
            .bind(id)
            .execute(pool)
            .await?;
        Ok(())
    }

    /// Decrement current jobs
    pub async fn decrement_jobs(&self, pool: &DbPool, id: i64) -> DbResult<()> {
        sqlx::query("UPDATE agents SET current_jobs = GREATEST(current_jobs - 1, 0), updated_at = NOW() WHERE id = $1")
            .bind(id)
            .execute(pool)
            .await?;
        Ok(())
    }
}

#[async_trait]
impl Repository<DbAgent, DbCreateAgent, DbUpdateAgent> for AgentRepository {
    async fn create(&self, pool: &DbPool, create: DbCreateAgent) -> DbResult<DbAgent> {
        let uuid = uuid::Uuid::new_v4();
        let now = chrono::Utc::now();

        let id: i64 = sqlx::query_scalar(AgentQueryBuilder::insert())
            .bind(uuid)
            .bind(&create.name)
            .bind(&create.token_hash)
            .bind(crate::models::agent::DbAgentStatus::Offline)
            .bind(create.executor_type)
            .bind(create.executor_config)
            .bind(sqlx::types::Json(&create.labels))
            .bind(create.max_concurrent_jobs)
            .bind(0_i32) // current_jobs
            .bind(now)
            .bind(now)
            .fetch_one(pool)
            .await?;

        self.find_by_id(pool, id).await?.ok_or_else(|| crate::DbError::NotFound {
            entity_type: "Agent".to_string(),
            id: id.to_string(),
        })
    }

    async fn find_by_id(&self, pool: &DbPool, id: i64) -> DbResult<Option<DbAgent>> {
        let row = sqlx::query(AgentQueryBuilder::select_by_id())
            .bind(id)
            .fetch_optional(pool)
            .await?;

        Ok(row.map(|r| DbAgent::from_row(&r)).transpose()?)
    }

    async fn update(&self, pool: &DbPool, id: i64, update: DbUpdateAgent) -> DbResult<DbAgent> {
        let row = sqlx::query(AgentQueryBuilder::update())
            .bind(id)
            .bind(&update.name)
            .bind(update.status)
            .bind(update.executor_config)
            .bind(update.labels.as_ref().map(sqlx::types::Json))
            .bind(update.max_concurrent_jobs)
            .bind(update.current_jobs)
            .bind(update.last_seen_at)
            .bind(update.last_used_at)
            .bind(&update.version)
            .bind(&update.os)
            .bind(&update.arch)
            .fetch_one(pool)
            .await?;

        Ok(DbAgent::from_row(&row)?)
    }

    async fn delete(&self, pool: &DbPool, id: i64) -> DbResult<()> {
        sqlx::query(AgentQueryBuilder::delete())
            .bind(id)
            .execute(pool)
            .await?;
        Ok(())
    }

    async fn list(&self, pool: &DbPool, limit: i64, offset: i64) -> DbResult<Vec<DbAgent>> {
        let rows = sqlx::query(
            "SELECT * FROM agents WHERE deleted_at IS NULL ORDER BY created_at DESC LIMIT $1 OFFSET $2"
        )
        .bind(limit)
        .bind(offset)
        .fetch_all(pool)
        .await?;

        let agents = rows.into_iter().map(|r| DbAgent::from_row(&r)).collect::<Result<Vec<_>, _>>()?;
        Ok(agents)
    }

    async fn count(&self, pool: &DbPool) -> DbResult<i64> {
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM agents WHERE deleted_at IS NULL")
            .fetch_one(pool)
            .await?;
        Ok(count)
    }
}