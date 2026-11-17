//! User Repository

use crate::{
    models::user::{DbUser, DbCreateUser, DbUpdateUser, UserQueryBuilder},
    repositories::Repository,
    DbPool, DbResult,
};
use async_trait::async_trait;
use sqlx::{FromRow, Row};

pub struct UserRepository;

impl UserRepository {
    pub fn new() -> Self {
        Self
    }

    /// Find user by username
    pub async fn find_by_name(&self, pool: &DbPool, name: &str) -> DbResult<Option<DbUser>> {
        let row = sqlx::query(UserQueryBuilder::select_by_name())
            .bind(name)
            .fetch_optional(pool)
            .await?;

        Ok(row.map(|r| DbUser::from_row(&r)).transpose()?)
    }

    /// Find user by email
    pub async fn find_by_email(&self, pool: &DbPool, email: &str) -> DbResult<Option<DbUser>> {
        let row = sqlx::query(UserQueryBuilder::select_by_email())
            .bind(email)
            .fetch_optional(pool)
            .await?;

        Ok(row.map(|r| DbUser::from_row(&r)).transpose()?)
    }

    /// Find user by UUID
    pub async fn find_by_uuid(&self, pool: &DbPool, uuid: uuid::Uuid) -> DbResult<Option<DbUser>> {
        let row = sqlx::query(UserQueryBuilder::select_by_uuid())
            .bind(uuid)
            .fetch_optional(pool)
            .await?;

        Ok(row.map(|r| DbUser::from_row(&r)).transpose()?)
    }

    /// Update last login timestamp
    pub async fn update_last_login(&self, pool: &DbPool, id: i64) -> DbResult<()> {
        sqlx::query("UPDATE users SET last_login_at = NOW(), updated_at = NOW() WHERE id = $1")
            .bind(id)
            .execute(pool)
            .await?;
        Ok(())
    }

    /// Update 2FA settings
    pub async fn update_two_factor(
        &self,
        pool: &DbPool,
        id: i64,
        enabled: bool,
        secret: Option<String>,
        backup_codes: Option<Vec<String>>,
    ) -> DbResult<()> {
        sqlx::query(
            r#"
            UPDATE users SET
                two_factor_enabled = $2,
                two_factor_secret = $3,
                backup_codes = COALESCE($4, '[]'::jsonb),
                updated_at = NOW()
            WHERE id = $1
            "#,
        )
        .bind(id)
        .bind(enabled)
        .bind(secret)
        .bind(backup_codes.map(serde_json::to_value).transpose()?)
        .execute(pool)
        .await?;
        Ok(())
    }

    /// List users with filters
    pub async fn list_filtered(
        &self,
        pool: &DbPool,
        term: Option<&str>,
        user_type: Option<crate::models::user::DbUserType>,
        status: Option<crate::models::user::DbUserStatus>,
        page: u32,
        per_page: u32,
    ) -> DbResult<Vec<DbUser>> {
        let mut query = String::from(UserQueryBuilder::select_all());
        query.push_str(" WHERE deleted_at IS NULL");

        let mut conditions = Vec::new();
        let mut param_idx = 1;

        if let Some(_term) = term {
            conditions.push(format!("(name ILIKE ${} OR full_name ILIKE ${} OR email ILIKE ${})", param_idx, param_idx, param_idx));
            param_idx += 1;
        }

        if user_type.is_some() {
            conditions.push(format!("user_type = ${}", param_idx));
            param_idx += 1;
        }

        if status.is_some() {
            conditions.push(format!("status = ${}", param_idx));
            param_idx += 1;
        }

        if !conditions.is_empty() {
            query.push_str(" AND ");
            query.push_str(&conditions.join(" AND "));
        }

        query.push_str(" ORDER BY created_at DESC LIMIT $");
        query.push_str(&param_idx.to_string());
        param_idx += 1;
        query.push_str(" OFFSET $");
        query.push_str(&param_idx.to_string());

        let mut q = sqlx::query(&query);

        if let Some(term) = term {
            let pattern = format!("%{}%", term);
            q = q.bind(pattern);
        }

        if let Some(ut) = user_type {
            q = q.bind(ut);
        }

        if let Some(s) = status {
            q = q.bind(s);
        }

        q = q.bind(per_page as i64);
        q = q.bind((page * per_page) as i64);

        let rows = q.fetch_all(pool).await?;
        let users = rows.into_iter().map(|r| DbUser::from_row(&r)).collect::<Result<Vec<_>, _>>()?;
        Ok(users)
    }

