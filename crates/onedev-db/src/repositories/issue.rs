//! Issue Repository

use crate::{
    models::issue::{DbIssue, DbCreateIssue, DbUpdateIssue, IssueQueryBuilder},
    repositories::Repository,
    DbPool, DbResult,
};
use async_trait::async_trait;
use sqlx::FromRow;

pub struct IssueRepository;

impl IssueRepository {
    pub fn new() -> Self {
        Self
    }

    /// Find issue by project and number
    pub async fn find_by_project_and_number(&self, pool: &DbPool, project_id: i64, number: i32) -> DbResult<Option<DbIssue>> {
        let row = sqlx::query(IssueQueryBuilder::select_by_project_and_number())
            .bind(project_id)
            .bind(number)
            .fetch_optional(pool)
            .await?;

        Ok(row.map(|r| DbIssue::from_row(&r)).transpose()?)
    }

    /// Get next issue number for project
    pub async fn next_number(&self, pool: &DbPool, project_id: i64) -> DbResult<i32> {
        let max: Option<i32> = sqlx::query_scalar(
            "SELECT COALESCE(MAX(number), 0) + 1 FROM issues WHERE project_id = $1"
        )
        .bind(project_id)
        .fetch_one(pool)
        .await?;
        Ok(max.unwrap_or(1))
    }

    /// Add watcher
    pub async fn add_watcher(&self, pool: &DbPool, issue_id: i64, user_id: i64) -> DbResult<()> {
        sqlx::query(
            "UPDATE issues SET watcher_ids = array_append(watcher_ids, $1) WHERE id = $2 AND NOT $1 = ANY(watcher_ids)"
        )
        .bind(user_id)
        .bind(issue_id)
        .execute(pool)
        .await?;
        Ok(())
    }

    /// Remove watcher
    pub async fn remove_watcher(&self, pool: &DbPool, issue_id: i64, user_id: i64) -> DbResult<()> {
        sqlx::query(
            "UPDATE issues SET watcher_ids = array_remove(watcher_ids, $1) WHERE id = $2"
        )
        .bind(user_id)
        .bind(issue_id)
        .execute(pool)
        .await?;
        Ok(())
    }

    /// Add voter
    pub async fn add_voter(&self, pool: &DbPool, issue_id: i64, user_id: i64) -> DbResult<()> {
        sqlx::query(
            "UPDATE issues SET voter_ids = array_append(voter_ids, $1) WHERE id = $2 AND NOT $1 = ANY(voter_ids)"
        )
        .bind(user_id)
        .bind(issue_id)
        .execute(pool)
        .await?;
        Ok(())
    }

    /// Remove voter
    pub async fn remove_voter(&self, pool: &DbPool, issue_id: i64, user_id: i64) -> DbResult<()> {
        sqlx::query(
            "UPDATE issues SET voter_ids = array_remove(voter_ids, $1) WHERE id = $2"
        )
        .bind(user_id)
        .bind(issue_id)
        .execute(pool)
        .await?;
        Ok(())
    }

    /// Add label
    pub async fn add_label(&self, pool: &DbPool, issue_id: i64, label_id: i64) -> DbResult<()> {
        sqlx::query(
            "UPDATE issues SET label_ids = array_append(label_ids, $1) WHERE id = $2 AND NOT $1 = ANY(label_ids)"
        )
        .bind(label_id)
        .bind(issue_id)
        .execute(pool)
        .await?;
        Ok(())
    }

    /// Remove label
    pub async fn remove_label(&self, pool: &DbPool, issue_id: i64, label_id: i64) -> DbResult<()> {
        sqlx::query(
            "UPDATE issues SET label_ids = array_remove(label_ids, $1) WHERE id = $2"
        )
        .bind(label_id)
        .bind(issue_id)
        .execute(pool)
        .await?;
        Ok(())
    }
}

#[async_trait]
impl Repository<DbIssue, DbCreateIssue, DbUpdateIssue> for IssueRepository {
    async fn create(&self, pool: &DbPool, create: DbCreateIssue) -> DbResult<DbIssue> {
        let uuid = uuid::Uuid::new_v4();
        let now = chrono::Utc::now();
        let number = self.next_number(pool, create.project_id).await?;

        let id: i64 = sqlx::query_scalar(IssueQueryBuilder::insert())
            .bind(uuid)
            .bind(create.project_id)
            .bind(number)
            .bind(&create.title)
            .bind(&create.description)
            .bind(crate::models::issue::DbIssueState::Open)
            .bind(create.submitter_id)
            .bind(create.assignee_id)
            .bind(create.priority)
            .bind(create.milestone_id)
            .bind(create.estimated_duration)
            .bind(create.estimated_duration) // remaining_duration
            .bind(0) // spent_duration
            .bind(now) // created_at
            .bind(now) // updated_at
            .bind(create.due_date)
            .bind(&create.label_ids)
            .bind(&Vec::<i64>::new()) // watcher_ids
            .bind(&Vec::<i64>::new()) // voter_ids
            .fetch_one(pool)
            .await?;

        self.find_by_id(pool, id).await?.ok_or_else(|| crate::DbError::NotFound {
            entity_type: "Issue".to_string(),
            id: id.to_string(),
        })
    }

    async fn find_by_id(&self, pool: &DbPool, id: i64) -> DbResult<Option<DbIssue>> {
        let row = sqlx::query(IssueQueryBuilder::select_by_id())
            .bind(id)
            .fetch_optional(pool)
            .await?;

        Ok(row.map(|r| DbIssue::from_row(&r)).transpose()?)
    }

    async fn update(&self, pool: &DbPool, id: i64, update: DbUpdateIssue) -> DbResult<DbIssue> {
        let row = sqlx::query(IssueQueryBuilder::update())
            .bind(id)
            .bind(&update.title)
            .bind(&update.description)
            .bind(update.state)
            .bind(update.assignee_id)
            .bind(update.priority)
            .bind(update.milestone_id)
            .bind(update.estimated_duration)
            .bind(update.remaining_duration)
            .bind(update.spent_duration)
            .bind(update.due_date)
            .bind(update.label_ids.as_ref().map(|v| v.as_slice()))
            .bind(update.watcher_ids.as_ref().map(|v| v.as_slice()))
            .bind(update.voter_ids.as_ref().map(|v| v.as_slice()))
            .fetch_one(pool)
            .await?;

        DbIssue::from_row(&row)
    }

    async fn delete(&self, pool: &DbPool, id: i64) -> DbResult<()> {
        sqlx::query(IssueQueryBuilder::delete())
            .bind(id)
            .execute(pool)
            .await?;
        Ok(())
    }

    async fn list(&self, pool: &DbPool, limit: i64, offset: i64) -> DbResult<Vec<DbIssue>> {
        let rows = sqlx::query(
            "SELECT * FROM issues ORDER BY created_at DESC LIMIT $1 OFFSET $2"
        )
        .bind(limit)
        .bind(offset)
        .fetch_all(pool)
        .await?;

        let issues = rows.into_iter().map(|r| DbIssue::from_row(&r)).collect::<Result<Vec<_>, _>>()?;
        Ok(issues)
    }

    async fn count(&self, pool: &DbPool) -> DbResult<i64> {
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM issues")
            .fetch_one(pool)
            .await?;
        Ok(count)
    }
}