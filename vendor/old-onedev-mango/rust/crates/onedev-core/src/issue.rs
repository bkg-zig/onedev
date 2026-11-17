use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::{CoreError, CoreResult};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Issue {
    pub id: i64,
    pub project_id: i64,
    pub number: i64,
    pub title: String,
    #[serde(default)]
    pub description: String,
    pub state: String,
    #[serde(default)]
    pub confidential: bool,
    pub submitter_id: i64,
    pub submit_date: DateTime<Utc>,
    pub closed_date: Option<DateTime<Utc>>,
    #[serde(default)]
    pub branch: Option<String>,
    #[serde(default)]
    pub fields: serde_json::Value,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IssueCreate {
    pub project_id: i64,
    pub title: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub confidential: bool,
    #[serde(default)]
    pub own_estimated_time: Option<i32>,
    #[serde(default)]
    pub iteration_ids: Vec<i64>,
    #[serde(default)]
    pub fields: serde_json::Value,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IssueUpdate {
    pub title: Option<String>,
    pub description: Option<String>,
    pub confidential: Option<bool>,
    pub state: Option<String>,
    pub fields: Option<serde_json::Value>,
}

impl IssueCreate {
    pub fn validate(&self) -> CoreResult<()> {
        let title = self.title.trim();
        if title.is_empty() || title.len() > 1000 {
            return Err(CoreError::validation(
                "issue title must contain between 1 and 1000 characters",
            ));
        }
        if self.description.len() > 1_000_000 {
            return Err(CoreError::validation("issue description is too long"));
        }
        Ok(())
    }
}
