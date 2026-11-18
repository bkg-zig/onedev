//! Issue Service — real DB-backed issue operations.

use crate::errors::AuthError;
use crate::services::user_service::user_ref;
use onedev_db::models::issue::{DbCreateIssue, DbIssue, DbIssueState, DbUpdateIssue};
use onedev_db::repositories::{IssueRepository, Repository, UserRepository};
use onedev_db::DbPool;

pub struct IssueService {
    db_pool: DbPool,
    issues: IssueRepository,
}

fn db_err(err: onedev_db::DbError) -> AuthError {
    AuthError::Internal(err.to_string())
}

impl IssueService {
    pub fn new(db_pool: DbPool) -> Self {
        Self {
            db_pool,
            issues: IssueRepository::new(),
        }
    }

    pub async fn list(&self, project_id: i64, limit: i64, offset: i64) -> Result<Vec<DbIssue>, AuthError> {
        let all = self.issues.list(&self.db_pool, i64::MAX, 0).await.map_err(db_err)?;
        Ok(all
            .into_iter()
            .filter(|issue| issue.project_id == project_id)
            .skip(offset as usize)
            .take(limit as usize)
            .collect())
    }

    pub async fn count(&self, project_id: i64) -> Result<i64, AuthError> {
        let all = self.issues.list(&self.db_pool, i64::MAX, 0).await.map_err(db_err)?;
        Ok(all.into_iter().filter(|issue| issue.project_id == project_id).count() as i64)
    }

    pub async fn get(&self, project_id: i64, id: i64) -> Result<DbIssue, AuthError> {
        let issue = self
            .issues
            .find_by_id(&self.db_pool, id)
            .await
            .map_err(db_err)?
            .ok_or_else(|| AuthError::Internal("issue not found".to_owned()))?;
        if issue.project_id != project_id {
            return Err(AuthError::Internal("issue not found".to_owned()));
        }
        Ok(issue)
    }

    pub async fn create(&self, project_id: i64, submitter_id: i64, title: String, description: Option<String>) -> Result<DbIssue, AuthError> {
        self.issues
            .create(
                &self.db_pool,
                DbCreateIssue {
                    project_id,
                    title,
                    description,
                    submitter_id,
                    assignee_id: None,
                    priority: 2,
                    milestone_id: None,
                    estimated_duration: None,
                    due_date: None,
                    label_ids: Vec::new(),
                },
            )
            .await
            .map_err(db_err)
    }

    pub async fn update(&self, project_id: i64, id: i64, update: DbUpdateIssue) -> Result<DbIssue, AuthError> {
        self.get(project_id, id).await?;
        self.issues.update(&self.db_pool, id, update).await.map_err(db_err)
    }

    pub async fn delete(&self, project_id: i64, id: i64) -> Result<(), AuthError> {
        self.get(project_id, id).await?;
        self.issues.delete(&self.db_pool, id).await.map_err(db_err)
    }

    pub async fn author_ref(&self, user_id: i64) -> crate::models::users::UserRef {
        UserRepository::new()
            .find_by_id(&self.db_pool, user_id)
            .await
            .ok()
            .flatten()
            .map(|user| user_ref(&user))
            .unwrap_or(crate::models::users::UserRef {
                id: user_id,
                uuid: uuid::Uuid::nil(),
                name: String::new(),
                full_name: None,
                avatar_url: None,
                user_type: crate::models::users::UserType::Ordinary,
            })
    }
}

pub fn issue_state_api(state: DbIssueState) -> crate::handlers::issue_handlers::IssueState {
    match state {
        DbIssueState::Open => crate::handlers::issue_handlers::IssueState::Open,
        DbIssueState::Closed => crate::handlers::issue_handlers::IssueState::Closed,
    }
}

pub fn issue_state_db(state: crate::handlers::issue_handlers::IssueState) -> DbIssueState {
    match state {
        crate::handlers::issue_handlers::IssueState::Open => DbIssueState::Open,
        _ => DbIssueState::Closed,
    }
}
