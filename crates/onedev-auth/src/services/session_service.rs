//! Session Service - Session management

use crate::models::session::{Session, CreateSession, SessionQuery};
use onedev_db::repositories::SessionRepository;
use onedev_db::{DbPool, DbResult};
use uuid::Uuid;
use chrono::{DateTime, Utc};

/// Session service
pub struct SessionService {
    repo: SessionRepository,
    db_pool: DbPool,
}

impl SessionService {
    pub fn new(db_pool: DbPool) -> Self {
        Self {
            repo: SessionRepository::new(),
            db_pool,
        }
    }

    /// Create a new session
    pub async fn create(&self, create: CreateSession) -> DbResult<Session> {
        let session = Session::new(create);
        self.repo.create(&self.db_pool, session).await
    }

    /// Find session by ID
    pub async fn find_by_id(&self, id: Uuid) -> DbResult<Option<Session>> {
        self.repo.find_by_id(&self.db_pool, id).await
    }

    /// Find session by user ID
    pub async fn find_by_user(&self, user_id: i64) -> DbResult<Vec<Session>> {
        self.repo.find_by_user(&self.db_pool, user_id).await
    }

    /// Update last accessed time
    pub async fn touch(&self, id: Uuid) -> DbResult<()> {
        self.repo.update_last_accessed(&self.db_pool, id, Utc::now()).await
    }

    /// Delete a session
    pub async fn delete(&self, id: Uuid) -> DbResult<()> {
        self.repo.delete(&self.db_pool, id).await
    }

    /// Delete all sessions for a user
    pub async fn delete_by_user(&self, user_id: i64) -> DbResult<()> {
        self.repo.delete_by_user(&self.db_pool, user_id).await
    }

    /// Delete expired sessions
    pub async fn delete_expired(&self) -> DbResult<u64> {
        self.repo.delete_expired(&self.db_pool).await
    }

    /// List sessions with filters
    pub async fn list(&self, query: SessionQuery) -> DbResult<Vec<Session>> {
        self.repo.list_filtered(&self.db_pool, query).await
    }

    /// Count sessions with filters
    pub async fn count(&self, query: SessionQuery) -> DbResult<i64> {
        self.repo.count_filtered(&self.db_pool, query).await
    }
}