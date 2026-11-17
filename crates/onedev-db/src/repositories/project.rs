//! Project Repository

use crate::{
    models::project::{DbProject, DbProjectStats, DbCreateProject, DbUpdateProject, ProjectQueryBuilder},
    repositories::Repository,
    DbPool, DbResult,
};
use async_trait::async_trait;
use sqlx::{FromRow, Row};

pub struct ProjectRepository;

impl ProjectRepository {
    pub fn new() -> Self {
        Self
    }

    /// Find project by path (and optional parent)
    pub async fn find_by_path(&self, pool: &DbPool, path: &str, parent_id: Option<i64>) -> DbResult<Option<DbProject>> {
        let row = sqlx::query(ProjectQueryBuilder::select_by_path())
            .bind(path)
            .bind(parent_id)
            .fetch_optional(pool)
            .await?;

        Ok(row.map(|r| DbProject::from_row(&r)).transpose()?)
    }

    /// Find project by UUID
    pub async fn find_by_uuid(&self, pool: &DbPool, uuid: uuid::Uuid) -> DbResult<Option<DbProject>> {
        let row = sqlx::query(ProjectQueryBuilder::select_by_uuid())
            .bind(uuid)
            .fetch_optional(pool)
            .await?;

        Ok(row.map(|r| DbProject::from_row(&r)).transpose()?)
    }

    /// List projects with filters
    pub async fn list_filtered(
        &self,
        pool: &DbPool,
        term: Option<&str>,
        visibility: Option<crate::models::project::DbProjectVisibility>,
        status: Option<crate::models::project::DbProjectStatus>,
        owner_id: Option<i64>,
        page: u32,
        per_page: u32,
    ) -> DbResult<Vec<DbProject>> {
        let mut query = String::from(ProjectQueryBuilder::select_all());
        query.push_str(" WHERE deleted_at IS NULL");

        let mut conditions = Vec::new();
        let mut param_idx = 1;

        if let Some(term) = term {
            conditions.push(format!("(name ILIKE ${} OR path ILIKE ${} OR description ILIKE ${})", param_idx, param_idx, param_idx));
            param_idx += 1;
        }

        if visibility.is_some() {
            conditions.push(format!("visibility = ${}", param_idx));
            param_idx += 1;
        }

        if status.is_some() {
            conditions.push(format!("status = ${}", param_idx));
            param_idx += 1;
        }

        if let Some(owner_id) = owner_id {
            conditions.push(format!("owner_id = ${}", param_idx));
            param_idx += 1;
        }

        if !conditions.is_empty() {
            query.push_str(" AND ");
            query.push_str(&conditions.join(" AND "));
        }

        query.push_str(" ORDER BY created_at DESC LIMIT $");
        query.push_str(&param_idx.to_string());
        param_idx += 1;
        query.push_str(" OFFSET $");
        query.push_str(&param_idx.to_string());

        let mut q = sqlx::query(&query);

        if let Some(term) = term {
            let pattern = format!("%{}%", term);
            q = q.bind(pattern);
        }

        if let Some(v) = visibility {
            q = q.bind(v);
        }

        if let Some(s) = status {
            q = q.bind(s);
        }

        if let Some(oid) = owner_id {
            q = q.bind(oid);
        }

        q = q.bind(per_page as i64);
        q = q.bind((page * per_page) as i64);

        let rows = q.fetch_all(pool).await?;
        let projects = rows.into_iter().map(|r| DbProject::from_row(&r)).collect::<Result<Vec<_>, _>>()?;
        Ok(projects)
    }

    /// Count projects with filters
    pub async fn count_filtered(
        &self,
        pool: &DbPool,
        term: Option<&str>,
        visibility: Option<crate::models::project::DbProjectVisibility>,
        status: Option<crate::models::project::DbProjectStatus>,
        owner_id: Option<i64>,
    ) -> DbResult<i64> {
        let mut query = String::from("SELECT COUNT(*) FROM projects WHERE deleted_at IS NULL");
        let mut conditions = Vec::new();
        let mut param_idx = 1;

        if let Some(term) = term {
            conditions.push(format!("(name ILIKE ${} OR path ILIKE ${} OR description ILIKE ${})", param_idx, param_idx, param_idx));
            param_idx += 1;
        }

        if visibility.is_some() {
            conditions.push(format!("visibility = ${}", param_idx));
            param_idx += 1;
        }

        if status.is_some() {
            conditions.push(format!("status = ${}", param_idx));
            param_idx += 1;
        }

        if let Some(owner_id) = owner_id {
            conditions.push(format!("owner_id = ${}", param_idx));
            param_idx += 1;
        }

        if !conditions.is_empty() {
            query.push_str(" AND ");
            query.push_str(&conditions.join(" AND "));
        }

        let mut q = sqlx::query_scalar(&query);

        if let Some(term) = term {
            let pattern = format!("%{}%", term);
            q = q.bind(pattern);
        }

        if let Some(v) = visibility {
            q = q.bind(v);
        }

        if let Some(s) = status {
            q = q.bind(s);
        }

        if let Some(oid) = owner_id {
            q = q.bind(oid);
        }

        let count: i64 = q.fetch_one(pool).await?;
        Ok(count)
    }

