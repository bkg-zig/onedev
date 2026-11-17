//! Workspace Repository

use crate::{
    models::workspace::{DbWorkspace, DbCreateWorkspace, DbUpdateWorkspace, WorkspaceQueryBuilder},
    repositories::Repository,
    DbPool, DbResult,
};
use async_trait::async_trait;
use sqlx::FromRow;

pub struct WorkspaceRepository;

impl WorkspaceRepository {
    pub fn new() -> Self {
        Self
    }

    /// Find by project ID
    pub async fn find_by_project_id(&self, pool: &DbPool, project_id: i64, limit: i64, offset: i64) -> DbResult<Vec<DbWorkspace>> {
        let rows = sqlx::query(WorkspaceQueryBuilder::select_by_project_id())
            .bind(project_id)
            .bind(limit)
            .bind(offset)
            .fetch_all(pool)
            .await?;

        let workspaces = rows.into_iter().map(|r| DbWorkspace::from_row(&r)).collect::<Result<Vec<_>, _>>()?;
        Ok(workspaces)
    }

    /// Find by creator ID
    pub async fn find_by_creator_id(&self, pool: &DbPool, creator_id: i64, limit: i64, offset: i64) -> DbResult<Vec<DbWorkspace>> {
        let rows = sqlx::query(WorkspaceQueryBuilder::select_by_creator_id())
            .bind(creator_id)
            .bind(limit)
            .bind(offset)
            .fetch_all(pool)
            .await?;

        let workspaces = rows.into_iter().map(|r| DbWorkspace::from_row(&r)).collect::<Result<Vec<_>, _>>()?;
        Ok(workspaces)
    }

    /// Find by agent ID
    pub async fn find_by_agent_id(&self, pool: &DbPool, agent_id: i64) -> DbResult<Vec<DbWorkspace>> {
        let rows = sqlx::query(WorkspaceQueryBuilder::select_by_agent_id())
            .bind(agent_id)
            .fetch_all(pool)
            .await?;

        let workspaces = rows.into_iter().map(|r| DbWorkspace::from_row(&r)).collect::<Result<Vec<_>, _>>()?;
        Ok(workspaces)
    }
}

#[async_trait]
impl Repository<DbWorkspace, DbCreateWorkspace, DbUpdateWorkspace> for WorkspaceRepository {
    async fn create(&self, pool: &DbPool, create: DbCreateWorkspace) -> DbResult<DbWorkspace> {
        let uuid = uuid::Uuid::new_v4();
        let now = chrono::Utc::now();

        let id: i64 = sqlx::query_scalar(WorkspaceQueryBuilder::insert())
            .bind(uuid)
            .bind(create.project_id)
            .bind(&create.name)
            .bind(create.spec)
            .bind(crate::models::workspace::DbWorkspaceStatus::Pending)
            .bind(Option::<i64>::None)
            .bind(create.creator_id)
            .bind(&create.commit_sha)
            .bind(&create.branch)
            .bind(now)
            .bind(now)
            .fetch_one(pool)
            .await?;

        self.find_by_id(pool, id).await?.ok_or_else(|| crate::DbError::NotFound {
            entity_type: "Workspace".to_string(),
            id: id.to_string(),
        })
    }

    async fn find_by_id(&self, pool: &DbPool, id: i64) -> DbResult<Option<DbWorkspace>> {
        let row = sqlx::query(WorkspaceQueryBuilder::select_by_id())
            .bind(id)
            .fetch_optional(pool)
            .await?;

        Ok(row.map(|r| DbWorkspace::from_row(&r)).transpose()?)
    }

    async fn update(&self, pool: &DbPool, id: i64, update: DbUpdateWorkspace) -> DbResult<DbWorkspace> {
        let row = sqlx::query(WorkspaceQueryBuilder::update())
            .bind(id)
            .bind(update.status)
            .bind(update.agent_id)
            .bind(&update.commit_sha)
            .bind(&update.branch)
            .bind(update.started_at)
            .bind(update.finished_at)
            .bind(&update.error_message)
            .fetch_one(pool)
            .await?;

        Ok(DbWorkspace::from_row(&row)?)
    }

    async fn delete(&self, pool: &DbPool, id: i64) -> DbResult<()> {
        sqlx::query(WorkspaceQueryBuilder::delete())
            .bind(id)
            .execute(pool)
            .await?;
        Ok(())
    }

    async fn list(&self, pool: &DbPool, limit: i64, offset: i64) -> DbResult<Vec<DbWorkspace>> {
        let rows = sqlx::query(
            "SELECT * FROM workspaces ORDER BY created_at DESC LIMIT $1 OFFSET $2"
        )
        .bind(limit)
        .bind(offset)
        .fetch_all(pool)
        .await?;

        let workspaces = rows.into_iter().map(|r| DbWorkspace::from_row(&r)).collect::<Result<Vec<_>, _>>()?;
        Ok(workspaces)
    }

    async fn count(&self, pool: &DbPool) -> DbResult<i64> {
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM workspaces")
            .fetch_one(pool)
            .await?;
        Ok(count)
    }
}