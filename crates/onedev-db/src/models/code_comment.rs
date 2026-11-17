//! Code Comment Database Model

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, Row};
use uuid::Uuid;

/// Code comment row from database
#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct DbCodeComment {
    pub id: i64,
    pub uuid: Uuid,
    pub project_id: i64,
    pub pull_request_id: Option<i64>,
    pub commit_sha: String,
    pub file_path: String,
    pub old_path: Option<String>,
    pub line_number: Option<i32>,
    pub old_line_number: Option<i32>,
    pub author_id: i64,
    pub content: String,
    pub parent_id: Option<i64>,
    pub resolved: bool,
    pub resolved_by_id: Option<i64>,
    pub resolved_at: Option<DateTime<Utc>>,
    pub outdated: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Code comment creation input for database
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DbCreateCodeComment {
    pub project_id: i64,
    pub pull_request_id: Option<i64>,
    pub commit_sha: String,
    pub file_path: String,
    pub old_path: Option<String>,
    pub line_number: Option<i32>,
    pub old_line_number: Option<i32>,
    pub author_id: i64,
    pub content: String,
    pub parent_id: Option<i64>,
}

/// Code comment update input for database
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DbUpdateCodeComment {
    pub content: Option<String>,
    pub resolved: Option<bool>,
    pub resolved_by_id: Option<i64>,
    pub outdated: Option<bool>,
}

/// Database query builder for code comments
pub struct CodeCommentQueryBuilder;

impl CodeCommentQueryBuilder {
    pub fn select_all() -> &'static str {
        r#"
        SELECT id, uuid, project_id, pull_request_id, commit_sha, file_path, old_path,
               line_number, old_line_number, author_id, content, parent_id,
               resolved, resolved_by_id, resolved_at, outdated, created_at, updated_at
        FROM code_comments
        "#
    }

    pub fn select_by_id() -> &'static str {
        r#"
        SELECT id, uuid, project_id, pull_request_id, commit_sha, file_path, old_path,
               line_number, old_line_number, author_id, content, parent_id,
               resolved, resolved_by_id, resolved_at, outdated, created_at, updated_at
        FROM code_comments WHERE id = $1
        "#
    }

    pub fn select_by_pull_request() -> &'static str {
        r#"
        SELECT id, uuid, project_id, pull_request_id, commit_sha, file_path, old_path,
               line_number, old_line_number, author_id, content, parent_id,
               resolved, resolved_by_id, resolved_at, outdated, created_at, updated_at
        FROM code_comments WHERE pull_request_id = $1
        ORDER BY file_path, line_number, created_at
        "#
    }

    pub fn select_by_commit() -> &'static str {
        r#"
        SELECT id, uuid, project_id, pull_request_id, commit_sha, file_path, old_path,
               line_number, old_line_number, author_id, content, parent_id,
               resolved, resolved_by_id, resolved_at, outdated, created_at, updated_at
        FROM code_comments WHERE project_id = $1 AND commit_sha = $2
        ORDER BY file_path, line_number, created_at
        "#
    }

    pub fn insert() -> &'static str {
        r#"
        INSERT INTO code_comments (uuid, project_id, pull_request_id, commit_sha, file_path,
                                   old_path, line_number, old_line_number, author_id, content,
                                   parent_id, resolved, resolved_by_id, resolved_at, outdated,
                                   created_at, updated_at)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17)
        RETURNING id
        "#
    }

    pub fn update() -> &'static str {
        r#"
        UPDATE code_comments SET
            content = COALESCE($2, content),
            resolved = COALESCE($3, resolved),
            resolved_by_id = COALESCE($4, resolved_by_id),
            resolved_at = CASE WHEN $3 = true AND resolved_at IS NULL THEN NOW() ELSE resolved_at END,
            outdated = COALESCE($5, outdated),
            updated_at = NOW()
        WHERE id = $1
        RETURNING id, uuid, project_id, pull_request_id, commit_sha, file_path, old_path,
                  line_number, old_line_number, author_id, content, parent_id,
                  resolved, resolved_by_id, resolved_at, outdated, created_at, updated_at
        "#
    }

    pub fn delete() -> &'static str {
        "DELETE FROM code_comments WHERE id = $1"
    }
}