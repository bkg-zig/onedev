use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Pack {
    pub id: i64,
    pub project_id: i64,
    pub package_type: String,
    pub name: String,
    pub version: String,
    pub status: String,
    pub upload_date: DateTime<Utc>,
    #[serde(default)]
    pub size: i64,
}
