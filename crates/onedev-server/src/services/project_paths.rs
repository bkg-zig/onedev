//! Project path resolution: a project's repository lives at
//! `<repos_root>/<ancestor paths>/<project path>.git`.

use std::path::{Path, PathBuf};

use onedev_db::DbPool;
use sqlx::Row;

/// A project together with its full (slash separated) path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectRef {
    pub id: i64,
    pub full_path: String,
}

/// Full path of one project (ancestors first), `None` when it does not exist.
pub async fn project_full_path(pool: &DbPool, project_id: i64) -> Result<Option<String>, sqlx::Error> {
    let row = sqlx::query(
        r#"
        WITH RECURSIVE chain AS (
            SELECT id, parent_id, path, 1 AS depth FROM projects WHERE id = $1
            UNION ALL
            SELECT p.id, p.parent_id, p.path, c.depth + 1
            FROM projects p JOIN chain c ON p.id = c.parent_id
            WHERE c.depth < 32
        )
        SELECT string_agg(path::text, '/' ORDER BY depth DESC) AS full_path FROM chain
        "#,
    )
    .bind(project_id)
    .fetch_one(pool)
    .await?;
    row.try_get::<Option<String>, _>("full_path")
}

/// Active (non-deleted) projects ordered by the given maintenance timestamp
/// column are listed by the callers; this resolves paths for a set of ids.
pub async fn project_full_paths(pool: &DbPool, ids: &[i64]) -> Result<Vec<ProjectRef>, sqlx::Error> {
    if ids.is_empty() {
        return Ok(Vec::new());
    }
    let rows = sqlx::query(
        r#"
        WITH RECURSIVE chain AS (
            SELECT id AS leaf, id, parent_id, path, 1 AS depth FROM projects WHERE id = ANY($1)
            UNION ALL
            SELECT c.leaf, p.id, p.parent_id, p.path, c.depth + 1
            FROM projects p JOIN chain c ON p.id = c.parent_id
            WHERE c.depth < 32
        )
        SELECT leaf, string_agg(path::text, '/' ORDER BY depth DESC) AS full_path
        FROM chain GROUP BY leaf ORDER BY leaf
        "#,
    )
    .bind(ids)
    .fetch_all(pool)
    .await?;
    rows.into_iter()
        .map(|row| {
            Ok(ProjectRef {
                id: row.try_get::<i64, _>("leaf")?,
                full_path: row.try_get::<String, _>("full_path")?,
            })
        })
        .collect()
}

/// Repository directory of a project path below `repos_root`, rejecting
/// anything that could leave the root.
pub fn repository_dir(repos_root: &Path, full_path: &str) -> Option<PathBuf> {
    onedev_git::utils::repository_path(repos_root, full_path).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repository_dir_rejects_traversal() {
        let root = Path::new("/srv/repos");
        assert_eq!(
            repository_dir(root, "group/app"),
            Some(PathBuf::from("/srv/repos/group/app.git"))
        );
        assert!(repository_dir(root, "../etc").is_none());
        assert!(repository_dir(root, "a/../../b").is_none());
        assert!(repository_dir(root, "").is_none());
        assert!(repository_dir(root, "/abs").is_none());
    }
}