    /// Update last activity timestamp
    pub async fn update_last_activity(&self, pool: &DbPool, id: i64) -> DbResult<()> {
        sqlx::query("UPDATE projects SET last_activity_at = NOW(), updated_at = NOW() WHERE id = $1")
            .bind(id)
            .execute(pool)
            .await?;
        Ok(())
    }

    /// Get project with stats
    ///
    /// Storage/repository/wiki/package sizes and fork/star counters are not
    /// persisted in the schema (the git layer owns them), so they are reported as 0.
    pub async fn find_with_stats(&self, pool: &DbPool, id: i64) -> DbResult<Option<(DbProject, DbProjectStats)>> {
        let row = sqlx::query(
            r#"
            SELECT p.*,
                   COALESCE(i.open_count, 0) as open_issues_count,
                   COALESCE(i.total_count, 0) as issues_count,
                   COALESCE(mr.open_count, 0) as open_merge_requests_count,
                   COALESCE(mr.total_count, 0) as merge_requests_count,
                   COALESCE(b.count, 0) as builds_count
            FROM projects p
            LEFT JOIN (
                SELECT project_id, COUNT(*) as total_count, COUNT(*) FILTER (WHERE state = 'open') as open_count
                FROM issues GROUP BY project_id
            ) i ON p.id = i.project_id
            LEFT JOIN (
                SELECT project_id, COUNT(*) as total_count, COUNT(*) FILTER (WHERE state = 'open') as open_count
                FROM pull_requests GROUP BY project_id
            ) mr ON p.id = mr.project_id
            LEFT JOIN (
                SELECT project_id, COUNT(*) as count FROM builds GROUP BY project_id
            ) b ON p.id = b.project_id
            WHERE p.id = $1
            "#,
        )
        .bind(id)
        .fetch_optional(pool)
        .await?;

        if let Some(row) = row {
            let project = DbProject::from_row(&row)?;
            let stats = DbProjectStats {
                issues_count: row.try_get("issues_count")?,
                open_issues_count: row.try_get("open_issues_count")?,
                merge_requests_count: row.try_get("merge_requests_count")?,
                open_merge_requests_count: row.try_get("open_merge_requests_count")?,
                builds_count: row.try_get("builds_count")?,
                storage_size: 0,
                repository_size: 0,
                wiki_size: 0,
                packages_size: 0,
                forks_count: 0,
                stars_count: 0,
            };
            Ok(Some((project, stats)))
        } else {
            Ok(None)
        }
    }
}

