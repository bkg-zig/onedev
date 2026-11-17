//! Authorization Repositories — Vanilla authorization entities.

use crate::{
    models::authorization::{
        DbAccessTokenAuthorization, DbBaseAuthorization, DbCreateIssueAuthorization,
        DbCreateUserAuthorization, DbGroupAuthorization, DbIssueAuthorization,
        DbLinkAuthorization, DbUpdateUserAuthorization, DbUserAuthorization,
        IssueAuthorizationQueryBuilder, UserAuthorizationQueryBuilder,
    },
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

    pub async fn find_by_user_id(&self, pool: &DbPool, user_id: i64) -> DbResult<Vec<DbUserAuthorization>> {
        let rows = sqlx::query(UserAuthorizationQueryBuilder::select_by_user_id())
            .bind(user_id)
            .fetch_all(pool)
            .await?;
        Ok(rows.into_iter().map(|r| DbUserAuthorization::from_row(&r)).collect::<Result<Vec<_>, _>>()?)
    }

    pub async fn find_by_project_id(&self, pool: &DbPool, project_id: i64) -> DbResult<Vec<DbUserAuthorization>> {
        let rows = sqlx::query(UserAuthorizationQueryBuilder::select_by_project_id())
            .bind(project_id)
            .fetch_all(pool)
            .await?;
        Ok(rows.into_iter().map(|r| DbUserAuthorization::from_row(&r)).collect::<Result<Vec<_>, _>>()?)
    }

    /// All authorizations of a user on a project (a user can hold
    /// several roles on the same project in vanilla).
    pub async fn find_by_user_and_project(&self, pool: &DbPool, user_id: i64, project_id: i64) -> DbResult<Vec<DbUserAuthorization>> {
        let rows = sqlx::query(UserAuthorizationQueryBuilder::select_by_user_and_project())
            .bind(user_id)
            .bind(project_id)
            .fetch_all(pool)
            .await?;
        Ok(rows.into_iter().map(|r| DbUserAuthorization::from_row(&r)).collect::<Result<Vec<_>, _>>()?)
    }

    pub async fn find_group_authorizations(&self, pool: &DbPool, group_ids: &[i64], project_id: i64) -> DbResult<Vec<DbGroupAuthorization>> {
        if group_ids.is_empty() {
            return Ok(Vec::new());
        }
        let rows = sqlx::query(
            "SELECT id, uuid, group_id, project_id, role_id, created_at, updated_at \
             FROM group_authorizations WHERE group_id = ANY($1) AND project_id = $2",
        )
        .bind(group_ids)
        .bind(project_id)
        .fetch_all(pool)
        .await?;
        Ok(rows.into_iter().map(|r| DbGroupAuthorization::from_row(&r)).collect::<Result<Vec<_>, _>>()?)
    }

    pub async fn find_base_authorizations(&self, pool: &DbPool, project_id: i64) -> DbResult<Vec<DbBaseAuthorization>> {
        let rows = sqlx::query(
            "SELECT id, uuid, project_id, role_id, created_at, updated_at \
             FROM base_authorizations WHERE project_id = $1",
        )
        .bind(project_id)
        .fetch_all(pool)
        .await?;
        Ok(rows.into_iter().map(|r| DbBaseAuthorization::from_row(&r)).collect::<Result<Vec<_>, _>>()?)
    }

    pub async fn find_access_token_authorizations(&self, pool: &DbPool, access_token_id: i64, project_id: i64) -> DbResult<Vec<DbAccessTokenAuthorization>> {
        let rows = sqlx::query(
            "SELECT id, uuid, project_id, access_token_id, role_id, created_at, updated_at \
             FROM access_token_authorizations WHERE access_token_id = $1 AND project_id = $2",
        )
        .bind(access_token_id)
        .bind(project_id)
        .fetch_all(pool)
        .await?;
        Ok(rows.into_iter().map(|r| DbAccessTokenAuthorization::from_row(&r)).collect::<Result<Vec<_>, _>>()?)
    }

    pub async fn find_link_authorizations(&self, pool: &DbPool, role_id: i64) -> DbResult<Vec<DbLinkAuthorization>> {
        let rows = sqlx::query(
            "SELECT id, uuid, link_name, role_id, created_at FROM link_authorizations WHERE role_id = $1",
        )
        .bind(role_id)
        .fetch_all(pool)
        .await?;
        Ok(rows.into_iter().map(|r| DbLinkAuthorization::from_row(&r)).collect::<Result<Vec<_>, _>>()?)
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
            .bind(create.role_id)
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
            .bind(update.role_id)
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
            "SELECT id, uuid, user_id, project_id, role_id, created_at, updated_at FROM user_authorizations ORDER BY created_at DESC LIMIT $1 OFFSET $2"
        )
        .bind(limit)
        .bind(offset)
        .fetch_all(pool)
        .await?;
        Ok(rows.into_iter().map(|r| DbUserAuthorization::from_row(&r)).collect::<Result<Vec<_>, _>>()?)
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

    pub async fn find_by_user_id(&self, pool: &DbPool, user_id: i64) -> DbResult<Vec<DbIssueAuthorization>> {
        let rows = sqlx::query(IssueAuthorizationQueryBuilder::select_by_user_id())
            .bind(user_id)
            .fetch_all(pool)
            .await?;
        Ok(rows.into_iter().map(|r| DbIssueAuthorization::from_row(&r)).collect::<Result<Vec<_>, _>>()?)
    }

    pub async fn find_by_issue_id(&self, pool: &DbPool, issue_id: i64) -> DbResult<Vec<DbIssueAuthorization>> {
        let rows = sqlx::query(IssueAuthorizationQueryBuilder::select_by_issue_id())
            .bind(issue_id)
            .fetch_all(pool)
            .await?;
        Ok(rows.into_iter().map(|r| DbIssueAuthorization::from_row(&r)).collect::<Result<Vec<_>, _>>()?)
    }
}

