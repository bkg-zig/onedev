//! Milestone API models

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// Milestone response
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct MilestoneResponse {
    pub id: i64,
    pub title: String,
    pub description: Option<String>,
    pub due_date: Option<DateTime<Utc>>,
    pub closed: bool,
}
