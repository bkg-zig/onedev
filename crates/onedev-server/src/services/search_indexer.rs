//! Search Indexer Service

use anyhow::Result;
use chrono::{DateTime, Utc};
use sqlx::{PgPool, Row};
use std::sync::Arc;
use tokio::sync::broadcast;
use tokio::task;
use tracing::{info, warn, error, debug};
use uuid::Uuid;

/// Search indexer configuration
#[derive(Debug, Clone)]
pub struct SearchIndexerConfig {
    pub batch_size: usize,
    pub index_interval_secs: u64,
    pub full_reindex_interval_hours: u64,
    pub enable_code_search: bool,
    pub enable_issue_search: bool,
    pub enable_wiki_search: bool,
}

impl Default for SearchIndexerConfig {
    fn default() -> Self {
        Self {
            batch_size: 100,
            index_interval_secs: 60,
            full_reindex_interval_hours: 24,
            enable_code_search: true,
            enable_issue_search: true,
            enable_wiki_search: true,
        }
    }
}

/// Search indexer service
pub struct SearchIndexer {
    db_pool: Arc<PgPool>,
    config: SearchIndexerConfig,
    shutdown_rx: broadcast::Receiver<()>,
    last_full_reindex: Arc<tokio::sync::RwLock<Option<DateTime<Utc>>>>,
}

impl SearchIndexer {
    /// Create a new search indexer
    pub fn new(
        db_pool: Arc<PgPool>,
        config: SearchIndexerConfig,
        shutdown_rx: broadcast::Receiver<()>,
    ) -> Self {
        Self {
            db_pool,
            config,
            shutdown_rx,
            last_full_reindex: Arc::new(tokio::sync::RwLock::new(None)),
        }
    }

    /// Run the indexer
    pub async fn run(mut self) {
        info!("Search indexer started");

        let mut interval = tokio::time::interval(
            tokio::time::Duration::from_secs(self.config.index_interval_secs)
        );
        let mut reindex_interval = tokio::time::interval(
            tokio::time::Duration::from_secs(self.config.full_reindex_interval_hours * 3600)
        );

        // Run incremental indexing at startup
        if let Err(e) = self.incremental_index().await {
            error!("Initial incremental indexing failed: {}", e);
        }

        loop {
            tokio::select! {
                _ = interval.tick() => {
                    if let Err(e) = self.incremental_index().await {
                        error!("Incremental indexing failed: {}", e);
                    }
                }
                _ = reindex_interval.tick() => {
                    if let Err(e) = self.full_reindex().await {
                        error!("Full reindex failed: {}", e);
                    }
                }
                _ = self.shutdown_rx.recv() => {
                    info!("Search indexer received shutdown signal");
                    break;
                }
            }
        }

        info!("Search indexer stopped");
    }

    /// Incremental indexing - only changed entities
    async fn incremental_index(&self) -> Result<()> {
        debug!("Running incremental indexing...");

        let mut total_indexed = 0;

        // Index updated issues
        if self.config.enable_issue_search {
            total_indexed += self.index_updated_issues().await?;
        }

        // Index updated projects
        total_indexed += self.index_updated_projects().await?;

        // Index updated wiki pages
        if self.config.enable_wiki_search {
            total_indexed += self.index_updated_wiki().await?;
        }

        // Index new commits (code search)
        if self.config.enable_code_search {
            total_indexed += self.index_new_commits().await?;
        }

        if total_indexed > 0 {
            info!("Incremental indexing completed: {} entities indexed", total_indexed);
        }

        Ok(())
    }

    /// Full reindex - all entities
    async fn full_reindex(&self) -> Result<()> {
        info!("Starting full reindex...");

        *self.last_full_reindex.write().await = Some(Utc::now());

        let mut total_indexed = 0;

        // Reindex all issues
        if self.config.enable_issue_search {
            total_indexed += self.reindex_all_issues().await?;
        }

        // Reindex all projects
        total_indexed += self.reindex_all_projects().await?;

        // Reindex all wiki pages
        if self.config.enable_wiki_search {
            total_indexed += self.reindex_all_wiki().await?;
        }

        // Reindex all code
        if self.config.enable_code_search {
            total_indexed += self.reindex_all_code().await?;
        }

        info!("Full reindex completed: {} entities indexed", total_indexed);
        Ok(())
    }

