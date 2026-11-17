//! Session repository

use crate::models::session::{Session, SessionQuery};
use chrono::{DateTime, Utc};
use onedev_db::{DbPool, DbResult};
use sqlx::Row;
use uuid::Uuid;

const SELECT_COLUMNS: &str = "
    id, user_id, username, user_agent, ip_address, created_at,
    last_accessed_at, expires_at, is_remember_me, two_factor_verified
";

pub struct SessionRepository;

impl SessionRepository {
    pub fn new() -> Self {
        Self
    }

    fn from_row(row: &sqlx::postgres::PgRow) -> Session {
        Session {
            id: row.get("id"),
            user_id: row.get("user_id"),
            username: row.get("username"),
            user_agent: row.get("user_agent"),
            ip_address: row.get("ip_address"),
            created_at: row.get("created_at"),
            last_accessed_at: row.get("last_accessed_at"),
            expires_at: row.get("expires_at"),
            is_remember_me: row.get("is_remember_me"),
            two_factor_verified: row.get("two_factor_verified"),
        }
    }

    pub async fn create(&self, pool: &DbPool, session: Session) -> DbResult<Session> {
        sqlx::query(
            "INSERT INTO sessions (
                id, user_id, username, user_agent, ip_address, created_at,
                last_accessed_at, expires_at, is_remember_me, two_factor_verified
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)",
        )
        .bind(session.id)
        .bind(session.user_id)
        .bind(&session.username)
        .bind(&session.user_agent)
        .bind(&session.ip_address)
        .bind(session.created_at)
        .bind(session.last_accessed_at)
        .bind(session.expires_at)
        .bind(session.is_remember_me)
        .bind(session.two_factor_verified)
        .execute(pool)
        .await?;
        Ok(session)
    }

    pub async fn find_by_id(&self, pool: &DbPool, id: Uuid) -> DbResult<Option<Session>> {
        let row = sqlx::query(&format!(
            "SELECT {SELECT_COLUMNS} FROM sessions WHERE id = $1"
        ))
        .bind(id)
        .fetch_optional(pool)
        .await?;
        Ok(row.as_ref().map(Self::from_row))
    }

    pub async fn find_by_user(&self, pool: &DbPool, user_id: i64) -> DbResult<Vec<Session>> {
        let rows = sqlx::query(&format!(
            "SELECT {SELECT_COLUMNS} FROM sessions WHERE user_id = $1 ORDER BY created_at DESC"
        ))
        .bind(user_id)
        .fetch_all(pool)
        .await?;
        Ok(rows.iter().map(Self::from_row).collect())
    }

    pub async fn update_last_accessed(
        &self,
        pool: &DbPool,
        id: Uuid,
        at: DateTime<Utc>,
    ) -> DbResult<()> {
        sqlx::query("UPDATE sessions SET last_accessed_at = $2 WHERE id = $1")
            .bind(id)
            .bind(at)
            .execute(pool)
            .await?;
        Ok(())
    }

    pub async fn delete(&self, pool: &DbPool, id: Uuid) -> DbResult<()> {
        sqlx::query("DELETE FROM sessions WHERE id = $1")
            .bind(id)
            .execute(pool)
            .await?;
        Ok(())
    }

    pub async fn delete_by_user(&self, pool: &DbPool, user_id: i64) -> DbResult<()> {
        sqlx::query("DELETE FROM sessions WHERE user_id = $1")
            .bind(user_id)
            .execute(pool)
            .await?;
        Ok(())
    }

    pub async fn delete_expired(&self, pool: &DbPool) -> DbResult<u64> {
        let result = sqlx::query("DELETE FROM sessions WHERE expires_at <= NOW()")
            .execute(pool)
            .await?;
        Ok(result.rows_affected())
    }

    pub async fn list_filtered(&self, pool: &DbPool, query: SessionQuery) -> DbResult<Vec<Session>> {
        let per_page = if query.per_page == 0 { 20 } else { query.per_page };
        let offset = i64::from(query.page.saturating_mul(per_page));
        let rows = sqlx::query(&format!(
            "SELECT {SELECT_COLUMNS}
             FROM sessions
             WHERE ($1::bigint IS NULL OR user_id = $1)
               AND (NOT $2 OR expires_at > NOW())
             ORDER BY created_at DESC
             LIMIT $3 OFFSET $4"
        ))
        .bind(query.user_id)
        .bind(query.active_only)
        .bind(i64::from(per_page))
        .bind(offset)
        .fetch_all(pool)
        .await?;
        Ok(rows.iter().map(Self::from_row).collect())
    }

    pub async fn count_filtered(&self, pool: &DbPool, query: SessionQuery) -> DbResult<i64> {
        let count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*)
             FROM sessions
             WHERE ($1::bigint IS NULL OR user_id = $1)
               AND (NOT $2 OR expires_at > NOW())",
        )
        .bind(query.user_id)
        .bind(query.active_only)
        .fetch_one(pool)
        .await?;
        Ok(count)
    }
}
