//! Package Database Model

use super::*;
use crate::DbResult;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, Postgres, Row};
use uuid::Uuid;

/// Package row from database
#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct DbPack {
    pub id: i64,
    pub uuid: Uuid,
    pub project_id: i64,
    pub name: String,
    pub version: String,
    pub pack_type: DbPackType,
    pub description: Option<String>,
    pub readme: Option<String>,
    pub author_id: i64,
    pub size: i64,
    pub file_count: i32,
    pub dependencies: Option<serde_json::Value>,
    pub metadata: Option<serde_json::Value>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
}

/// Package type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "pack_type", rename_all = "lowercase")]
pub enum DbPackType {
    Maven,
    Npm,
    PyPI,
    NuGet,
    Cargo,
    Generic,
}

/// Package creation input for database
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DbCreatePack {
    pub project_id: i64,
    pub name: String,
    pub version: String,
    pub pack_type: DbPackType,
    pub description: Option<String>,
    pub readme: Option<String>,
    pub author_id: i64,
    pub size: i64,
    pub file_count: i32,
    pub dependencies: Option<serde_json::Value>,
    pub metadata: Option<serde_json::Value>,
}

/// Package update input for database
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DbUpdatePack {
    pub description: Option<String>,
    pub readme: Option<String>,
    pub metadata: Option<serde_json::Value>,
}

/// Package blob reference row from database
#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct DbPackBlobRef {
    pub id: i64,
    pub uuid: Uuid,
    pub pack_id: i64,
    pub blob_sha256: String,
    pub path: String,
    pub size: i64,
    pub created_at: DateTime<Utc>,
}

/// Package label row from database
#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct DbPackLabel {
    pub id: i64,
    pub uuid: Uuid,
    pub pack_id: i64,
    pub name: String,
    pub color: String,
    pub created_at: DateTime<Utc>,
}

/// Database query builder for packages
pub struct PackQueryBuilder;

impl PackQueryBuilder {
    pub fn select_all() -> &'static str {
        r#"
        SELECT id, uuid, project_id, name, version, pack_type, description, readme,
               author_id, size, file_count, dependencies, metadata,
               created_at, updated_at, deleted_at
        FROM packs
        "#
    }

    pub fn select_by_id() -> &'static str {
        r#"
        SELECT id, uuid, project_id, name, version, pack_type, description, readme,
               author_id, size, file_count, dependencies, metadata,
               created_at, updated_at, deleted_at
        FROM packs WHERE id = $1
        "#
    }

    pub fn select_by_project_and_name() -> &'static str {
        r#"
        SELECT id, uuid, project_id, name, version, pack_type, description, readme,
               author_id, size, file_count, dependencies, metadata,
               created_at, updated_at, deleted_at
        FROM packs WHERE project_id = $1 AND name = $2 AND deleted_at IS NULL
        ORDER BY version DESC
        "#
    }

    pub fn insert() -> &'static str {
        r#"
        INSERT INTO packs (uuid, project_id, name, version, pack_type, description, readme,
                           author_id, size, file_count, dependencies, metadata,
                           created_at, updated_at)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14)
        RETURNING id
        "#
    }

    pub fn update() -> &'static str {
        r#"
        UPDATE packs SET
            description = COALESCE($2, description),
            readme = COALESCE($3, readme),
            metadata = COALESCE($4, metadata),
            updated_at = NOW()
        WHERE id = $1
        RETURNING id, uuid, project_id, name, version, pack_type, description, readme,
                  author_id, size, file_count, dependencies, metadata,
                  created_at, updated_at, deleted_at
        "#
    }

    pub fn delete() -> &'static str {
        "UPDATE packs SET deleted_at = NOW(), updated_at = NOW() WHERE id = $1"
    }
}