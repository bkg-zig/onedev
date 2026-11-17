//! Dashboard Repository

use crate::{
    models::dashboard::{DbDashboard, DbCreateDashboard, DbUpdateDashboard, DbDashboardShare, DashboardQueryBuilder, DashboardShareQueryBuilder},
    repositories::Repository,
    DbPool, DbResult,
};
use async_trait::async_trait;
use sqlx::FromRow;

pub struct DashboardRepository;

impl DashboardRepository {
    pub fn new() -> Self {
        Self
    }

    /// Find by owner
    pub async fn find_by_owner(&self, pool: &DbPool, owner_id: i64, owner_type: crate::models::dashboard::DbDashboardOwnerType) -> DbResult<Vec<DbDashboard>> {
        let rows = sqlx::query(DashboardQueryBuilder::select_by_owner())
            .bind(owner_id)
            .bind(owner_type)
            .fetch_all(pool)
            .await?;

        let dashboards = rows.into_iter().map(|r| DbDashboard::from_row(&r)).collect::<Result<Vec<_>, _>>()?;
        Ok(dashboards)
    }

    /// Find default by owner
    pub async fn find_default_by_owner(&self, pool: &DbPool, owner_id: i64, owner_type: crate::models::dashboard::DbDashboardOwnerType) -> DbResult<Option<DbDashboard>> {
        let row = sqlx::query(DashboardQueryBuilder::select_default_by_owner())
            .bind(owner_id)
            .bind(owner_type)
            .fetch_optional(pool)
            .await?;

        Ok(row.map(|r| DbDashboard::from_row(&r)).transpose()?)
    }
}

#[async_trait]
impl Repository<DbDashboard, DbCreateDashboard, DbUpdateDashboard> for DashboardRepository {
    async fn create(&self, pool: &DbPool, create: DbCreateDashboard) -> DbResult<DbDashboard> {
        let uuid = uuid::Uuid::new_v4();
        let now = chrono::Utc::now();

        // If this is default, unset other defaults
        if create.is_default {
            sqlx::query(
                "UPDATE dashboards SET is_default = FALSE, updated_at = NOW() WHERE owner_id = $1 AND owner_type = $2 AND is_default = TRUE"
            )
            .bind(create.owner_id)
            .bind(create.owner_type)
            .execute(pool)
            .await?;
        }

        let id: i64 = sqlx::query_scalar(DashboardQueryBuilder::insert())
            .bind(uuid)
            .bind(create.owner_id)
            .bind(create.owner_type)
            .bind(&create.name)
            .bind(create.layout)
            .bind(create.is_default)
            .bind(now)
            .bind(now)
            .fetch_one(pool)
            .await?;

        self.find_by_id(pool, id).await?.ok_or_else(|| crate::DbError::NotFound {
            entity_type: "Dashboard".to_string(),
            id: id.to_string(),
        })
    }

    async fn find_by_id(&self, pool: &DbPool, id: i64) -> DbResult<Option<DbDashboard>> {
        let row = sqlx::query(DashboardQueryBuilder::select_by_id())
            .bind(id)
            .fetch_optional(pool)
            .await?;

        Ok(row.map(|r| DbDashboard::from_row(&r)).transpose()?)
    }

    async fn update(&self, pool: &DbPool, id: i64, update: DbUpdateDashboard) -> DbResult<DbDashboard> {
        // If setting as default, unset other defaults
        if update.is_default == Some(true) {
            let current = self.find_by_id(pool, id).await?;
            if let Some(dashboard) = current {
                sqlx::query(
                    "UPDATE dashboards SET is_default = FALSE, updated_at = NOW() WHERE owner_id = $1 AND owner_type = $2 AND is_default = TRUE AND id != $3"
                )
                .bind(dashboard.owner_id)
                .bind(dashboard.owner_type)
                .bind(id)
                .execute(pool)
                .await?;
            }
        }

        let row = sqlx::query(DashboardQueryBuilder::update())
            .bind(id)
            .bind(&update.name)
            .bind(update.layout)
            .bind(update.is_default)
            .fetch_one(pool)
            .await?;

        Ok(DbDashboard::from_row(&row)?)
    }

    async fn delete(&self, pool: &DbPool, id: i64) -> DbResult<()> {
        sqlx::query(DashboardQueryBuilder::delete())
            .bind(id)
            .execute(pool)
            .await?;
        Ok(())
    }

