//! Pull Request Repository

use crate::{
    DbPool, DbResult,
    models::pull_request::{
        DbCreatePullRequest, DbPullRequest, DbUpdatePullRequest, PullRequestQueryBuilder,
    },
    repositories::Repository,
};
use async_trait::async_trait;
use sqlx::FromRow;

pub struct PullRequestRepository;

impl PullRequestRepository {
    pub fn new() -> Self {
        Self
    }

    /// Find pull request by project and number
    pub async fn find_by_project_and_number(
        &self,
        pool: &DbPool,
        project_id: i64,
        number: i32,
    ) -> DbResult<Option<DbPullRequest>> {
        let row = sqlx::query(PullRequestQueryBuilder::select_by_project_and_number())
            .bind(project_id)
            .bind(number)
            .fetch_optional(pool)
            .await?;

        Ok(row.map(|r| DbPullRequest::from_row(&r)).transpose()?)
    }

    /// Get next pull request number for project
    pub async fn next_number(&self, pool: &DbPool, project_id: i64) -> DbResult<i32> {
        let max: Option<i32> = sqlx::query_scalar(
            "SELECT COALESCE(MAX(number), 0) + 1 FROM pull_requests WHERE project_id = $1",
        )
        .bind(project_id)
        .fetch_one(pool)
        .await?;
        Ok(max.unwrap_or(1))
    }

    /// Add reviewer
    pub async fn add_reviewer(&self, pool: &DbPool, pr_id: i64, user_id: i64) -> DbResult<()> {
        sqlx::query(
            "UPDATE pull_requests SET reviewer_ids = array_append(reviewer_ids, $1) WHERE id = $2 AND NOT $1 = ANY(reviewer_ids)"
        )
        .bind(user_id)
        .bind(pr_id)
        .execute(pool)
        .await?;
        Ok(())
    }

    /// Remove reviewer
    pub async fn remove_reviewer(&self, pool: &DbPool, pr_id: i64, user_id: i64) -> DbResult<()> {
        sqlx::query(
            "UPDATE pull_requests SET reviewer_ids = array_remove(reviewer_ids, $1) WHERE id = $2",
        )
        .bind(user_id)
        .bind(pr_id)
        .execute(pool)
        .await?;
        Ok(())
    }

    /// Add watcher
    pub async fn add_watcher(&self, pool: &DbPool, pr_id: i64, user_id: i64) -> DbResult<()> {
        sqlx::query(
            "UPDATE pull_requests SET watcher_ids = array_append(watcher_ids, $1) WHERE id = $2 AND NOT $1 = ANY(watcher_ids)"
        )
        .bind(user_id)
        .bind(pr_id)
        .execute(pool)
        .await?;
        Ok(())
    }

    /// Remove watcher
    pub async fn remove_watcher(&self, pool: &DbPool, pr_id: i64, user_id: i64) -> DbResult<()> {
        sqlx::query(
            "UPDATE pull_requests SET watcher_ids = array_remove(watcher_ids, $1) WHERE id = $2",
        )
        .bind(user_id)
        .bind(pr_id)
        .execute(pool)
        .await?;
        Ok(())
    }

    /// Add label
    pub async fn add_label(&self, pool: &DbPool, pr_id: i64, label_id: i64) -> DbResult<()> {
        sqlx::query(
            "UPDATE pull_requests SET label_ids = array_append(label_ids, $1) WHERE id = $2 AND NOT $1 = ANY(label_ids)"
        )
        .bind(label_id)
        .bind(pr_id)
        .execute(pool)
        .await?;
        Ok(())
    }

    /// Remove label
    pub async fn remove_label(&self, pool: &DbPool, pr_id: i64, label_id: i64) -> DbResult<()> {
        sqlx::query(
            "UPDATE pull_requests SET label_ids = array_remove(label_ids, $1) WHERE id = $2",
        )
        .bind(label_id)
        .bind(pr_id)
        .execute(pool)
        .await?;
        Ok(())
    }
}

#[async_trait]
impl Repository<DbPullRequest, DbCreatePullRequest, DbUpdatePullRequest> for PullRequestRepository {
    async fn create(&self, pool: &DbPool, create: DbCreatePullRequest) -> DbResult<DbPullRequest> {
        let uuid = uuid::Uuid::new_v4();
        let now = chrono::Utc::now();
        let number = self.next_number(pool, create.project_id).await?;

        let id: i64 = sqlx::query_scalar(PullRequestQueryBuilder::insert())
            .bind(uuid)
            .bind(create.project_id)
            .bind(number)
            .bind(&create.title)
            .bind(&create.description)
            .bind(crate::models::pull_request::DbPullRequestState::Open)
            .bind(create.author_id)
            .bind(create.source_project_id)
            .bind(&create.source_branch)
            .bind(create.target_project_id)
            .bind(&create.target_branch)
            .bind(create.merge_strategy)
            .bind(&create.head_commit_sha)
            .bind(&create.base_commit_sha)
            .bind(&create.merge_base_sha)
            .bind(true) // mergeable
            .bind(Option::<serde_json::Value>::None) // merge_check_status
            .bind(now)
            .bind(now)
            .bind(&create.label_ids)
            .bind(&create.reviewer_ids)
            .bind(&create.watcher_ids)
            .bind(create.draft)
            .fetch_one(pool)
            .await?;

        self.find_by_id(pool, id)
            .await?
            .ok_or_else(|| crate::DbError::NotFound {
                entity_type: "PullRequest".to_string(),
                id: id.to_string(),
            })
    }

    async fn find_by_id(&self, pool: &DbPool, id: i64) -> DbResult<Option<DbPullRequest>> {
        let row = sqlx::query(PullRequestQueryBuilder::select_by_id())
            .bind(id)
            .fetch_optional(pool)
            .await?;

        Ok(row.map(|r| DbPullRequest::from_row(&r)).transpose()?)
    }

    async fn update(
        &self,
        pool: &DbPool,
        id: i64,
        update: DbUpdatePullRequest,
    ) -> DbResult<DbPullRequest> {
        let row = sqlx::query(PullRequestQueryBuilder::update())
            .bind(id)
            .bind(&update.title)
            .bind(&update.description)
            .bind(update.state)
            .bind(update.merge_strategy)
            .bind(&update.merge_commit_sha)
            .bind(&update.head_commit_sha)
            .bind(update.mergeable)
            .bind(&update.merge_check_status)
            .bind(update.draft)
            .bind(update.label_ids.as_deref())
            .bind(update.reviewer_ids.as_deref())
            .bind(update.watcher_ids.as_deref())
            .fetch_one(pool)
            .await?;

        Ok(DbPullRequest::from_row(&row)?)
    }

    async fn delete(&self, pool: &DbPool, id: i64) -> DbResult<()> {
        sqlx::query(PullRequestQueryBuilder::delete())
            .bind(id)
            .execute(pool)
            .await?;
        Ok(())
    }

    async fn list(&self, pool: &DbPool, limit: i64, offset: i64) -> DbResult<Vec<DbPullRequest>> {
        let rows =
            sqlx::query("SELECT * FROM pull_requests ORDER BY created_at DESC LIMIT $1 OFFSET $2")
                .bind(limit)
                .bind(offset)
                .fetch_all(pool)
                .await?;

        let prs = rows
            .into_iter()
            .map(|r| DbPullRequest::from_row(&r))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(prs)
    }

    async fn count(&self, pool: &DbPool) -> DbResult<i64> {
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM pull_requests")
            .fetch_one(pool)
            .await?;
        Ok(count)
    }
}