    /// Count users with filters
    pub async fn count_filtered(
        &self,
        pool: &DbPool,
        term: Option<&str>,
        user_type: Option<crate::models::user::DbUserType>,
        status: Option<crate::models::user::DbUserStatus>,
    ) -> DbResult<i64> {
        let mut query = String::from("SELECT COUNT(*) FROM users WHERE deleted_at IS NULL");
        let mut conditions = Vec::new();
        let mut param_idx = 1;

        if let Some(_term) = term {
            conditions.push(format!("(name ILIKE ${} OR full_name ILIKE ${} OR email ILIKE ${})", param_idx, param_idx, param_idx));
            param_idx += 1;
        }

        if user_type.is_some() {
            conditions.push(format!("user_type = ${}", param_idx));
            param_idx += 1;
        }

        if status.is_some() {
            conditions.push(format!("status = ${}", param_idx));
            param_idx += 1;
        }

        if !conditions.is_empty() {
            query.push_str(" AND ");
            query.push_str(&conditions.join(" AND "));
        }

        let mut q = sqlx::query_scalar(&query);

        if let Some(term) = term {
            let pattern = format!("%{}%", term);
            q = q.bind(pattern);
        }

        if let Some(ut) = user_type {
            q = q.bind(ut);
        }

        if let Some(s) = status {
            q = q.bind(s);
        }

        let count: i64 = q.fetch_one(pool).await?;
        Ok(count)
    }
}

#[async_trait]
impl Repository<DbUser, DbCreateUser, DbUpdateUser> for UserRepository {
    async fn create(&self, pool: &DbPool, create: DbCreateUser) -> DbResult<DbUser> {
        let uuid = uuid::Uuid::new_v4();
        let now = chrono::Utc::now();

        let id: i64 = sqlx::query_scalar(UserQueryBuilder::insert())
            .bind(uuid)
            .bind(&create.name)
            .bind(&create.full_name)
            .bind(&create.email)
            .bind(&create.password_hash)
            .bind(create.user_type)
            .bind(crate::models::user::DbUserStatus::Active)
            .bind(&create.avatar_url)
            .bind(false) // two_factor_enabled
            .bind(Option::<String>::None) // two_factor_secret
            .bind(sqlx::types::Json(Vec::<String>::new())) // backup_codes (JSONB)
            .bind(now)
            .bind(now)
            .bind(create.notify_own_events)
            .bind(create.keep_emails_private)
            .bind(&create.timezone)
            .bind(&create.locale)
            .bind(Option::<serde_json::Value>::None) // ai_setting
            .bind(Option::<serde_json::Value>::None) // two_factor_auth
            .fetch_one(pool)
            .await?;

        self.find_by_id(pool, id).await?.ok_or_else(|| crate::DbError::NotFound {
            entity_type: "User".to_string(),
            id: id.to_string(),
        })
    }

    async fn find_by_id(&self, pool: &DbPool, id: i64) -> DbResult<Option<DbUser>> {
        let row = sqlx::query(UserQueryBuilder::select_by_id())
            .bind(id)
            .fetch_optional(pool)
            .await?;

        Ok(row.map(|r| DbUser::from_row(&r)).transpose()?)
    }

    async fn update(&self, pool: &DbPool, id: i64, update: DbUpdateUser) -> DbResult<DbUser> {
        let ai_setting_json = update.ai_setting.map(serde_json::to_value).transpose()?;
        let two_factor_auth_json = update.two_factor_auth.map(serde_json::to_value).transpose()?;
        let backup_codes_json = update.backup_codes.map(serde_json::to_value).transpose()?;

        let row = sqlx::query(UserQueryBuilder::update())
            .bind(id)
            .bind(&update.full_name)
            .bind(&update.email)
            .bind(&update.avatar_url)
            .bind(update.user_type)
            .bind(update.status)
            .bind(&update.password_hash)
            .bind(update.two_factor_enabled)
            .bind(&update.two_factor_secret)
            .bind(backup_codes_json)
            .bind(update.last_login_at)
            .bind(update.notify_own_events)
            .bind(update.keep_emails_private)
            .bind(&update.timezone)
            .bind(&update.locale)
            .bind(ai_setting_json)
            .bind(two_factor_auth_json)
            .fetch_one(pool)
            .await?;

        Ok(DbUser::from_row(&row)?)
    }

    async fn delete(&self, pool: &DbPool, id: i64) -> DbResult<()> {
        sqlx::query(UserQueryBuilder::delete())
            .bind(id)
            .execute(pool)
            .await?;
        Ok(())
    }

    async fn list(&self, pool: &DbPool, limit: i64, offset: i64) -> DbResult<Vec<DbUser>> {
        let rows = sqlx::query(UserQueryBuilder::list_with_pagination())
            .bind(limit)
            .bind(offset)
            .fetch_all(pool)
            .await?;

        let users = rows.into_iter().map(|r| DbUser::from_row(&r)).collect::<Result<Vec<_>, _>>()?;
        Ok(users)
    }

    async fn count(&self, pool: &DbPool) -> DbResult<i64> {
        let count: i64 = sqlx::query_scalar(UserQueryBuilder::count())
            .fetch_one(pool)
            .await?;
        Ok(count)
    }
}