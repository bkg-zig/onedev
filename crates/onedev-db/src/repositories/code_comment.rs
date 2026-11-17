//! Code Comment Repository

use crate::{
    models::code_comment::{DbCodeComment, DbCreateCodeComment, DbUpdateCodeComment, CodeCommentQueryBuilder},
    repositories::Repository,
    DbPool, DbResult,
};
use async_trait::async_trait;

pub struct CodeCommentRepository;

impl CodeCommentRepository {
    pub fn new() -> Self {
        Self
    }

    /// Find by pull request ID
    pub async fn find_by_pull_request(&self, pool: &DbPool, pr_id: i64) -> DbResult<Vec<DbCodeComment>> {
        let rows = sqlx::query(CodeCommentQueryBuilder::select_by_pull_request())
            .bind(pr_id)
            .fetch_all(pool)
            .await?;

        let comments = rows.into_iter().map(|r| DbCodeComment::from_row(&r)).collect::<Result<Vec<_>, _>>()?;
        Ok(comments)
    }

    /// Find by commit
    pub async fn find_by_commit(&self, pool: &DbPool, project_id: i64, commit_sha: &str) -> DbResult<Vec<DbCodeComment>> {
        let rows = sqlx::query(CodeCommentQueryBuilder::select_by_commit())
            .bind(project_id)
            .bind(commit_sha)
            .fetch_all(pool)
            .await?;

        let comments = rows.into_iter().map(|r| DbCodeComment::from_row(&r)).collect::<Result<Vec<_>, _>>()?;
        Ok(comments)
    }

    /// Find replies (children)
    pub async fn find_replies(&self, pool: &DbPool, parent_id: i64) -> DbResult<Vec<DbCodeComment>> {
        let rows = sqlx::query(
            "SELECT * FROM code_comments WHERE parent_id = $1 ORDER BY created_at ASC"
        )
        .bind(parent_id)
        .fetch_all(pool)
        .await?;

        let comments = rows.into_iter().map(|r| DbCodeComment::from_row(&r)).collect::<Result<Vec<_>, _>>()?;
        Ok(comments)
    }
}

#[async_trait]
impl Repository<DbCodeComment, DbCreateCodeComment, DbUpdateCodeComment> for CodeCommentRepository {
    async fn create(&self, pool: &DbPool, create: DbCreateCodeComment) -> DbResult<DbCodeComment> {
        let uuid = uuid::Uuid::new_v4();
        let now = chrono::Utc::now();

        let id: i64 = sqlx::query_scalar(CodeCommentQueryBuilder::insert())
            .bind(uuid)
            .bind(create.project_id)
            .bind(create.pull_request_id)
            .bind(&create.commit_sha)
            .bind(&create.file_path)
            .bind(&create.old_path)
            .bind(create.line_number)
            .bind(create.old_line_number)
            .bind(create.author_id)
            .bind(&create.content)
            .bind(create.parent_id)
            .bind(false) // resolved
            .bind(Option::<i64>::None) // resolved_by_id
            .bind(Option::<chrono::DateTime<chrono::Utc>>::None) // resolved_at
            .bind(false) // outdated
            .bind(now)
            .bind(now)
            .fetch_one(pool)
            .await?;

        self.find_by_id(pool, id).await?.ok_or_else(|| crate::DbError::NotFound {
            entity_type: "CodeComment".to_string(),
            id: id.to_string(),
        })
    }

    async fn find_by_id(&self, pool: &DbPool, id: i64) -> DbResult<Option<DbCodeComment>> {
        let row = sqlx::query(CodeCommentQueryBuilder::select_by_id())
            .bind(id)
            .fetch_optional(pool)
            .await?;

        Ok(row.map(|r| DbCodeComment::from_row(&r)).transpose()?)
    }

    async fn update(&self, pool: &DbPool, id: i64, update: DbUpdateCodeComment) -> DbResult<DbCodeComment> {
        let row = sqlx::query(CodeCommentQueryBuilder::update())
            .bind(id)
            .bind(&update.content)
            .bind(update.resolved)
            .bind(update.resolved_by_id)
            .bind(update.outdated)
            .fetch_one(pool)
            .await?;

        DbCodeComment::from_row(&row)
    }

    async fn delete(&self, pool: &DbPool, id: i64) -> DbResult<()> {
        sqlx::query(CodeCommentQueryBuilder::delete())
            .bind(id)
            .execute(pool)
            .await?;
        Ok(())
    }

    async fn list(&self, pool: &DbPool, limit: i64, offset: i64) -> DbResult<Vec<DbCodeComment>> {
        let rows = sqlx::query(
            "SELECT * FROM code_comments ORDER BY created_at DESC LIMIT $1 OFFSET $2"
        )
        .bind(limit)
        .bind(offset)
        .fetch_all(pool)
        .await?;

        let comments = rows.into_iter().map(|r| DbCodeComment::from_row(&r)).collect::<Result<Vec<_>, _>>()?;
        Ok(comments)
    }

    async fn count(&self, pool: &DbPool) -> DbResult<i64> {
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM code_comments")
            .fetch_one(pool)
            .await?;
        Ok(count)
    }
}