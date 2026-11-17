//! Label API models

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// Label response
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct LabelResponse {
    pub id: i64,
    pub name: String,
    pub color: String,
    pub description: Option<String>,
}
