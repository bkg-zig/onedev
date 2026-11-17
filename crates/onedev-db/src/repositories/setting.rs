//! Setting Repository

use crate::{
    models::setting::{DbSystemSetting, DbProjectSetting, DbUserSetting, DbCreateSetting, DbUpdateSetting, SystemSettingQueryBuilder, ProjectSettingQueryBuilder},
    repositories::Repository,
    DbPool, DbResult,
};
use async_trait::async_trait;
use sqlx::FromRow;

pub struct SystemSettingRepository;

impl SystemSettingRepository {
    pub fn new() -> Self {
        Self
    }

    /// Find by key
    pub async fn find_by_key(&self, pool: &DbPool, key: &str) -> DbResult<Option<DbSystemSetting>> {
        let row = sqlx::query(SystemSettingQueryBuilder::select_by_key())
            .bind(key)
            .fetch_optional(pool)
            .await?;

        Ok(row.map(|r| DbSystemSetting::from_row(&r)).transpose()?)
    }

    /// Get value by key (the `value` column is JSONB, so it is deserialized via serde)
    pub async fn get_value<T: serde::de::DeserializeOwned>(
        &self,
        pool: &DbPool,
        key: &str,
    ) -> DbResult<Option<T>> {
        let value: Option<serde_json::Value> =
            sqlx::query_scalar("SELECT value FROM system_settings WHERE key = $1")
                .bind(key)
                .fetch_optional(pool)
                .await?;
        Ok(value.map(serde_json::from_value).transpose()?)
    }

    /// Set value by key (the `value` column is JSONB, so it is serialized via serde)
    pub async fn set_value<T: serde::Serialize + Send>(
        &self,
        pool: &DbPool,
        key: &str,
        value: T,
    ) -> DbResult<()> {
        sqlx::query(
            r#"
            INSERT INTO system_settings (uuid, key, value, created_at, updated_at)
            VALUES ($1, $2, $3, NOW(), NOW())
            ON CONFLICT (key) DO UPDATE SET
                value = EXCLUDED.value,
                updated_at = NOW()
            "#,
        )
        .bind(uuid::Uuid::new_v4())
        .bind(key)
        .bind(sqlx::types::Json(value))
        .execute(pool)
        .await?;
        Ok(())
    }
}

#[async_trait]
impl Repository<DbSystemSetting, DbCreateSetting, DbUpdateSetting> for SystemSettingRepository {
    async fn create(&self, pool: &DbPool, create: DbCreateSetting) -> DbResult<DbSystemSetting> {
        let uuid = uuid::Uuid::new_v4();
        let now = chrono::Utc::now();

        let id: i64 = sqlx::query_scalar(SystemSettingQueryBuilder::insert())
            .bind(uuid)
            .bind(&create.key)
            .bind(create.value)
            .bind(&create.description)
            .bind(&create.category)
            .bind(create.editable)
            .bind(now)
            .bind(now)
            .fetch_one(pool)
            .await?;

        self.find_by_id(pool, id).await?.ok_or_else(|| crate::DbError::NotFound {
            entity_type: "SystemSetting".to_string(),
            id: id.to_string(),
        })
    }

    async fn find_by_id(&self, pool: &DbPool, id: i64) -> DbResult<Option<DbSystemSetting>> {
        let row = sqlx::query(SystemSettingQueryBuilder::select_by_id())
            .bind(id)
            .fetch_optional(pool)
            .await?;

        Ok(row.map(|r| DbSystemSetting::from_row(&r)).transpose()?)
    }

    async fn update(&self, pool: &DbPool, id: i64, update: DbUpdateSetting) -> DbResult<DbSystemSetting> {
        let row = sqlx::query(SystemSettingQueryBuilder::update())
            .bind(id)
            .bind(update.value)
            .bind(&update.description)
            .bind(update.editable)
            .fetch_one(pool)
            .await?;

        Ok(DbSystemSetting::from_row(&row)?)
    }

    async fn delete(&self, pool: &DbPool, id: i64) -> DbResult<()> {
        sqlx::query(SystemSettingQueryBuilder::delete())
            .bind(id)
            .execute(pool)
            .await?;
        Ok(())
    }

