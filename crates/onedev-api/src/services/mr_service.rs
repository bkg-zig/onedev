//! Merge Request Service — real DB-backed pull request operations.

use crate::errors::AuthError;
use onedev_db::models::pull_request::{DbCreatePullRequest, DbMergeStrategy, DbPullRequest, DbUpdatePullRequest};
use onedev_db::repositories::{PullRequestRepository, Repository};
use onedev_db::DbPool;

pub struct MergeRequestService {
    db_pool: DbPool,
    mrs: PullRequestRepository,
}

fn db_err(err: onedev_db::DbError) -> AuthError {
    AuthError::Internal(err.to_string())
}

impl MergeRequestService {
    pub fn new(db_pool: DbPool) -> Self {
        Self {
            db_pool,
            mrs: PullRequestRepository::new(),
        }
    }

    pub async fn list(&self, project_id: i64, limit: i64, offset: i64) -> Result<Vec<DbPullRequest>, AuthError> {
        let all = self.mrs.list(&self.db_pool, i64::MAX, 0).await.map_err(db_err)?;
        Ok(all
            .into_iter()
            .filter(|mr| mr.project_id == project_id || mr.target_project_id == project_id)
            .skip(offset as usize)
            .take(limit as usize)
            .collect())
    }

    pub async fn count(&self, project_id: i64) -> Result<i64, AuthError> {
        let all = self.mrs.list(&self.db_pool, i64::MAX, 0).await.map_err(db_err)?;
        Ok(all
            .into_iter()
            .filter(|mr| mr.project_id == project_id || mr.target_project_id == project_id)
            .count() as i64)
    }

    pub async fn get(&self, project_id: i64, id: i64) -> Result<DbPullRequest, AuthError> {
        let mr = self
            .mrs
            .find_by_id(&self.db_pool, id)
            .await
            .map_err(db_err)?
            .ok_or_else(|| AuthError::Internal("merge request not found".to_owned()))?;
        if mr.project_id != project_id && mr.target_project_id != project_id {
            return Err(AuthError::Internal("merge request not found".to_owned()));
        }
        Ok(mr)
    }

    pub async fn create(&self, project_id: i64, create: DbCreatePullRequest) -> Result<DbPullRequest, AuthError> {
        let _ = project_id;
        self.mrs.create(&self.db_pool, create).await.map_err(db_err)
    }

    pub async fn update(&self, project_id: i64, id: i64, update: DbUpdatePullRequest) -> Result<DbPullRequest, AuthError> {
        self.get(project_id, id).await?;
        self.mrs.update(&self.db_pool, id, update).await.map_err(db_err)
    }

    pub async fn delete(&self, project_id: i64, id: i64) -> Result<(), AuthError> {
        self.get(project_id, id).await?;
        self.mrs.delete(&self.db_pool, id).await.map_err(db_err)
    }

    pub fn strategy_db(strategy: crate::handlers::mr_handlers::MergeStrategy) -> DbMergeStrategy {
        match strategy {
            crate::handlers::mr_handlers::MergeStrategy::MergeCommit => DbMergeStrategy::MergeCommit,
            crate::handlers::mr_handlers::MergeStrategy::Squash => DbMergeStrategy::Squash,
            crate::handlers::mr_handlers::MergeStrategy::FastForward => DbMergeStrategy::FastForward,
            crate::handlers::mr_handlers::MergeStrategy::RebaseMerge => DbMergeStrategy::Rebase,
        }
    }

    pub fn strategy_api(strategy: DbMergeStrategy) -> crate::handlers::mr_handlers::MergeStrategy {
        match strategy {
            DbMergeStrategy::MergeCommit => crate::handlers::mr_handlers::MergeStrategy::MergeCommit,
            DbMergeStrategy::Squash => crate::handlers::mr_handlers::MergeStrategy::Squash,
            DbMergeStrategy::FastForward => crate::handlers::mr_handlers::MergeStrategy::FastForward,
            DbMergeStrategy::Rebase => crate::handlers::mr_handlers::MergeStrategy::RebaseMerge,
        }
    }

    pub fn state_api(state: onedev_db::models::pull_request::DbPullRequestState) -> crate::handlers::mr_handlers::MergeRequestState {
        use crate::handlers::mr_handlers::MergeRequestState;
        match state {
            onedev_db::models::pull_request::DbPullRequestState::Open => MergeRequestState::Open,
            onedev_db::models::pull_request::DbPullRequestState::Merged => MergeRequestState::Merged,
            _ => MergeRequestState::Closed,
        }
    }

    pub fn state_db(state: crate::handlers::mr_handlers::MergeRequestState) -> onedev_db::models::pull_request::DbPullRequestState {
        use onedev_db::models::pull_request::DbPullRequestState;
        match state {
            crate::handlers::mr_handlers::MergeRequestState::Open => DbPullRequestState::Open,
            crate::handlers::mr_handlers::MergeRequestState::Merged => DbPullRequestState::Merged,
            _ => DbPullRequestState::Closed,
        }
    }
}
