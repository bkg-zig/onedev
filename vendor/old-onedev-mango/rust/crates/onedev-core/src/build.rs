use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum BuildStatus {
    QUEUED,
    RUNNING,
    SUCCEEDED,
    FAILED,
    CANCELLED,
    TIMED_OUT,
}

impl BuildStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::QUEUED => "QUEUED",
            Self::RUNNING => "RUNNING",
            Self::SUCCEEDED => "SUCCEEDED",
            Self::FAILED => "FAILED",
            Self::CANCELLED => "CANCELLED",
            Self::TIMED_OUT => "TIMED_OUT",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "QUEUED" => Some(Self::QUEUED),
            "RUNNING" => Some(Self::RUNNING),
            "SUCCEEDED" => Some(Self::SUCCEEDED),
            "FAILED" => Some(Self::FAILED),
            "CANCELLED" => Some(Self::CANCELLED),
            "TIMED_OUT" => Some(Self::TIMED_OUT),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Build {
    pub id: i64,
    pub project_id: i64,
    pub number: i64,
    pub ref_name: String,
    pub commit_hash: String,
    pub status: BuildStatus,
    pub submit_date: DateTime<Utc>,
    pub finish_date: Option<DateTime<Utc>>,
}
