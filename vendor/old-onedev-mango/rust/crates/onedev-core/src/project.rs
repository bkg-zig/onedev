use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::{CoreError, CoreResult};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Project {
    pub id: i64,
    pub parent_id: Option<i64>,
    pub forked_from_id: Option<i64>,
    pub name: String,
    pub key: String,
    pub path: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub maintenance: bool,
    pub create_date: DateTime<Utc>,
    #[serde(default = "default_true")]
    pub code_management: bool,
    #[serde(default)]
    pub wiki_management: bool,
    #[serde(default = "default_true")]
    pub pack_management: bool,
    #[serde(default = "default_true")]
    pub issue_management: bool,
    #[serde(default)]
    pub time_tracking: bool,
    #[serde(default)]
    pub service_desk_email_address: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectCreate {
    pub parent_id: Option<i64>,
    pub forked_from_id: Option<i64>,
    pub name: String,
    #[serde(default)]
    pub key: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub maintenance: bool,
    #[serde(default = "default_true")]
    pub code_management: bool,
    #[serde(default)]
    pub wiki_management: bool,
    #[serde(default = "default_true")]
    pub pack_management: bool,
    #[serde(default = "default_true")]
    pub issue_management: bool,
    #[serde(default)]
    pub time_tracking: bool,
    #[serde(default)]
    pub service_desk_email_address: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectUpdate {
    pub parent_id: Option<i64>,
    pub forked_from_id: Option<i64>,
    pub name: Option<String>,
    pub key: Option<String>,
    pub description: Option<String>,
    pub maintenance: Option<bool>,
    pub code_management: Option<bool>,
    pub wiki_management: Option<bool>,
    pub pack_management: Option<bool>,
    pub issue_management: Option<bool>,
    pub time_tracking: Option<bool>,
    pub service_desk_email_address: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CloneUrl {
    pub http: String,
    pub ssh: String,
}

impl ProjectCreate {
    pub fn validate(&self) -> CoreResult<()> {
        let name = self.name.trim();
        if name.is_empty() || name.len() > 255 {
            return Err(CoreError::validation(
                "project name must contain between 1 and 255 characters",
            ));
        }
        if name.contains('/') {
            return Err(CoreError::validation("project name must not contain '/'"));
        }
        if self.key.len() > 12 {
            return Err(CoreError::validation(
                "project key must not be longer than 12 characters",
            ));
        }
        Ok(())
    }
}

fn default_true() -> bool {
    true
}
