//! Search & Query System

use crate::models::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Search query trait
pub trait SearchQuery: Send + Sync + Serialize + for<'de> Deserialize<'de> {
    type Result;
    fn validate(&self) -> Result<(), SearchError>;
    fn to_sql(&self) -> String;
}

/// Search criteria base
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SearchCriteria {
    pub query: Option<String>,
    pub filters: HashMap<String, FilterValue>,
    pub sort: Option<SortSpec>,
    pub pagination: Pagination,
    pub highlight: bool,
    pub facets: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum FilterValue {
    String(String),
    Number(f64),
    Boolean(bool),
    Array(Vec<String>),
    Range(RangeFilter),
    Exists(bool),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RangeFilter {
    pub gte: Option<f64>,
    pub lte: Option<f64>,
    pub gt: Option<f64>,
    pub lt: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SortSpec {
    pub field: String,
    pub order: SortOrder,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Pagination {
    pub page: u32,
    pub per_page: u32,
    pub cursor: Option<String>,
}

/// Search result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult<T> {
    pub items: Vec<T>,
    pub total: u64,
    pub page: u32,
    pub per_page: u32,
    pub total_pages: u32,
    pub facets: HashMap<String, FacetResult>,
    pub took_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FacetResult {
    pub field: String,
    pub buckets: Vec<FacetBucket>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FacetBucket {
    pub value: String,
    pub count: u64,
}

/// Search error
#[derive(Debug, thiserror::Error)]
pub enum SearchError {
    #[error("Invalid query: {0}")]
    InvalidQuery(String),
    #[error("Invalid filter: {0}")]
    InvalidFilter(String),
    #[error("Invalid sort: {0}")]
    InvalidSort(String),
    #[error("Search engine error: {0}")]
    EngineError(String),
    #[error("Index not found: {0}")]
    IndexNotFound(String),
}

/// Code search criteria
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CodeSearchCriteria {
    pub query: String,
    pub project_id: Option<i64>,
    pub repository_id: Option<i64>,
    pub branch: Option<String>,
    pub file_pattern: Option<String>,
    pub language: Option<String>,
    pub case_sensitive: bool,
    pub regex: bool,
    pub whole_word: bool,
    pub pagination: Pagination,
}

/// Code search result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeSearchResult {
    pub file_path: String,
    pub repository: String,
    pub branch: String,
    pub commit_sha: String,
    pub lines: Vec<CodeMatchLine>,
    pub context_before: Vec<String>,
    pub context_after: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeMatchLine {
    pub line_number: u32,
    pub content: String,
    pub matches: Vec<MatchSpan>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MatchSpan {
    pub start: usize,
    pub end: usize,
}

/// Entity search criteria (issues, MRs, builds, etc.)
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct EntitySearchCriteria {
    pub entity_type: EntityType,
    pub query: Option<String>,
    pub project_id: Option<i64>,
    pub group_id: Option<i64>,
    pub filters: HashMap<String, FilterValue>,
    pub sort: Option<SortSpec>,
    pub pagination: Pagination,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, strum::EnumString, strum::Display)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case")]
pub enum EntityType {
    #[default]
    Issue,
    MergeRequest,
    Build,
    Pipeline,
    Project,
    Group,
    User,
    Package,
    Release,
    WikiPage,
    Snippet,
    Commit,
    Tag,
    Branch,
}

/// Commit search criteria
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CommitSearchCriteria {
    pub query: Option<String>,
    pub project_id: Option<i64>,
    pub branch: Option<String>,
    pub author: Option<String>,
    pub author_email: Option<String>,
    pub since: Option<DateTime<Utc>>,
    pub until: Option<DateTime<Utc>>,
    pub path: Option<String>,
    pub pagination: Pagination,
}

/// Commit search result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommitSearchResult {
    pub id: String,
    pub short_id: String,
    pub title: String,
    pub message: Option<String>,
    pub author_name: String,
    pub author_email: String,
    pub author_date: DateTime<Utc>,
    pub committer_name: String,
    pub committer_email: String,
    pub committer_date: DateTime<Utc>,
    pub parent_ids: Vec<String>,
    pub stats: Option<CommitStats>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommitStats {
    pub files_changed: u32,
    pub additions: u32,
    pub deletions: u32,
}

/// Search index trait
pub trait SearchIndex: Send + Sync {
    fn index_name(&self) -> &str;
    fn index_document(&self, doc: serde_json::Value) -> Result<(), SearchError>;
    fn delete_document(&self, id: &str) -> Result<(), SearchError>;
    fn search(&self, criteria: &SearchCriteria) -> Result<SearchResult<serde_json::Value>, SearchError>;
    fn refresh(&self) -> Result<(), SearchError>;
}

/// Search service trait
#[async_trait::async_trait]
pub trait SearchService: Send + Sync {
    async fn search_code(&self, criteria: &CodeSearchCriteria) -> Result<SearchResult<CodeSearchResult>, SearchError>;
    async fn search_entities(&self, criteria: &EntitySearchCriteria) -> Result<SearchResult<serde_json::Value>, SearchError>;
    async fn search_commits(&self, criteria: &CommitSearchCriteria) -> Result<SearchResult<CommitSearchResult>, SearchError>;
    async fn index_project(&self, project_id: i64) -> Result<(), SearchError>;
    async fn delete_project_index(&self, project_id: i64) -> Result<(), SearchError>;
    async fn reindex_all(&self) -> Result<(), SearchError>;
}

/// Query builder for type-safe queries
pub struct QueryBuilder<T> {
    criteria: SearchCriteria,
    _phantom: std::marker::PhantomData<T>,
}

impl<T> QueryBuilder<T> {
    pub fn new() -> Self {
        Self {
            criteria: SearchCriteria::default(),
            _phantom: std::marker::PhantomData,
        }
    }

    pub fn query(mut self, q: impl Into<String>) -> Self {
        self.criteria.query = Some(q.into());
        self
    }

    pub fn filter(mut self, field: impl Into<String>, value: FilterValue) -> Self {
        self.criteria.filters.insert(field.into(), value);
        self
    }

    pub fn filter_exists(mut self, field: impl Into<String>, exists: bool) -> Self {
        self.criteria.filters.insert(field.into(), FilterValue::Exists(exists));
        self
    }

    pub fn filter_range(mut self, field: impl Into<String>, range: RangeFilter) -> Self {
        self.criteria.filters.insert(field.into(), FilterValue::Range(range));
        self
    }

    pub fn sort(mut self, field: impl Into<String>, order: SortOrder) -> Self {
        self.criteria.sort = Some(SortSpec { field: field.into(), order });
        self
    }

    pub fn paginate(mut self, page: u32, per_page: u32) -> Self {
        self.criteria.pagination = Pagination { page, per_page, cursor: None };
        self
    }

    pub fn cursor(mut self, cursor: impl Into<String>) -> Self {
        self.criteria.pagination.cursor = Some(cursor.into());
        self
    }

    pub fn highlight(mut self, enabled: bool) -> Self {
        self.criteria.highlight = enabled;
        self
    }

    pub fn facet(mut self, field: impl Into<String>) -> Self {
        self.criteria.facets.push(field.into());
        self
    }

    pub fn build(self) -> SearchCriteria {
        self.criteria
    }
}

impl<T> Default for QueryBuilder<T> {
    fn default() -> Self {
        Self::new()
    }
}