#[async_trait]
impl Repository<DbProject, DbCreateProject, DbUpdateProject> for ProjectRepository {
    async fn create(&self, pool: &DbPool, create: DbCreateProject) -> DbResult<DbProject> {
        let uuid = uuid::Uuid::new_v4();
        let now = chrono::Utc::now();

        let id: i64 = sqlx::query_scalar(ProjectQueryBuilder::insert())
            .bind(uuid)
            .bind(&create.name)
            .bind(&create.path)
            .bind(&create.description)
            .bind(&create.readme)
            .bind(create.visibility)
            .bind(crate::models::project::DbProjectStatus::Active)
            .bind(create.owner_id)
            .bind(create.owner_type)
            .bind(create.parent_id)
            .bind(create.forked_from_id)
            .bind(&create.default_branch)
            .bind(&create.avatar_url)
            .bind(true) // issues_enabled
            .bind(true) // merge_requests_enabled
            .bind(true) // builds_enabled
            .bind(true) // wiki_enabled
            .bind(true) // snippets_enabled
            .bind(true) // packs_enabled
            .bind(false) // pages_enabled
            .bind(false) // container_registry_enabled
            .bind(false) // only_allow_merge_if_pipeline_succeeds
            .bind(false) // only_allow_merge_if_all_discussions_resolved
            .bind(true) // remove_source_branch_after_merge
            .bind(true) // enable_merge_commit
            .bind(true) // enable_squash_merge
            .bind(true) // enable_fast_forward_merge
            .bind(Option::<String>::None) // squash_merge_template
            .bind(60) // build_timeout
            .bind(Option::<String>::None) // build_coverage_regex
            .bind(now)
            .bind(now)
            .bind(Option::<serde_json::Value>::None) // issue_setting
            .bind(Option::<serde_json::Value>::None) // pull_request_setting
            .bind(Option::<serde_json::Value>::None) // build_setting
            .bind(Option::<serde_json::Value>::None) // pack_setting
            .bind(Option::<serde_json::Value>::None) // workspace_setting
            .bind(Option::<serde_json::Value>::None) // code_indexing_setting
            .bind(Option::<serde_json::Value>::None) // wiki_setting
            .bind(Option::<serde_json::Value>::None) // webhooks
            .bind(Option::<serde_json::Value>::None) // branch_protections
            .bind(Option::<serde_json::Value>::None) // tag_protections
            .fetch_one(pool)
            .await?;

        self.find_by_id(pool, id).await?.ok_or_else(|| crate::DbError::NotFound {
            entity_type: "Project".to_string(),
            id: id.to_string(),
        })
    }

    async fn find_by_id(&self, pool: &DbPool, id: i64) -> DbResult<Option<DbProject>> {
        let row = sqlx::query(ProjectQueryBuilder::select_by_id())
            .bind(id)
            .fetch_optional(pool)
            .await?;

        Ok(row.map(|r| DbProject::from_row(&r)).transpose()?)
    }

    async fn update(&self, pool: &DbPool, id: i64, update: DbUpdateProject) -> DbResult<DbProject> {
        let row = sqlx::query(ProjectQueryBuilder::update())
            .bind(id)
            .bind(&update.name)
            .bind(&update.description)
            .bind(&update.readme)
            .bind(update.visibility)
            .bind(update.status)
            .bind(&update.default_branch)
            .bind(&update.avatar_url)
            .bind(update.issues_enabled)
            .bind(update.merge_requests_enabled)
            .bind(update.builds_enabled)
            .bind(update.wiki_enabled)
            .bind(update.snippets_enabled)
            .bind(update.packs_enabled)
            .bind(update.pages_enabled)
            .bind(update.container_registry_enabled)
            .bind(update.only_allow_merge_if_pipeline_succeeds)
            .bind(update.only_allow_merge_if_all_discussions_resolved)
            .bind(update.remove_source_branch_after_merge)
            .bind(update.enable_merge_commit)
            .bind(update.enable_squash_merge)
            .bind(update.enable_fast_forward_merge)
            .bind(&update.squash_merge_template)
            .bind(update.build_timeout)
            .bind(&update.build_coverage_regex)
            .bind(&update.issue_setting)
            .bind(&update.pull_request_setting)
            .bind(&update.build_setting)
            .bind(&update.pack_setting)
            .bind(&update.workspace_setting)
            .bind(&update.code_indexing_setting)
            .bind(&update.wiki_setting)
            .bind(&update.webhooks)
            .bind(&update.branch_protections)
            .bind(&update.tag_protections)
            .fetch_one(pool)
            .await?;

        Ok(DbProject::from_row(&row)?)
    }

    async fn delete(&self, pool: &DbPool, id: i64) -> DbResult<()> {
        sqlx::query(ProjectQueryBuilder::delete())
            .bind(id)
            .execute(pool)
            .await?;
        Ok(())
    }

    async fn list(&self, pool: &DbPool, limit: i64, offset: i64) -> DbResult<Vec<DbProject>> {
        let rows = sqlx::query(ProjectQueryBuilder::list_with_pagination())
            .bind(limit)
            .bind(offset)
            .fetch_all(pool)
            .await?;

        let projects = rows.into_iter().map(|r| DbProject::from_row(&r)).collect::<Result<Vec<_>, _>>()?;
        Ok(projects)
    }

    async fn count(&self, pool: &DbPool) -> DbResult<i64> {
        let count: i64 = sqlx::query_scalar(ProjectQueryBuilder::count())
            .fetch_one(pool)
            .await?;
        Ok(count)
    }
}