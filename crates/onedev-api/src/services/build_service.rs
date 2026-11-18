//! Build Service — real DB-backed build tracking.
//!
//! Triggering a build validates the request and queues a `Pending` build
//! row; execution itself is the server's build scheduler, which picks up
//! queued rows (vanilla splits scheduling and execution the same way).

use crate::errors::AuthError;
use onedev_db::models::build::{
    DbBuild, DbBuildStatus, DbBuildTrigger, DbCreateBuild,
};
use onedev_db::repositories::{BuildRepository, Repository};
use onedev_db::DbPool;

pub struct BuildService {
    db_pool: DbPool,
    builds: BuildRepository,
}

fn db_err(err: onedev_db::DbError) -> AuthError {
    AuthError::Internal(err.to_string())
}

pub fn status_api(status: DbBuildStatus) -> crate::handlers::build_handlers::BuildStatus {
    use crate::handlers::build_handlers::BuildStatus;
    match status {
        DbBuildStatus::Pending => BuildStatus::Pending,
        DbBuildStatus::Running => BuildStatus::Running,
        DbBuildStatus::Success => BuildStatus::Success,
        DbBuildStatus::Failed => BuildStatus::Failed,
        DbBuildStatus::Cancelled => BuildStatus::Canceled,
        DbBuildStatus::Skipped => BuildStatus::Skipped,
    }
}

impl BuildService {
    pub fn new(db_pool: DbPool) -> Self {
        Self {
            db_pool,
            builds: BuildRepository::new(),
        }
    }

    pub async fn list(&self, project_id: i64, limit: i64, offset: i64) -> Result<Vec<DbBuild>, AuthError> {
        self.builds
            .find_by_project_id(&self.db_pool, project_id, limit, offset)
            .await
            .map_err(db_err)
    }

    pub async fn count(&self, project_id: i64) -> Result<i64, AuthError> {
        let all = self
            .builds
            .find_by_project_id(&self.db_pool, project_id, i64::MAX, 0)
            .await
            .map_err(db_err)?;
        Ok(all.len() as i64)
    }

    pub async fn get(&self, project_id: i64, id: i64) -> Result<DbBuild, AuthError> {
        let build = self
            .builds
            .find_by_id(&self.db_pool, id)
            .await
            .map_err(db_err)?
            .ok_or_else(|| AuthError::Internal("build not found".to_owned()))?;
        if build.project_id != project_id {
            return Err(AuthError::Internal("build not found".to_owned()));
        }
        Ok(build)
    }

    /// Queue a manual build: validates the ref name and stores a `Pending`
    /// row with the next build number for the project.
    pub async fn trigger(
        &self,
        project_id: i64,
        author_id: i64,
        ref_name: String,
        variables: std::collections::HashMap<String, String>,
    ) -> Result<DbBuild, AuthError> {
        if ref_name.trim().is_empty() || ref_name.contains("..") || ref_name.contains(' ') {
            return Err(AuthError::Internal("invalid ref name".to_owned()));
        }
        let number = self.builds.next_number(&self.db_pool, project_id).await.map_err(db_err)?;
        self.builds
            .create(
                &self.db_pool,
                DbCreateBuild {
                    project_id,
                    number,
                    ref_name,
                    commit_sha: String::new(),
                    commit_message: None,
                    author_id,
                    trigger: DbBuildTrigger::Manual,
                    trigger_data: None,
                    spec: serde_json::json!({}),
                    parameters: Some(
                        serde_json::to_value(&variables).map_err(|e| AuthError::Internal(e.to_string()))?,
                    ),
                },
            )
            .await
            .map_err(db_err)
    }
}
