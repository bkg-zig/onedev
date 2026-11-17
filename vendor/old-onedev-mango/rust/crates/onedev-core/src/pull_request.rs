use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::{CoreError, CoreResult};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PullRequest {
    pub id: i64,
    pub project_id: i64,
    pub number: i64,
    pub title: String,
    #[serde(default)]
    pub description: String,
    pub source_branch: String,
    pub target_branch: String,
    pub state: String,
    pub author_id: i64,
    pub create_date: DateTime<Utc>,
    #[serde(default)]
    pub merged_date: Option<DateTime<Utc>>,
    #[serde(default)]
    pub closed_date: Option<DateTime<Utc>>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PullRequestCreate {
    pub project_id: i64,
    pub title: String,
    #[serde(default)]
    pub description: String,
    pub source_branch: String,
    pub target_branch: String,
}

impl PullRequestCreate {
    pub fn validate(&self) -> CoreResult<()> {
        if self.title.trim().is_empty() {
            return Err(CoreError::validation("pull request title is required"));
        }
        if self.source_branch.trim().is_empty() || self.target_branch.trim().is_empty() {
            return Err(CoreError::validation(
                "source and target branches are required",
            ));
        }
        if self.source_branch == self.target_branch {
            return Err(CoreError::validation(
                "source and target branches must differ",
            ));
        }
        Ok(())
    }
}
