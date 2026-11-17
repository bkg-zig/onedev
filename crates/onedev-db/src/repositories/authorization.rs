//! Authorization Repository

use crate::{
    models::authorization::{DbUserAuthorization, DbCreateUserAuthorization, DbUpdateUserAuthorization, DbIssueAuthorization, UserAuthorizationQueryBuilder, IssueAuthorizationQueryBuilder},
    repositories::Repository,
    DbPool, DbResult,
};
use async_trait::async_trait;
use sqlx::FromRow;

pub struct UserAuthorizationRepository;

impl UserAuthorizationRepository {
    pub fn new() -> Self {
        Self
    }

    /// Find by user ID
    pub async fn find_by_user_id(&self, pool: &DbPool, user_id: i64) -> DbResult<Vec<DbUserAuthorization>> {
        let rows = sqlx::query(UserAuthorizationQueryBuilder::select_by_user_id())
            .bind(user_id)
            .fetch_all(pool)
            .await?;

        let auths = rows.into_iter().map(|r| DbUserAuthorization::from_row(&r)).collect::<Result<Vec<_>, _>>()?;
        Ok(auths)
    }

    /// Find by project ID
    pub async fn find_by_project_id(&self, pool: &DbPool, project_id: i64) -> DbResult<Vec<DbUserAuthorization>> {
        let rows = sqlx::query(UserAuthorizationQueryBuilder::select_by_project_id())
            .bind(project_id)
            .fetch_all(pool)
            .await?;

        let auths = rows.into_iter().map(|r| DbUserAuthorization::from_row(&r)).collect::<Result<Vec<_>, _>>()?;
        Ok(auths)
    }

    /// Find by user and project
    pub async fn find_by_user_and_project(&self, pool: &DbPool, user_id: i64, project_id: i64) -> DbResult<Option<DbUserAuthorization>> {
        let row = sqlx::query(UserAuthorizationQueryBuilder::select_by_user_and_project())
            .bind(user_id)
            .bind(project_id)
            .fetch_optional(pool)
            .await?;

        Ok(row.map(|r| DbUserAuthorization::from_row(&r)).transpose()?)
    }

    /// Get user's highest role in project
    pub async fn get_highest_role(&self, pool: &DbPool, user_id: i64, project_id: i64) -> DbResult<Option<crate::models::authorization::DbProjectRole>> {
        let role: Option<crate::models::authorization::DbProjectRole> = sqlx::query_scalar(
            r#"
            SELECT role FROM user_authorizations
            WHERE user_id = $1 AND project_id = $2
            ORDER BY CASE role
                WHEN 'owner' THEN 5
                WHEN 'maintainer' THEN 4
                WHEN 'developer' THEN 3
                WHEN 'reporter' THEN 2
                WHEN 'guest' THEN 1
                ELSE 0
            END DESC
            LIMIT 1
            "#
        )
        .bind(user_id)
        .bind(project_id)
        .fetch_optional(pool)
        .await?;

        Ok(role)
    }
}

#[async_trait]
impl Repository<DbUserAuthorization, DbCreateUserAuthorization, DbUpdateUserAuthorization> for UserAuthorizationRepository {
    async fn create(&self, pool: &DbPool, create: DbCreateUserAuthorization) -> DbResult<DbUserAuthorization> {
        let uuid = uuid::Uuid::new_v4();
        let now = chrono::Utc::now();

        let id: i64 = sqlx::query_scalar(UserAuthorizationQueryBuilder::insert())
            .bind(uuid)
            .bind(create.user_id)
            .bind(create.project_id)
            .bind(create.role)
            .bind(now)
            .bind(now)
            .fetch_one(pool)
            .await?;

        self.find_by_id(pool, id).await?.ok_or_else(|| crate::DbError::NotFound {
            entity_type: "UserAuthorization".to_string(),
            id: id.to_string(),
        })
    }

    async fn find_by_id(&self, pool: &DbPool, id: i64) -> DbResult<Option<DbUserAuthorization>> {
        let row = sqlx::query(UserAuthorizationQueryBuilder::select_by_id())
            .bind(id)
            .fetch_optional(pool)
            .await?;

        Ok(row.map(|r| DbUserAuthorization::from_row(&r)).transpose()?)
    }

    async fn update(&self, pool: &DbPool, id: i64, update: DbUpdateUserAuthorization) -> DbResult<DbUserAuthorization> {
        let row = sqlx::query(UserAuthorizationQueryBuilder::update())
            .bind(id)
            .bind(update.role)
            .fetch_one(pool)
            .await?;

        Ok(DbUserAuthorization::from_row(&row)?)
    }

