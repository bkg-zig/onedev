//! Build Repository

use crate::{
    DbPool, DbResult,
    models::build::{
        BuildQueryBuilder, DbBuild, DbBuildJob, DbBuildStep, DbCreateBuild, DbUpdateBuild,
    },
    repositories::Repository,
};
use async_trait::async_trait;
use sqlx::FromRow;

pub struct BuildRepository;

impl BuildRepository {
    pub fn new() -> Self {
        Self
    }

    /// Find by project ID
    pub async fn find_by_project_id(
        &self,
        pool: &DbPool,
        project_id: i64,
        limit: i64,
        offset: i64,
    ) -> DbResult<Vec<DbBuild>> {
        let rows = sqlx::query(BuildQueryBuilder::select_by_project_id())
            .bind(project_id)
            .bind(limit)
            .bind(offset)
            .fetch_all(pool)
            .await?;

        let builds = rows
            .into_iter()
            .map(|r| DbBuild::from_row(&r))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(builds)
    }

    /// Find by project and number
    pub async fn find_by_project_and_number(
        &self,
        pool: &DbPool,
        project_id: i64,
        number: i32,
    ) -> DbResult<Option<DbBuild>> {
        let row = sqlx::query(BuildQueryBuilder::select_by_project_and_number())
            .bind(project_id)
            .bind(number)
            .fetch_optional(pool)
            .await?;

        Ok(row.map(|r| DbBuild::from_row(&r)).transpose()?)
    }

    /// Find running builds
    pub async fn find_running(&self, pool: &DbPool) -> DbResult<Vec<DbBuild>> {
        let rows = sqlx::query(BuildQueryBuilder::select_running())
            .fetch_all(pool)
            .await?;

        let builds = rows
            .into_iter()
            .map(|r| DbBuild::from_row(&r))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(builds)
    }

    /// Get next build number for project
    pub async fn next_number(&self, pool: &DbPool, project_id: i64) -> DbResult<i32> {
        let max: Option<i32> = sqlx::query_scalar(
            "SELECT COALESCE(MAX(number), 0) + 1 FROM builds WHERE project_id = $1",
        )
        .bind(project_id)
        .fetch_one(pool)
        .await?;
        Ok(max.unwrap_or(1))
    }
}

#[async_trait]
impl Repository<DbBuild, DbCreateBuild, DbUpdateBuild> for BuildRepository {
    async fn create(&self, pool: &DbPool, create: DbCreateBuild) -> DbResult<DbBuild> {
        let uuid = uuid::Uuid::new_v4();
        let now = chrono::Utc::now();
        let number = self.next_number(pool, create.project_id).await?;

        let id: i64 = sqlx::query_scalar(BuildQueryBuilder::insert())
            .bind(uuid)
            .bind(create.project_id)
            .bind(number)
            .bind(&create.ref_name)
            .bind(&create.commit_sha)
            .bind(&create.commit_message)
            .bind(create.author_id)
            .bind(crate::models::build::DbBuildStatus::Pending)
            .bind(create.trigger)
            .bind(create.trigger_data)
            .bind(create.spec)
            .bind(create.parameters)
            .bind(now)
            .bind(now)
            .fetch_one(pool)
            .await?;

        self.find_by_id(pool, id)
            .await?
            .ok_or_else(|| crate::DbError::NotFound {
                entity_type: "Build".to_string(),
                id: id.to_string(),
            })
    }

    async fn find_by_id(&self, pool: &DbPool, id: i64) -> DbResult<Option<DbBuild>> {
        let row = sqlx::query(BuildQueryBuilder::select_by_id())
            .bind(id)
            .fetch_optional(pool)
            .await?;

        Ok(row.map(|r| DbBuild::from_row(&r)).transpose()?)
    }

    async fn update(&self, pool: &DbPool, id: i64, update: DbUpdateBuild) -> DbResult<DbBuild> {
        let row = sqlx::query(BuildQueryBuilder::update())
            .bind(id)
            .bind(update.status)
            .bind(update.agent_id)
            .bind(update.started_at)
            .bind(update.finished_at)
            .bind(update.duration)
            .bind(&update.error_message)
            .bind(update.artifacts)
            .fetch_one(pool)
            .await?;

        Ok(DbBuild::from_row(&row)?)
    }

    async fn delete(&self, pool: &DbPool, id: i64) -> DbResult<()> {
        sqlx::query("DELETE FROM builds WHERE id = $1")
            .bind(id)
            .execute(pool)
            .await?;
        Ok(())
    }