    /// Index updated issues
    async fn index_updated_issues(&self) -> Result<usize> {
        let batch_size = self.config.batch_size as i64;
        let indexed = sqlx::query(
            r#"
            WITH updated AS (
                SELECT id, project_id, title, description, state, priority, assignee_id, author_id, created_at, updated_at
                FROM issues
                WHERE deleted_at IS NULL
                AND (search_indexed_at IS NULL OR search_indexed_at < updated_at)
                LIMIT $1
            )
            INSERT INTO search_index (entity_type, entity_id, project_id, title, content, metadata, indexed_at)
            SELECT 'issue', id, project_id, title,
                   COALESCE(description, ''),
                   jsonb_build_object(
                       'state', state,
                       'priority', priority,
                       'assignee_id', assignee_id,
                       'author_id', author_id
                   ),
                   NOW()
            FROM updated
            ON CONFLICT (entity_type, entity_id) DO UPDATE SET
                title = EXCLUDED.title,
                content = EXCLUDED.content,
                metadata = EXCLUDED.metadata,
                indexed_at = EXCLUDED.indexed_at
            RETURNING entity_id
            "#
        )
            .bind(batch_size)
            .fetch_all(self.db_pool.as_ref()).await?;

        // Update search_indexed_at
        if !indexed.is_empty() {
            let ids: Vec<Uuid> = indexed.iter().filter_map(|r| r.try_get("entity_id").ok()).collect();
            sqlx::query("UPDATE issues SET search_indexed_at = $1 WHERE id = ANY($2)")
                .bind(Utc::now())
                .bind(&ids)
                .execute(self.db_pool.as_ref()).await?;
        }

        Ok(indexed.len())
    }

    /// Index updated projects
    async fn index_updated_projects(&self) -> Result<usize> {
        let batch_size = self.config.batch_size as i64;
        let indexed = sqlx::query(
            r#"
            WITH updated AS (
                SELECT id, name, description, visibility, owner_id, created_at, updated_at
                FROM projects
                WHERE deleted_at IS NULL
                AND (search_indexed_at IS NULL OR search_indexed_at < updated_at)
                LIMIT $1
            )
            INSERT INTO search_index (entity_type, entity_id, project_id, title, content, metadata, indexed_at)
            SELECT 'project', id, id, name,
                   COALESCE(description, ''),
                   jsonb_build_object(
                       'visibility', visibility,
                       'owner_id', owner_id
                   ),
                   NOW()
            FROM updated
            ON CONFLICT (entity_type, entity_id) DO UPDATE SET
                title = EXCLUDED.title,
                content = EXCLUDED.content,
                metadata = EXCLUDED.metadata,
                indexed_at = EXCLUDED.indexed_at
            RETURNING entity_id
            "#
        )
            .bind(batch_size)
            .fetch_all(self.db_pool.as_ref()).await?;

        if !indexed.is_empty() {
            let ids: Vec<Uuid> = indexed.iter().filter_map(|r| r.try_get("entity_id").ok()).collect();
            sqlx::query("UPDATE projects SET search_indexed_at = $1 WHERE id = ANY($2)")
                .bind(Utc::now())
                .bind(&ids)
                .execute(self.db_pool.as_ref()).await?;
        }

        Ok(indexed.len())
    }

    /// Index updated wiki pages
    async fn index_updated_wiki(&self) -> Result<usize> {
        let batch_size = self.config.batch_size as i64;
        let indexed = sqlx::query(
            r#"
            WITH updated AS (
                SELECT wp.id, wp.project_id, wp.title, wp.content, wp.author_id, wp.created_at, wp.updated_at
                FROM wiki_pages wp
                WHERE wp.deleted_at IS NULL
                AND (wp.search_indexed_at IS NULL OR wp.search_indexed_at < wp.updated_at)
                LIMIT $1
            )
            INSERT INTO search_index (entity_type, entity_id, project_id, title, content, metadata, indexed_at)
            SELECT 'wiki', id, project_id, title, content,
                   jsonb_build_object('author_id', author_id),
                   NOW()
            FROM updated
            ON CONFLICT (entity_type, entity_id) DO UPDATE SET
                title = EXCLUDED.title,
                content = EXCLUDED.content,
                metadata = EXCLUDED.metadata,
                indexed_at = EXCLUDED.indexed_at
            RETURNING entity_id
            "#
        )
            .bind(batch_size)
            .fetch_all(self.db_pool.as_ref()).await?;

        if !indexed.is_empty() {
            let ids: Vec<Uuid> = indexed.iter().filter_map(|r| r.try_get("entity_id").ok()).collect();
            sqlx::query("UPDATE wiki_pages SET search_indexed_at = $1 WHERE id = ANY($2)")
                .bind(Utc::now())
                .bind(&ids)
                .execute(self.db_pool.as_ref()).await?;
        }

        Ok(indexed.len())
    }

    /// Index new commits
    async fn index_new_commits(&self) -> Result<usize> {
        // This would index commit messages, file paths, code content
        // For now, just return 0
        Ok(0)
    }