    async fn delete(&self, pool: &DbPool, id: i64) -> DbResult<()> {
        sqlx::query(UserAuthorizationQueryBuilder::delete())
            .bind(id)
            .execute(pool)
            .await?;
        Ok(())
    }

    async fn list(&self, pool: &DbPool, limit: i64, offset: i64) -> DbResult<Vec<DbUserAuthorization>> {
        let rows = sqlx::query(
            "SELECT * FROM user_authorizations ORDER BY created_at DESC LIMIT $1 OFFSET $2"
        )
        .bind(limit)
        .bind(offset)
        .fetch_all(pool)
        .await?;

        let auths = rows.into_iter().map(|r| DbUserAuthorization::from_row(&r)).collect::<Result<Vec<_>, _>>()?;
        Ok(auths)
    }

    async fn count(&self, pool: &DbPool) -> DbResult<i64> {
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM user_authorizations")
            .fetch_one(pool)
            .await?;
        Ok(count)
    }
}

pub struct IssueAuthorizationRepository;

impl IssueAuthorizationRepository {
    pub fn new() -> Self {
        Self
    }

    /// Find by user ID
    pub async fn find_by_user_id(&self, pool: &DbPool, user_id: i64) -> DbResult<Vec<DbIssueAuthorization>> {
        let rows = sqlx::query(IssueAuthorizationQueryBuilder::select_by_user_id())
            .bind(user_id)
            .fetch_all(pool)
            .await?;

        let auths = rows.into_iter().map(|r| DbIssueAuthorization::from_row(&r)).collect::<Result<Vec<_>, _>>()?;
        Ok(auths)
    }

    /// Find by issue ID
    pub async fn find_by_issue_id(&self, pool: &DbPool, issue_id: i64) -> DbResult<Vec<DbIssueAuthorization>> {
        let rows = sqlx::query(IssueAuthorizationQueryBuilder::select_by_issue_id())
            .bind(issue_id)
            .fetch_all(pool)
            .await?;

        let auths = rows.into_iter().map(|r| DbIssueAuthorization::from_row(&r)).collect::<Result<Vec<_>, _>>()?;
        Ok(auths)
    }
}

#[async_trait]
impl Repository<DbIssueAuthorization, DbIssueAuthorization, DbIssueAuthorization> for IssueAuthorizationRepository {
    async fn create(&self, pool: &DbPool, create: DbIssueAuthorization) -> DbResult<DbIssueAuthorization> {
        let uuid = uuid::Uuid::new_v4();
        let now = chrono::Utc::now();

        let id: i64 = sqlx::query_scalar(IssueAuthorizationQueryBuilder::insert())
            .bind(uuid)
            .bind(create.user_id)
            .bind(create.issue_id)
            .bind(create.permission)
            .bind(now)
            .fetch_one(pool)
            .await?;

        self.find_by_id(pool, id).await?.ok_or_else(|| crate::DbError::NotFound {
            entity_type: "IssueAuthorization".to_string(),
            id: id.to_string(),
        })
    }

    async fn find_by_id(&self, pool: &DbPool, id: i64) -> DbResult<Option<DbIssueAuthorization>> {
        let row = sqlx::query("SELECT * FROM issue_authorizations WHERE id = $1")
            .bind(id)
            .fetch_optional(pool)
            .await?;

        Ok(row.map(|r| DbIssueAuthorization::from_row(&r)).transpose()?)
    }

    async fn update(&self, pool: &DbPool, id: i64, update: DbIssueAuthorization) -> DbResult<DbIssueAuthorization> {
        let row = sqlx::query(
            "UPDATE issue_authorizations SET permission = $2 WHERE id = $1 RETURNING *"
        )
        .bind(id)
        .bind(update.permission)
        .fetch_one(pool)
        .await?;

        Ok(DbIssueAuthorization::from_row(&row)?)
    }

    async fn delete(&self, pool: &DbPool, id: i64) -> DbResult<()> {
        sqlx::query(IssueAuthorizationQueryBuilder::delete())
            .bind(id)
            .execute(pool)
            .await?;
        Ok(())
    }

    async fn list(&self, pool: &DbPool, limit: i64, offset: i64) -> DbResult<Vec<DbIssueAuthorization>> {
        let rows = sqlx::query("SELECT * FROM issue_authorizations ORDER BY created_at DESC LIMIT $1 OFFSET $2")
            .bind(limit)
            .bind(offset)
            .fetch_all(pool)
            .await?;

        let auths = rows.into_iter().map(|r| DbIssueAuthorization::from_row(&r)).collect::<Result<Vec<_>, _>>()?;
        Ok(auths)
    }

    async fn count(&self, pool: &DbPool) -> DbResult<i64> {
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM issue_authorizations")
            .fetch_one(pool)
            .await?;
        Ok(count)
    }
}