#[async_trait]
impl Repository<DbIssueAuthorization, DbCreateIssueAuthorization, DbIssueAuthorization> for IssueAuthorizationRepository {
    async fn create(&self, pool: &DbPool, create: DbCreateIssueAuthorization) -> DbResult<DbIssueAuthorization> {
        let uuid = uuid::Uuid::new_v4();
        let now = chrono::Utc::now();

        let id: i64 = sqlx::query_scalar(IssueAuthorizationQueryBuilder::insert())
            .bind(uuid)
            .bind(create.user_id)
            .bind(create.issue_id)
            .bind(now)
            .fetch_one(pool)
            .await?;

        self.find_by_id(pool, id).await?.ok_or_else(|| crate::DbError::NotFound {
            entity_type: "IssueAuthorization".to_string(),
            id: id.to_string(),
        })
    }

    async fn find_by_id(&self, pool: &DbPool, id: i64) -> DbResult<Option<DbIssueAuthorization>> {
        let row = sqlx::query(IssueAuthorizationQueryBuilder::select_by_id())
            .bind(id)
            .fetch_optional(pool)
            .await?;
        Ok(row.map(|r| DbIssueAuthorization::from_row(&r)).transpose()?)
    }

    async fn update(&self, pool: &DbPool, id: i64, _update: DbIssueAuthorization) -> DbResult<DbIssueAuthorization> {
        // Vanilla IssueAuthorization has no mutable fields besides the pair.
        self.find_by_id(pool, id).await?.ok_or_else(|| crate::DbError::NotFound {
            entity_type: "IssueAuthorization".to_string(),
            id: id.to_string(),
        })
    }

    async fn delete(&self, pool: &DbPool, id: i64) -> DbResult<()> {
        sqlx::query(IssueAuthorizationQueryBuilder::delete())
            .bind(id)
            .execute(pool)
            .await?;
        Ok(())
    }

    async fn list(&self, pool: &DbPool, limit: i64, offset: i64) -> DbResult<Vec<DbIssueAuthorization>> {
        let rows = sqlx::query("SELECT id, uuid, user_id, issue_id, created_at FROM issue_authorizations ORDER BY created_at DESC LIMIT $1 OFFSET $2")
            .bind(limit)
            .bind(offset)
            .fetch_all(pool)
            .await?;
        Ok(rows.into_iter().map(|r| DbIssueAuthorization::from_row(&r)).collect::<Result<Vec<_>, _>>()?)
    }

    async fn count(&self, pool: &DbPool) -> DbResult<i64> {
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM issue_authorizations")
            .fetch_one(pool)
            .await?;
        Ok(count)
    }
}