    async fn list(&self, pool: &DbPool, limit: i64, offset: i64) -> DbResult<Vec<DbBuild>> {
        let rows = sqlx::query("SELECT * FROM builds ORDER BY created_at DESC LIMIT $1 OFFSET $2")
            .bind(limit)
            .bind(offset)
            .fetch_all(pool)
            .await?;

        let builds = rows
            .into_iter()
            .map(|r| DbBuild::from_row(&r))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(builds)
    }

    async fn count(&self, pool: &DbPool) -> DbResult<i64> {
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM builds")
            .fetch_one(pool)
            .await?;
        Ok(count)
    }
}

pub struct BuildJobRepository;

impl BuildJobRepository {
    pub fn new() -> Self {
        Self
    }

    /// Find by build ID
    pub async fn find_by_build_id(
        &self,
        pool: &DbPool,
        build_id: i64,
    ) -> DbResult<Vec<DbBuildJob>> {
        let rows =
            sqlx::query("SELECT * FROM build_jobs WHERE build_id = $1 ORDER BY order_num ASC")
                .bind(build_id)
                .fetch_all(pool)
                .await?;

        let jobs = rows
            .into_iter()
            .map(|r| DbBuildJob::from_row(&r))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(jobs)
    }
}

#[async_trait]
impl Repository<DbBuildJob, DbBuildJob, DbBuildJob> for BuildJobRepository {
    async fn create(&self, pool: &DbPool, create: DbBuildJob) -> DbResult<DbBuildJob> {
        let uuid = uuid::Uuid::new_v4();
        let now = chrono::Utc::now();

        let id: i64 = sqlx::query_scalar(
            r#"
            INSERT INTO build_jobs (uuid, build_id, name, status, agent_id, started_at, finished_at,
                                    duration, error_message, spec, dependencies, order_num, created_at, updated_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14)
            RETURNING id
            "#,
        )
        .bind(uuid)
        .bind(create.build_id)
        .bind(&create.name)
        .bind(create.status)
        .bind(create.agent_id)
        .bind(create.started_at)
        .bind(create.finished_at)
        .bind(create.duration)
        .bind(&create.error_message)
        .bind(create.spec)
        .bind(&create.dependencies)
        .bind(create.order)
        .bind(now)
        .bind(now)
        .fetch_one(pool)
        .await?;

        self.find_by_id(pool, id)
            .await?
            .ok_or_else(|| crate::DbError::NotFound {
                entity_type: "BuildJob".to_string(),
                id: id.to_string(),
            })
    }

    async fn find_by_id(&self, pool: &DbPool, id: i64) -> DbResult<Option<DbBuildJob>> {
        let row = sqlx::query("SELECT * FROM build_jobs WHERE id = $1")
            .bind(id)
            .fetch_optional(pool)
            .await?;

        Ok(row.map(|r| DbBuildJob::from_row(&r)).transpose()?)
    }

    async fn update(&self, pool: &DbPool, id: i64, update: DbBuildJob) -> DbResult<DbBuildJob> {
        let row = sqlx::query(
            r#"
            UPDATE build_jobs SET
                name = $2, status = $3, agent_id = $4, started_at = $5, finished_at = $6,
                duration = $7, error_message = $8, spec = $9, dependencies = $10, order_num = $11,
                updated_at = NOW()
            WHERE id = $1
            RETURNING *
            "#,
        )
        .bind(id)
        .bind(&update.name)
        .bind(update.status)
        .bind(update.agent_id)
        .bind(update.started_at)
        .bind(update.finished_at)
        .bind(update.duration)
        .bind(&update.error_message)
        .bind(update.spec)
        .bind(&update.dependencies)
        .bind(update.order)
        .fetch_one(pool)
        .await?;

        Ok(DbBuildJob::from_row(&row)?)
    }

    async fn delete(&self, pool: &DbPool, id: i64) -> DbResult<()> {
        sqlx::query("DELETE FROM build_jobs WHERE id = $1")
            .bind(id)
            .execute(pool)
            .await?;
        Ok(())
    }

