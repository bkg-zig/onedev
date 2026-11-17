//! Pipeline Repository

use crate::{
    models::pipeline::{DbPipeline, DbCreatePipeline, DbUpdatePipeline, DbPipelineSchedule, PipelineQueryBuilder, PipelineScheduleQueryBuilder},
    repositories::Repository,
    DbPool, DbResult,
};
use async_trait::async_trait;

pub struct PipelineRepository;

impl PipelineRepository {
    pub fn new() -> Self {
        Self
    }

    /// Find by project ID
    pub async fn find_by_project_id(&self, pool: &DbPool, project_id: i64) -> DbResult<Vec<DbPipeline>> {
        let rows = sqlx::query(PipelineQueryBuilder::select_by_project_id())
            .bind(project_id)
            .fetch_all(pool)
            .await?;

        let pipelines = rows.into_iter().map(|r| DbPipeline::from_row(&r)).collect::<Result<Vec<_>, _>>()?;
        Ok(pipelines)
    }

    /// Find default by project
    pub async fn find_default_by_project(&self, pool: &DbPool, project_id: i64) -> DbResult<Option<DbPipeline>> {
        let row = sqlx::query(PipelineQueryBuilder::select_default_by_project())
            .bind(project_id)
            .fetch_optional(pool)
            .await?;

        Ok(row.map(|r| DbPipeline::from_row(&r)).transpose()?)
    }
}

#[async_trait]
impl Repository<DbPipeline, DbCreatePipeline, DbUpdatePipeline> for PipelineRepository {
    async fn create(&self, pool: &DbPool, create: DbCreatePipeline) -> DbResult<DbPipeline> {
        let uuid = uuid::Uuid::new_v4();
        let now = chrono::Utc::now();

        // If this is default, unset other defaults
        if create.is_default {
            sqlx::query(
                "UPDATE pipelines SET is_default = FALSE, updated_at = NOW() WHERE project_id = $1 AND is_default = TRUE"
            )
            .bind(create.project_id)
            .execute(pool)
            .await?;
        }

        let id: i64 = sqlx::query_scalar(PipelineQueryBuilder::insert())
            .bind(uuid)
            .bind(create.project_id)
            .bind(&create.name)
            .bind(create.spec)
            .bind(create.is_default)
            .bind(create.protected)
            .bind(now)
            .bind(now)
            .fetch_one(pool)
            .await?;

        self.find_by_id(pool, id).await?.ok_or_else(|| crate::DbError::NotFound {
            entity_type: "Pipeline".to_string(),
            id: id.to_string(),
        })
    }

    async fn find_by_id(&self, pool: &DbPool, id: i64) -> DbResult<Option<DbPipeline>> {
        let row = sqlx::query(PipelineQueryBuilder::select_by_id())
            .bind(id)
            .fetch_optional(pool)
            .await?;

        Ok(row.map(|r| DbPipeline::from_row(&r)).transpose()?)
    }

    async fn update(&self, pool: &DbPool, id: i64, update: DbUpdatePipeline) -> DbResult<DbPipeline> {
        // If setting as default, unset other defaults
        if update.is_default == Some(true) {
            let current = self.find_by_id(pool, id).await?;
            if let Some(pipeline) = current {
                sqlx::query(
                    "UPDATE pipelines SET is_default = FALSE, updated_at = NOW() WHERE project_id = $1 AND is_default = TRUE AND id != $2"
                )
                .bind(pipeline.project_id)
                .bind(id)
                .execute(pool)
                .await?;
            }
        }

        let row = sqlx::query(PipelineQueryBuilder::update())
            .bind(id)
            .bind(&update.name)
            .bind(update.spec.map(serde_json::to_value).transpose()?)
            .bind(update.is_default)
            .bind(update.protected)
            .fetch_one(pool)
            .await?;

        DbPipeline::from_row(&row)
    }

    async fn delete(&self, pool: &DbPool, id: i64) -> DbResult<()> {
        sqlx::query(PipelineQueryBuilder::delete())
            .bind(id)
            .execute(pool)
            .await?;
        Ok(())
    }

    async fn list(&self, pool: &DbPool, limit: i64, offset: i64) -> DbResult<Vec<DbPipeline>> {
        let rows = sqlx::query(
            "SELECT * FROM pipelines ORDER BY created_at DESC LIMIT $1 OFFSET $2"
        )
        .bind(limit)
        .bind(offset)
        .fetch_all(pool)
        .await?;

        let pipelines = rows.into_iter().map(|r| DbPipeline::from_row(&r)).collect::<Result<Vec<_>, _>>()?;
        Ok(pipelines)
    }

    async fn count(&self, pool: &DbPool) -> DbResult<i64> {
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM pipelines")
            .fetch_one(pool)
            .await?;
        Ok(count)
    }
}