    async fn list(&self, pool: &DbPool, limit: i64, offset: i64) -> DbResult<Vec<DbSystemSetting>> {
        let rows = sqlx::query(
            "SELECT * FROM system_settings ORDER BY category, key LIMIT $1 OFFSET $2"
        )
        .bind(limit)
        .bind(offset)
        .fetch_all(pool)
        .await?;

        let settings = rows.into_iter().map(|r| DbSystemSetting::from_row(&r)).collect::<Result<Vec<_>, _>>()?;
        Ok(settings)
    }

    async fn count(&self, pool: &DbPool) -> DbResult<i64> {
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM system_settings")
            .fetch_one(pool)
            .await?;
        Ok(count)
    }
}

pub struct ProjectSettingRepository;

impl ProjectSettingRepository {
    pub fn new() -> Self {
        Self
    }

    /// Find by project ID
    pub async fn find_by_project_id(&self, pool: &DbPool, project_id: i64) -> DbResult<Vec<DbProjectSetting>> {
        let rows = sqlx::query(ProjectSettingQueryBuilder::select_by_project_id())
            .bind(project_id)
            .fetch_all(pool)
            .await?;

        let settings = rows.into_iter().map(|r| DbProjectSetting::from_row(&r)).collect::<Result<Vec<_>, _>>()?;
        Ok(settings)
    }

    /// Find by project and key
    pub async fn find_by_project_and_key(&self, pool: &DbPool, project_id: i64, key: &str) -> DbResult<Option<DbProjectSetting>> {
        let row = sqlx::query(ProjectSettingQueryBuilder::select_by_project_and_key())
            .bind(project_id)
            .bind(key)
            .fetch_optional(pool)
            .await?;

        Ok(row.map(|r| DbProjectSetting::from_row(&r)).transpose()?)
    }

    /// Upsert setting
    pub async fn upsert(&self, pool: &DbPool, project_id: i64, key: &str, value: serde_json::Value) -> DbResult<i64> {
        let id: i64 = sqlx::query_scalar(ProjectSettingQueryBuilder::upsert())
            .bind(uuid::Uuid::new_v4())
            .bind(project_id)
            .bind(key)
            .bind(value)
            .bind(chrono::Utc::now())
            .bind(chrono::Utc::now())
            .fetch_one(pool)
            .await?;
        Ok(id)
    }

    /// Delete setting
    pub async fn delete(&self, pool: &DbPool, project_id: i64, key: &str) -> DbResult<()> {
        sqlx::query(ProjectSettingQueryBuilder::delete())
            .bind(project_id)
            .bind(key)
            .execute(pool)
            .await?;
        Ok(())
    }
}

pub struct UserSettingRepository;

impl UserSettingRepository {
    pub fn new() -> Self {
        Self
    }

    /// Find by user ID
    pub async fn find_by_user_id(&self, pool: &DbPool, user_id: i64) -> DbResult<Vec<DbUserSetting>> {
        let rows = sqlx::query(
            "SELECT * FROM user_settings WHERE user_id = $1 ORDER BY key"
        )
        .bind(user_id)
        .fetch_all(pool)
        .await?;

        let settings = rows.into_iter().map(|r| DbUserSetting::from_row(&r)).collect::<Result<Vec<_>, _>>()?;
        Ok(settings)
    }

    /// Find by user and key
    pub async fn find_by_user_and_key(&self, pool: &DbPool, user_id: i64, key: &str) -> DbResult<Option<DbUserSetting>> {
        let row = sqlx::query(
            "SELECT * FROM user_settings WHERE user_id = $1 AND key = $2"
        )
        .bind(user_id)
        .bind(key)
        .fetch_optional(pool)
        .await?;

        Ok(row.map(|r| DbUserSetting::from_row(&r)).transpose()?)
    }

    /// Upsert setting
    pub async fn upsert(&self, pool: &DbPool, user_id: i64, key: &str, value: serde_json::Value) -> DbResult<i64> {
        let id: i64 = sqlx::query_scalar(
            r#"
            INSERT INTO user_settings (uuid, user_id, key, value, created_at, updated_at)
            VALUES ($1, $2, $3, $4, NOW(), NOW())
            ON CONFLICT (user_id, key) DO UPDATE SET
                value = EXCLUDED.value,
                updated_at = NOW()
            RETURNING id
            "#,
        )
        .bind(uuid::Uuid::new_v4())
        .bind(user_id)
        .bind(key)
        .bind(value)
        .fetch_one(pool)
        .await?;
        Ok(id)
    }
}