    async fn list(&self, pool: &DbPool, limit: i64, offset: i64) -> DbResult<Vec<DbBuildJob>> {
        let rows =
            sqlx::query("SELECT * FROM build_jobs ORDER BY created_at DESC LIMIT $1 OFFSET $2")
                .bind(limit)
                .bind(offset)
                .fetch_all(pool)
                .await?;

        let jobs = rows
            .into_iter()
            .map(|r| DbBuildJob::from_row(&r))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(jobs)
    }

    async fn count(&self, pool: &DbPool) -> DbResult<i64> {
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM build_jobs")
            .fetch_one(pool)
            .await?;
        Ok(count)
    }
}

pub struct BuildStepRepository;

impl BuildStepRepository {
    pub fn new() -> Self {
        Self
    }

    /// Find by job ID
    pub async fn find_by_job_id(&self, pool: &DbPool, job_id: i64) -> DbResult<Vec<DbBuildStep>> {
        let rows =
            sqlx::query("SELECT * FROM build_steps WHERE job_id = $1 ORDER BY order_num ASC")
                .bind(job_id)
                .fetch_all(pool)
                .await?;

        let steps = rows
            .into_iter()
            .map(|r| DbBuildStep::from_row(&r))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(steps)
    }
}

#[async_trait]
impl Repository<DbBuildStep, DbBuildStep, DbBuildStep> for BuildStepRepository {
    async fn create(&self, pool: &DbPool, create: DbBuildStep) -> DbResult<DbBuildStep> {
        let uuid = uuid::Uuid::new_v4();
        let now = chrono::Utc::now();

        let id: i64 = sqlx::query_scalar(
            r#"
            INSERT INTO build_steps (uuid, job_id, name, status, command, image, started_at, finished_at,
                                     duration, error_message, logs, order_num, created_at, updated_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14)
            RETURNING id
            "#,
        )
        .bind(uuid)
        .bind(create.job_id)
        .bind(&create.name)
        .bind(create.status)
        .bind(&create.command)
        .bind(&create.image)
        .bind(create.started_at)
        .bind(create.finished_at)
        .bind(create.duration)
        .bind(&create.error_message)
        .bind(&create.logs)
        .bind(create.order)
        .bind(now)
        .bind(now)
        .fetch_one(pool)
        .await?;

        self.find_by_id(pool, id)
            .await?
            .ok_or_else(|| crate::DbError::NotFound {
                entity_type: "BuildStep".to_string(),
                id: id.to_string(),
            })
    }

    async fn find_by_id(&self, pool: &DbPool, id: i64) -> DbResult<Option<DbBuildStep>> {
        let row = sqlx::query("SELECT * FROM build_steps WHERE id = $1")
            .bind(id)
            .fetch_optional(pool)
            .await?;

        Ok(row.map(|r| DbBuildStep::from_row(&r)).transpose()?)
    }

    async fn update(&self, pool: &DbPool, id: i64, update: DbBuildStep) -> DbResult<DbBuildStep> {
        let row = sqlx::query(
            r#"
            UPDATE build_steps SET
                name = $2, status = $3, command = $4, image = $5, started_at = $6, finished_at = $7,
                duration = $8, error_message = $9, logs = $10, order_num = $11, updated_at = NOW()
            WHERE id = $1
            RETURNING *
            "#,
        )
        .bind(id)
        .bind(&update.name)
        .bind(update.status)
        .bind(&update.command)
        .bind(&update.image)
        .bind(update.started_at)
        .bind(update.finished_at)
        .bind(update.duration)
        .bind(&update.error_message)
        .bind(&update.logs)
        .bind(update.order)
        .fetch_one(pool)
        .await?;

        Ok(DbBuildStep::from_row(&row)?)
    }

    async fn delete(&self, pool: &DbPool, id: i64) -> DbResult<()> {
        sqlx::query("DELETE FROM build_steps WHERE id = $1")
            .bind(id)
            .execute(pool)
            .await?;
        Ok(())
    }

    async fn list(&self, pool: &DbPool, limit: i64, offset: i64) -> DbResult<Vec<DbBuildStep>> {
        let rows =
            sqlx::query("SELECT * FROM build_steps ORDER BY created_at DESC LIMIT $1 OFFSET $2")
                .bind(limit)
                .bind(offset)
                .fetch_all(pool)
                .await?;

        let steps = rows
            .into_iter()
            .map(|r| DbBuildStep::from_row(&r))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(steps)
    }

    async fn count(&self, pool: &DbPool) -> DbResult<i64> {
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM build_steps")
            .fetch_one(pool)
            .await?;
        Ok(count)
    }
}