pub struct PipelineScheduleRepository;

impl PipelineScheduleRepository {
    pub fn new() -> Self {
        Self
    }

    /// Find by pipeline ID
    pub async fn find_by_pipeline_id(&self, pool: &DbPool, pipeline_id: i64) -> DbResult<Vec<DbPipelineSchedule>> {
        let rows = sqlx::query(PipelineScheduleQueryBuilder::select_by_pipeline_id())
            .bind(pipeline_id)
            .fetch_all(pool)
            .await?;

        let schedules = rows.into_iter().map(|r| DbPipelineSchedule::from_row(&r)).collect::<Result<Vec<_>, _>>()?;
        Ok(schedules)
    }

    /// Find due schedules
    pub async fn find_due(&self, pool: &DbPool) -> DbResult<Vec<DbPipelineSchedule>> {
        let rows = sqlx::query(PipelineScheduleQueryBuilder::select_due())
            .fetch_all(pool)
            .await?;

        let schedules = rows.into_iter().map(|r| DbPipelineSchedule::from_row(&r)).collect::<Result<Vec<_>, _>>()?;
        Ok(schedules)
    }

    /// Update next run time
    pub async fn update_next_run(&self, pool: &DbPool, id: i64, next_run_at: chrono::DateTime<chrono::Utc>) -> DbResult<()> {
        sqlx::query(
            "UPDATE pipeline_schedules SET next_run_at = $2, last_run_at = NOW(), updated_at = NOW() WHERE id = $1"
        )
        .bind(id)
        .bind(next_run_at)
        .execute(pool)
        .await?;
        Ok(())
    }
}

#[async_trait]
impl Repository<DbPipelineSchedule, DbPipelineSchedule, DbPipelineSchedule> for PipelineScheduleRepository {
    async fn create(&self, pool: &DbPool, create: DbPipelineSchedule) -> DbResult<DbPipelineSchedule> {
        let uuid = uuid::Uuid::new_v4();
        let now = chrono::Utc::now();

        let id: i64 = sqlx::query_scalar(PipelineScheduleQueryBuilder::insert())
            .bind(uuid)
            .bind(create.pipeline_id)
            .bind(&create.cron_expression)
            .bind(&create.timezone)
            .bind(&create.ref_name)
            .bind(create.parameters.map(serde_json::to_value).transpose()?)
            .bind(create.active)
            .bind(create.next_run_at)
            .bind(now)
            .bind(now)
            .fetch_one(pool)
            .await?;

        self.find_by_id(pool, id).await?.ok_or_else(|| crate::DbError::NotFound {
            entity_type: "PipelineSchedule".to_string(),
            id: id.to_string(),
        })
    }

    async fn find_by_id(&self, pool: &DbPool, id: i64) -> DbResult<Option<DbPipelineSchedule>> {
        let row = sqlx::query("SELECT * FROM pipeline_schedules WHERE id = $1")
            .bind(id)
            .fetch_optional(pool)
            .await?;

        Ok(row.map(|r| DbPipelineSchedule::from_row(&r)).transpose()?)
    }

    async fn update(&self, pool: &DbPool, id: i64, update: DbPipelineSchedule) -> DbResult<DbPipelineSchedule> {
        let row = sqlx::query(PipelineScheduleQueryBuilder::update())
            .bind(id)
            .bind(&update.cron_expression)
            .bind(&update.timezone)
            .bind(&update.ref_name)
            .bind(update.parameters.map(serde_json::to_value).transpose()?)
            .bind(update.active)
            .bind(update.next_run_at)
            .bind(update.last_run_at)
            .fetch_one(pool)
            .await?;

        DbPipelineSchedule::from_row(&row)
    }

    async fn delete(&self, pool: &DbPool, id: i64) -> DbResult<()> {
        sqlx::query(PipelineScheduleQueryBuilder::delete())
            .bind(id)
            .execute(pool)
            .await?;
        Ok(())
    }

    async fn list(&self, pool: &DbPool, limit: i64, offset: i64) -> DbResult<Vec<DbPipelineSchedule>> {
        let rows = sqlx::query(
            "SELECT * FROM pipeline_schedules ORDER BY created_at DESC LIMIT $1 OFFSET $2"
        )
        .bind(limit)
        .bind(offset)
        .fetch_all(pool)
        .await?;

        let schedules = rows.into_iter().map(|r| DbPipelineSchedule::from_row(&r)).collect::<Result<Vec<_>, _>>()?;
        Ok(schedules)
    }

    async fn count(&self, pool: &DbPool) -> DbResult<i64> {
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM pipeline_schedules")
            .fetch_one(pool)
            .await?;
        Ok(count)
    }
}