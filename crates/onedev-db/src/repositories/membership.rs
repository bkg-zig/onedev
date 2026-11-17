//! Membership Repository

use crate::{
    models::membership::{DbMembership, DbCreateMembership, DbUpdateMembership, MembershipQueryBuilder},
    repositories::Repository,
    DbPool, DbResult,
};
use async_trait::async_trait;
use sqlx::FromRow;

pub struct MembershipRepository;

impl MembershipRepository {
    pub fn new() -> Self {
        Self
    }

    /// Find by user ID
    pub async fn find_by_user_id(&self, pool: &DbPool, user_id: i64) -> DbResult<Vec<DbMembership>> {
        let rows = sqlx::query(MembershipQueryBuilder::select_by_user_id())
            .bind(user_id)
            .fetch_all(pool)
            .await?;

        let memberships = rows.into_iter().map(|r| DbMembership::from_row(&r)).collect::<Result<Vec<_>, _>>()?;
        Ok(memberships)
    }

    /// Find by group ID
    pub async fn find_by_group_id(&self, pool: &DbPool, group_id: i64) -> DbResult<Vec<DbMembership>> {
        let rows = sqlx::query(MembershipQueryBuilder::select_by_group_id())
            .bind(group_id)
            .fetch_all(pool)
            .await?;

        let memberships = rows.into_iter().map(|r| DbMembership::from_row(&r)).collect::<Result<Vec<_>, _>>()?;
        Ok(memberships)
    }

    /// Find by user and group
    pub async fn find_by_user_and_group(&self, pool: &DbPool, user_id: i64, group_id: i64) -> DbResult<Option<DbMembership>> {
        let row = sqlx::query(MembershipQueryBuilder::select_by_user_and_group())
            .bind(user_id)
            .bind(group_id)
            .fetch_optional(pool)
            .await?;

        Ok(row.map(|r| DbMembership::from_row(&r)).transpose()?)
    }

    /// Delete by user and group
    pub async fn delete_by_user_and_group(&self, pool: &DbPool, user_id: i64, group_id: i64) -> DbResult<()> {
        sqlx::query(MembershipQueryBuilder::delete_by_user_and_group())
            .bind(user_id)
            .bind(group_id)
            .execute(pool)
            .await?;
        Ok(())
    }
}

#[async_trait]
impl Repository<DbMembership, DbCreateMembership, DbUpdateMembership> for MembershipRepository {
    async fn create(&self, pool: &DbPool, create: DbCreateMembership) -> DbResult<DbMembership> {
        let uuid = uuid::Uuid::new_v4();
        let now = chrono::Utc::now();

        let id: i64 = sqlx::query_scalar(MembershipQueryBuilder::insert())
            .bind(uuid)
            .bind(create.user_id)
            .bind(create.group_id)
            .bind(create.role)
            .bind(now)
            .bind(now)
            .fetch_one(pool)
            .await?;

        self.find_by_id(pool, id).await?.ok_or_else(|| crate::DbError::NotFound {
            entity_type: "Membership".to_string(),
            id: id.to_string(),
        })
    }

    async fn find_by_id(&self, pool: &DbPool, id: i64) -> DbResult<Option<DbMembership>> {
        let row = sqlx::query(MembershipQueryBuilder::select_by_id())
            .bind(id)
            .fetch_optional(pool)
            .await?;

        Ok(row.map(|r| DbMembership::from_row(&r)).transpose()?)
    }

    async fn update(&self, pool: &DbPool, id: i64, update: DbUpdateMembership) -> DbResult<DbMembership> {
        let row = sqlx::query(MembershipQueryBuilder::update())
            .bind(id)
            .bind(update.role)
            .fetch_one(pool)
            .await?;

        Ok(DbMembership::from_row(&row)?)
    }

    async fn delete(&self, pool: &DbPool, id: i64) -> DbResult<()> {
        sqlx::query(MembershipQueryBuilder::delete())
            .bind(id)
            .execute(pool)
            .await?;
        Ok(())
    }

    async fn list(&self, pool: &DbPool, limit: i64, offset: i64) -> DbResult<Vec<DbMembership>> {
        let rows = sqlx::query(
            "SELECT * FROM memberships ORDER BY created_at DESC LIMIT $1 OFFSET $2"
        )
        .bind(limit)
        .bind(offset)
        .fetch_all(pool)
        .await?;

        let memberships = rows.into_iter().map(|r| DbMembership::from_row(&r)).collect::<Result<Vec<_>, _>>()?;
        Ok(memberships)
    }

    async fn count(&self, pool: &DbPool) -> DbResult<i64> {
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM memberships")
            .fetch_one(pool)
            .await?;
        Ok(count)
    }
}