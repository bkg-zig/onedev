//! Pagination models

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// Cursor-based pagination request
#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub struct CursorPagination {
    /// Cursor for next page (base64 encoded)
    pub after: Option<String>,
    /// Cursor for previous page (base64 encoded)
    pub before: Option<String>,
    /// Maximum items per page (default: 20, max: 100)
    #[serde(default = "default_limit")]
    #[schema(default = 20, minimum = 1, maximum = 100)]
    pub limit: u32,
}

/// Offset-based pagination request
#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub struct OffsetPagination {
    /// Page number (0-indexed)
    #[serde(default)]
    #[schema(default = 0, minimum = 0)]
    pub page: u32,
    /// Items per page (default: 20, max: 100)
    #[serde(default = "default_limit")]
    #[schema(default = 20, minimum = 1, maximum = 100)]
    pub per_page: u32,
}

/// Pagination response metadata
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct PaginationMeta {
    /// Total count of items
    pub total: u64,
    /// Current page (for offset pagination)
    pub page: Option<u32>,
    /// Items per page
    pub per_page: u32,
    /// Total pages (for offset pagination)
    pub total_pages: Option<u32>,
    /// Next cursor (for cursor pagination)
    pub next_cursor: Option<String>,
    /// Previous cursor (for cursor pagination)
    pub prev_cursor: Option<String>,
    /// Whether there are more items
    pub has_more: bool,
}

/// Paginated response wrapper
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct PaginatedResponse<T> {
    pub items: Vec<T>,
    pub pagination: PaginationMeta,
}

impl<T> PaginatedResponse<T> {
    pub fn new(items: Vec<T>, total: u64, pagination: impl Into<PaginationMeta>) -> Self {
        let mut meta = pagination.into();
        meta.total = total;
        if let (Some(page), per_page) = (meta.page, meta.per_page) {
            if per_page > 0 {
                let total_pages = ((total + per_page as u64 - 1) / per_page as u64) as u32;
                meta.total_pages = Some(total_pages);
                meta.has_more = page + 1 < total_pages;
            }
        }
        Self {
            items,
            pagination: meta,
        }
    }
}

/// Sort order for list queries
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema, Default)]
#[serde(rename_all = "lowercase")]
pub enum SortOrder {
    #[default]
    Asc,
    Desc,
}

impl Default for OffsetPagination {
    fn default() -> Self {
        Self {
            page: 0,
            per_page: default_limit(),
        }
    }
}

fn default_limit() -> u32 {
    20
}

impl From<OffsetPagination> for PaginationMeta {
    fn from(p: OffsetPagination) -> Self {
        // Total is unknown at this point; `PaginatedResponse::new` fills it in.
        Self {
            total: 0,
            page: Some(p.page),
            per_page: p.per_page,
            total_pages: None,
            next_cursor: None,
            prev_cursor: None,
            has_more: false,
        }
    }
}