    async fn list(&self, pool: &DbPool, limit: i64, offset: i64) -> DbResult<Vec<DbDashboard>> {
        let rows = sqlx::query(
            "SELECT * FROM dashboards ORDER BY created_at DESC LIMIT $1 OFFSET $2"
        )
        .bind(limit)
        .bind(offset)
        .fetch_all(pool)
        .await?;

        let dashboards = rows.into_iter().map(|r| DbDashboard::from_row(&r)).collect::<Result<Vec<_>, _>>()?;
        Ok(dashboards)
    }

    async fn count(&self, pool: &DbPool) -> DbResult<i64> {
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM dashboards")
            .fetch_one(pool)
            .await?;
        Ok(count)
    }
}

pub struct DashboardShareRepository;

impl DashboardShareRepository {
    pub fn new() -> Self {
        Self
    }

    /// Find by dashboard ID
    pub async fn find_by_dashboard_id(&self, pool: &DbPool, dashboard_id: i64) -> DbResult<Vec<DbDashboardShare>> {
        let rows = sqlx::query(DashboardShareQueryBuilder::select_by_dashboard_id())
            .bind(dashboard_id)
            .fetch_all(pool)
            .await?;

        let shares = rows.into_iter().map(|r| DbDashboardShare::from_row(&r)).collect::<Result<Vec<_>, _>>()?;
        Ok(shares)
    }

    /// Find by dashboard and user
    pub async fn find_by_dashboard_and_user(&self, pool: &DbPool, dashboard_id: i64, user_id: i64) -> DbResult<Option<DbDashboardShare>> {
        let row = sqlx::query(DashboardShareQueryBuilder::select_by_dashboard_and_user())
            .bind(dashboard_id)
            .bind(user_id)
            .fetch_optional(pool)
            .await?;

        Ok(row.map(|r| DbDashboardShare::from_row(&r)).transpose()?)
    }
}

#[async_trait]
impl Repository<DbDashboardShare, DbDashboardShare, DbDashboardShare> for DashboardShareRepository {
    async fn create(&self, pool: &DbPool, create: DbDashboardShare) -> DbResult<DbDashboardShare> {
        let uuid = uuid::Uuid::new_v4();
        let now = chrono::Utc::now();

        let id: i64 = sqlx::query_scalar(DashboardShareQueryBuilder::insert())
            .bind(uuid)
            .bind(create.dashboard_id)
            .bind(create.user_id)
            .bind(create.permission)
            .bind(now)
            .fetch_one(pool)
            .await?;

        self.find_by_id(pool, id).await?.ok_or_else(|| crate::DbError::NotFound {
            entity_type: "DashboardShare".to_string(),
            id: id.to_string(),
        })
    }

    async fn find_by_id(&self, pool: &DbPool, id: i64) -> DbResult<Option<DbDashboardShare>> {
        let row = sqlx::query(DashboardShareQueryBuilder::select_by_id())
            .bind(id)
            .fetch_optional(pool)
            .await?;

        Ok(row.map(|r| DbDashboardShare::from_row(&r)).transpose()?)
    }

    async fn update(&self, pool: &DbPool, id: i64, update: DbDashboardShare) -> DbResult<DbDashboardShare> {
        let row = sqlx::query(DashboardShareQueryBuilder::update())
            .bind(id)
            .bind(update.permission)
            .fetch_one(pool)
            .await?;

        Ok(DbDashboardShare::from_row(&row)?)
    }

    async fn delete(&self, pool: &DbPool, id: i64) -> DbResult<()> {
        sqlx::query(DashboardShareQueryBuilder::delete())
            .bind(id)
            .execute(pool)
            .await?;
        Ok(())
    }

    async fn list(&self, pool: &DbPool, limit: i64, offset: i64) -> DbResult<Vec<DbDashboardShare>> {
        let rows = sqlx::query(
            "SELECT * FROM dashboard_shares ORDER BY created_at DESC LIMIT $1 OFFSET $2"
        )
        .bind(limit)
        .bind(offset)
        .fetch_all(pool)
        .await?;

        let shares = rows.into_iter().map(|r| DbDashboardShare::from_row(&r)).collect::<Result<Vec<_>, _>>()?;
        Ok(shares)
    }

    async fn count(&self, pool: &DbPool) -> DbResult<i64> {
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM dashboard_shares")
            .fetch_one(pool)
            .await?;
        Ok(count)
    }
}