    /// Reindex all issues
    async fn reindex_all_issues(&self) -> Result<usize> {
        let mut total = 0;
        let mut offset = 0;
        let batch_size = self.config.batch_size as i64;

        loop {
            let indexed = sqlx::query(
                r#"
                INSERT INTO search_index (entity_type, entity_id, project_id, title, content, metadata, indexed_at)
                SELECT 'issue', id, project_id, title,
                       COALESCE(description, ''),
                       jsonb_build_object(
                           'state', state,
                           'priority', priority,
                           'assignee_id', assignee_id,
                           'author_id', author_id
                       ),
                       NOW()
                FROM issues
                WHERE deleted_at IS NULL
                ORDER BY id
                LIMIT $1 OFFSET $2
                ON CONFLICT (entity_type, entity_id) DO UPDATE SET
                    title = EXCLUDED.title,
                    content = EXCLUDED.content,
                    metadata = EXCLUDED.metadata,
                    indexed_at = EXCLUDED.indexed_at
                RETURNING entity_id
                "#
            )
                .bind(batch_size)
                .bind(offset as i64)
                .fetch_all(self.db_pool.as_ref()).await?;

            let count = indexed.len();
            total += count;

            if count < self.config.batch_size {
                break;
            }
            offset += count;
        }

        sqlx::query("UPDATE issues SET search_indexed_at = $1 WHERE deleted_at IS NULL")
            .bind(Utc::now())
            .execute(self.db_pool.as_ref()).await?;

        Ok(total)
    }

    /// Reindex all projects
    async fn reindex_all_projects(&self) -> Result<usize> {
        let mut total = 0;
        let mut offset = 0;
        let batch_size = self.config.batch_size as i64;

        loop {
            let indexed = sqlx::query(
                r#"
                INSERT INTO search_index (entity_type, entity_id, project_id, title, content, metadata, indexed_at)
                SELECT 'project', id, id, name,
                       COALESCE(description, ''),
                       jsonb_build_object('visibility', visibility, 'owner_id', owner_id),
                       NOW()
                FROM projects
                WHERE deleted_at IS NULL
                ORDER BY id
                LIMIT $1 OFFSET $2
                ON CONFLICT (entity_type, entity_id) DO UPDATE SET
                    title = EXCLUDED.title,
                    content = EXCLUDED.content,
                    metadata = EXCLUDED.metadata,
                    indexed_at = EXCLUDED.indexed_at
                RETURNING entity_id
                "#
            )
                .bind(batch_size)
                .bind(offset as i64)
                .fetch_all(self.db_pool.as_ref()).await?;

            let count = indexed.len();
            total += count;

            if count < self.config.batch_size {
                break;
            }
            offset += count;
        }

        sqlx::query("UPDATE projects SET search_indexed_at = $1 WHERE deleted_at IS NULL")
            .bind(Utc::now())
            .execute(self.db_pool.as_ref()).await?;

        Ok(total)
    }

    /// Reindex all wiki pages
    async fn reindex_all_wiki(&self) -> Result<usize> {
        let mut total = 0;
        let mut offset = 0;
        let batch_size = self.config.batch_size as i64;

        loop {
            let indexed = sqlx::query(
                r#"
                INSERT INTO search_index (entity_type, entity_id, project_id, title, content, metadata, indexed_at)
                SELECT 'wiki', id, project_id, title, content,
                       jsonb_build_object('author_id', author_id),
                       NOW()
                FROM wiki_pages
                WHERE deleted_at IS NULL
                ORDER BY id
                LIMIT $1 OFFSET $2
                ON CONFLICT (entity_type, entity_id) DO UPDATE SET
                    title = EXCLUDED.title,
                    content = EXCLUDED.content,
                    metadata = EXCLUDED.metadata,
                    indexed_at = EXCLUDED.indexed_at
                RETURNING entity_id
                "#
            )
                .bind(batch_size)
                .bind(offset as i64)
                .fetch_all(self.db_pool.as_ref()).await?;

            let count = indexed.len();
            total += count;

            if count < self.config.batch_size {
                break;
            }
            offset += count;
        }

        sqlx::query("UPDATE wiki_pages SET search_indexed_at = $1 WHERE deleted_at IS NULL")
            .bind(Utc::now())
            .execute(self.db_pool.as_ref()).await?;

        Ok(total)
    }

    /// Reindex all code (commits)
    async fn reindex_all_code(&self) -> Result<usize> {
        // Would reindex all commit messages, file paths, code snippets
        Ok(0)
    }

    /// Trigger manual reindex
    pub async fn trigger_reindex(&self) -> Result<()> {
        self.full_reindex().await
    }

    /// Get indexing stats
    pub async fn get_stats(&self) -> Result<SearchIndexStats> {
        let stats = sqlx::query(
            r#"
            SELECT
                entity_type,
                COUNT(*) as count,
                MAX(indexed_at) as last_indexed
            FROM search_index
            GROUP BY entity_type
            "#
        ).fetch_all(self.db_pool.as_ref()).await?;

        let mut by_type = std::collections::HashMap::new();
        for row in stats {
            let entity_type: String = row.try_get("entity_type")?;
            let count: i64 = row.try_get("count").unwrap_or(0);
            let last_indexed: Option<DateTime<Utc>> = row.try_get("last_indexed").ok();
            by_type.insert(entity_type, (count as usize, last_indexed));
        }

        let last_full = *self.last_full_reindex.read().await;

        Ok(SearchIndexStats {
            by_type,
            last_full_reindex: last_full,
        })
    }
}

/// Search index statistics
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SearchIndexStats {
    pub by_type: std::collections::HashMap<String, (usize, Option<DateTime<Utc>>)>,
    pub last_full_reindex: Option<DateTime<Utc>>,
}