//! Project Database Model

use super::*;
use crate::DbResult;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, Postgres, Row};
use uuid::Uuid;

/// Project visibility in the database
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "project_visibility", rename_all = "lowercase")]
pub enum DbProjectVisibility {
    Private,
    Internal,
    Public,
}

impl From<DbProjectVisibility> for onedev_core::models::project::ProjectVisibility {
    fn from(v: DbProjectVisibility) -> Self {
        match v {
            DbProjectVisibility::Private => onedev_core::models::project::ProjectVisibility::Private,
            DbProjectVisibility::Internal => onedev_core::models::project::ProjectVisibility::Internal,
            DbProjectVisibility::Public => onedev_core::models::project::ProjectVisibility::Public,
        }
    }
}

impl From<onedev_core::models::project::ProjectVisibility> for DbProjectVisibility {
    fn from(v: onedev_core::models::project::ProjectVisibility) -> Self {
        match v {
            onedev_core::models::project::ProjectVisibility::Private => DbProjectVisibility::Private,
            onedev_core::models::project::ProjectVisibility::Internal => DbProjectVisibility::Internal,
            onedev_core::models::project::ProjectVisibility::Public => DbProjectVisibility::Public,
        }
    }
}

/// Project status in the database
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "project_status", rename_all = "lowercase")]
pub enum DbProjectStatus {
    Active,
    Archived,
    Maintenance,
    Deleted,
}

impl From<DbProjectStatus> for onedev_core::models::project::ProjectStatus {
    fn from(s: DbProjectStatus) -> Self {
        match s {
            DbProjectStatus::Active => onedev_core::models::project::ProjectStatus::Active,
            DbProjectStatus::Archived => onedev_core::models::project::ProjectStatus::Archived,
            DbProjectStatus::Maintenance => onedev_core::models::project::ProjectStatus::Maintenance,
            DbProjectStatus::Deleted => onedev_core::models::project::ProjectStatus::Deleted,
        }
    }
}

impl From<onedev_core::models::project::ProjectStatus> for DbProjectStatus {
    fn from(s: onedev_core::models::project::ProjectStatus) -> Self {
        match s {
            onedev_core::models::project::ProjectStatus::Active => DbProjectStatus::Active,
            onedev_core::models::project::ProjectStatus::Archived => DbProjectStatus::Archived,
            onedev_core::models::project::ProjectStatus::Maintenance => DbProjectStatus::Maintenance,
            onedev_core::models::project::ProjectStatus::Deleted => DbProjectStatus::Deleted,
        }
    }
}

/// Owner type in the database
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "owner_type", rename_all = "lowercase")]
pub enum DbOwnerType {
    User,
    Group,
}

impl From<DbOwnerType> for onedev_core::models::project::OwnerType {
    fn from(t: DbOwnerType) -> Self {
        match t {
            DbOwnerType::User => onedev_core::models::project::OwnerType::User,
            DbOwnerType::Group => onedev_core::models::project::OwnerType::Group,
        }
    }
}

impl From<onedev_core::models::project::OwnerType> for DbOwnerType {
    fn from(t: onedev_core::models::project::OwnerType) -> Self {
        match t {
            onedev_core::models::project::OwnerType::User => DbOwnerType::User,
            onedev_core::models::project::OwnerType::Group => DbOwnerType::Group,
        }
    }
}

/// Project row from database
#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct DbProject {
    pub id: i64,
    pub uuid: Uuid,
    pub name: String,
    pub path: String,
    pub description: Option<String>,
    pub readme: Option<String>,
    pub visibility: DbProjectVisibility,
    pub status: DbProjectStatus,
    pub owner_id: i64,
    pub owner_type: DbOwnerType,
    pub parent_id: Option<i64>,
    pub forked_from_id: Option<i64>,
    pub default_branch: String,
    pub avatar_url: Option<String>,
    // Features
    pub issues_enabled: bool,
    pub merge_requests_enabled: bool,
    pub builds_enabled: bool,
    pub wiki_enabled: bool,
    pub snippets_enabled: bool,
    pub packs_enabled: bool,
    pub pages_enabled: bool,
    pub container_registry_enabled: bool,
    // Settings
    pub only_allow_merge_if_pipeline_succeeds: bool,
    pub only_allow_merge_if_all_discussions_resolved: bool,
    pub remove_source_branch_after_merge: bool,
    pub enable_merge_commit: bool,
    pub enable_squash_merge: bool,
    pub enable_fast_forward_merge: bool,
    pub squash_merge_template: Option<String>,
    // CI/CD
    pub build_timeout: i32,
    pub build_coverage_regex: Option<String>,
    // Timestamps
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
    pub last_activity_at: Option<DateTime<Utc>>,
    // Additional settings (JSON)
    pub issue_setting: Option<serde_json::Value>,
    pub pull_request_setting: Option<serde_json::Value>,
    pub build_setting: Option<serde_json::Value>,
    pub pack_setting: Option<serde_json::Value>,
    pub workspace_setting: Option<serde_json::Value>,
    pub code_indexing_setting: Option<serde_json::Value>,
    pub wiki_setting: Option<serde_json::Value>,
    pub webhooks: Option<serde_json::Value>,
    pub branch_protections: Option<serde_json::Value>,
    pub tag_protections: Option<serde_json::Value>,
}

/// Project creation input for database
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DbCreateProject {
    pub name: String,
    pub path: String,
    pub description: Option<String>,
    pub readme: Option<String>,
    pub visibility: DbProjectVisibility,
    pub owner_id: i64,
    pub owner_type: DbOwnerType,
    pub parent_id: Option<i64>,
    pub forked_from_id: Option<i64>,
    pub default_branch: String,
    pub initialize_with_readme: bool,
    pub template_name: Option<String>,
    pub import_url: Option<String>,
    pub mirror: bool,
}

/// Project update input for database
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DbUpdateProject {
    pub name: Option<String>,
    pub description: Option<String>,
    pub readme: Option<String>,
    pub visibility: Option<DbProjectVisibility>,
    pub status: Option<DbProjectStatus>,
    pub default_branch: Option<String>,
    pub avatar_url: Option<String>,
    pub issues_enabled: Option<bool>,
    pub merge_requests_enabled: Option<bool>,
    pub builds_enabled: Option<bool>,
    pub wiki_enabled: Option<bool>,
    pub snippets_enabled: Option<bool>,
    pub packs_enabled: Option<bool>,
    pub pages_enabled: Option<bool>,
    pub container_registry_enabled: Option<bool>,
    pub only_allow_merge_if_pipeline_succeeds: Option<bool>,
    pub only_allow_merge_if_all_discussions_resolved: Option<bool>,
    pub remove_source_branch_after_merge: Option<bool>,
    pub enable_merge_commit: Option<bool>,
    pub enable_squash_merge: Option<bool>,
    pub enable_fast_forward_merge: Option<bool>,
    pub squash_merge_template: Option<String>,
    pub build_timeout: Option<i32>,
    pub build_coverage_regex: Option<String>,
}

impl DbProject {
    /// Convert to core domain model
    pub fn to_core(&self) -> onedev_core::models::project::Project {
        onedev_core::models::project::Project {
            id: self.id,
            uuid: self.uuid,
            name: self.name.clone(),
            path: self.path.clone(),
            description: self.description.clone(),
            readme: self.readme.clone(),
            visibility: self.visibility.into(),
            status: self.status.into(),
            owner_id: self.owner_id,
            owner_type: self.owner_type.into(),
            parent_id: self.parent_id,
            forked_from_id: self.forked_from_id,
            default_branch: self.default_branch.clone(),
            avatar_url: self.avatar_url.clone(),
            issues_enabled: self.issues_enabled,
            merge_requests_enabled: self.merge_requests_enabled,
            builds_enabled: self.builds_enabled,
            wiki_enabled: self.wiki_enabled,
            snippets_enabled: self.snippets_enabled,
            packs_enabled: self.packs_enabled,
            pages_enabled: self.pages_enabled,
            container_registry_enabled: self.container_registry_enabled,
            only_allow_merge_if_pipeline_succeeds: self.only_allow_merge_if_pipeline_succeeds,
            only_allow_merge_if_all_discussions_resolved: self.only_allow_merge_if_all_discussions_resolved,
            remove_source_branch_after_merge: self.remove_source_branch_after_merge,
            enable_merge_commit: self.enable_merge_commit,
            enable_squash_merge: self.enable_squash_merge,
            enable_fast_forward_merge: self.enable_fast_forward_merge,
            squash_merge_template: self.squash_merge_template.clone(),
            build_timeout: self.build_timeout as u32,
            build_coverage_regex: self.build_coverage_regex.clone(),
            created_at: self.created_at,
            updated_at: self.updated_at,
            deleted_at: self.deleted_at,
            last_activity_at: self.last_activity_at,
        }
    }

    /// Get full path including parent
    pub fn full_path(&self) -> String {
        if let Some(_parent_id) = self.parent_id {
            // Would need parent lookup - simplified here
            format!("parent/{}", self.path)
        } else {
            self.path.clone()
        }
    }

    /// Check if project is fork
    pub fn is_fork(&self) -> bool {
        self.forked_from_id.is_some()
    }

    /// Check if project is archived
    pub fn is_archived(&self) -> bool {
        matches!(self.status, DbProjectStatus::Archived)
    }

    /// Check if project is public
    pub fn is_public(&self) -> bool {
        matches!(self.visibility, DbProjectVisibility::Public)
    }
}

impl DbCreateProject {
    /// Create from core CreateProject
    pub fn from_core(create: &onedev_core::models::project::CreateProject, owner_id: i64, owner_type: DbOwnerType) -> Self {
        Self {
            name: create.name.clone(),
            path: create.path.clone(),
            description: create.description.clone(),
            readme: None,
            visibility: create.visibility.into(),
            owner_id,
            owner_type,
            parent_id: None,
            forked_from_id: None,
            default_branch: "main".to_string(),
            initialize_with_readme: create.initialize_with_readme,
            template_name: create.template_name.clone(),
            import_url: create.import_url.clone(),
            mirror: create.mirror,
        }
    }
}

/// Database query builder for projects
pub struct ProjectQueryBuilder;

impl ProjectQueryBuilder {
    pub fn select_all() -> &'static str {
        r#"
        SELECT id, uuid, name, path, description, readme, visibility, status,
               owner_id, owner_type, parent_id, forked_from_id, default_branch,
               avatar_url, issues_enabled, merge_requests_enabled, builds_enabled,
               wiki_enabled, snippets_enabled, packs_enabled, pages_enabled,
               container_registry_enabled, only_allow_merge_if_pipeline_succeeds,
               only_allow_merge_if_all_discussions_resolved, remove_source_branch_after_merge,
               enable_merge_commit, enable_squash_merge, enable_fast_forward_merge,
               squash_merge_template, build_timeout, build_coverage_regex,
               created_at, updated_at, deleted_at, last_activity_at,
               issue_setting, pull_request_setting, build_setting, pack_setting,
               workspace_setting, code_indexing_setting, wiki_setting, webhooks,
               branch_protections, tag_protections
        FROM projects
        "#
    }

    pub fn select_by_id() -> &'static str {
        r#"
        SELECT id, uuid, name, path, description, readme, visibility, status,
               owner_id, owner_type, parent_id, forked_from_id, default_branch,
               avatar_url, issues_enabled, merge_requests_enabled, builds_enabled,
               wiki_enabled, snippets_enabled, packs_enabled, pages_enabled,
               container_registry_enabled, only_allow_merge_if_pipeline_succeeds,
               only_allow_merge_if_all_discussions_resolved, remove_source_branch_after_merge,
               enable_merge_commit, enable_squash_merge, enable_fast_forward_merge,
               squash_merge_template, build_timeout, build_coverage_regex,
               created_at, updated_at, deleted_at, last_activity_at,
               issue_setting, pull_request_setting, build_setting, pack_setting,
               workspace_setting, code_indexing_setting, wiki_setting, webhooks,
               branch_protections, tag_protections
        FROM projects WHERE id = $1
        "#
    }

    pub fn select_by_path() -> &'static str {
        r#"
        SELECT id, uuid, name, path, description, readme, visibility, status,
               owner_id, owner_type, parent_id, forked_from_id, default_branch,
               avatar_url, issues_enabled, merge_requests_enabled, builds_enabled,
               wiki_enabled, snippets_enabled, packs_enabled, pages_enabled,
               container_registry_enabled, only_allow_merge_if_pipeline_succeeds,
               only_allow_merge_if_all_discussions_resolved, remove_source_branch_after_merge,
               enable_merge_commit, enable_squash_merge, enable_fast_forward_merge,
               squash_merge_template, build_timeout, build_coverage_regex,
               created_at, updated_at, deleted_at, last_activity_at,
               issue_setting, pull_request_setting, build_setting, pack_setting,
               workspace_setting, code_indexing_setting, wiki_setting, webhooks,
               branch_protections, tag_protections
        FROM projects WHERE path = $1 AND parent_id IS NOT DISTINCT FROM $2
        "#
    }

    pub fn select_by_uuid() -> &'static str {
        r#"
        SELECT id, uuid, name, path, description, readme, visibility, status,
               owner_id, owner_type, parent_id, forked_from_id, default_branch,
               avatar_url, issues_enabled, merge_requests_enabled, builds_enabled,
               wiki_enabled, snippets_enabled, packs_enabled, pages_enabled,
               container_registry_enabled, only_allow_merge_if_pipeline_succeeds,
               only_allow_merge_if_all_discussions_resolved, remove_source_branch_after_merge,
               enable_merge_commit, enable_squash_merge, enable_fast_forward_merge,
               squash_merge_template, build_timeout, build_coverage_regex,
               created_at, updated_at, deleted_at, last_activity_at,
               issue_setting, pull_request_setting, build_setting, pack_setting,
               workspace_setting, code_indexing_setting, wiki_setting, webhooks,
               branch_protections, tag_protections
        FROM projects WHERE uuid = $1
        "#
    }

    pub fn insert() -> &'static str {
        r#"
        INSERT INTO projects (uuid, name, path, description, readme, visibility, status,
                              owner_id, owner_type, parent_id, forked_from_id, default_branch,
                              avatar_url, issues_enabled, merge_requests_enabled, builds_enabled,
                              wiki_enabled, snippets_enabled, packs_enabled, pages_enabled,
                              container_registry_enabled, only_allow_merge_if_pipeline_succeeds,
                              only_allow_merge_if_all_discussions_resolved, remove_source_branch_after_merge,
                              enable_merge_commit, enable_squash_merge, enable_fast_forward_merge,
                              squash_merge_template, build_timeout, build_coverage_regex,
                              created_at, updated_at, issue_setting, pull_request_setting,
                              build_setting, pack_setting, workspace_setting, code_indexing_setting,
                              wiki_setting, webhooks, branch_protections, tag_protections)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18, $19, $20, $21,
                $22, $23, $24, $25, $26, $27, $28, $29, $30, $31, $32, $33, $34, $35, $36, $37, $38, $39, $40, $41, $42)
        RETURNING id
        "#
    }

    pub fn update() -> &'static str {
        r#"
        UPDATE projects SET
            name = COALESCE($2, name),
            description = COALESCE($3, description),
            readme = COALESCE($4, readme),
            visibility = COALESCE($5, visibility),
            status = COALESCE($6, status),
            default_branch = COALESCE($7, default_branch),
            avatar_url = COALESCE($8, avatar_url),
            issues_enabled = COALESCE($9, issues_enabled),
            merge_requests_enabled = COALESCE($10, merge_requests_enabled),
            builds_enabled = COALESCE($11, builds_enabled),
            wiki_enabled = COALESCE($12, wiki_enabled),
            snippets_enabled = COALESCE($13, snippets_enabled),
            packs_enabled = COALESCE($14, packs_enabled),
            pages_enabled = COALESCE($15, pages_enabled),
            container_registry_enabled = COALESCE($16, container_registry_enabled),
            only_allow_merge_if_pipeline_succeeds = COALESCE($17, only_allow_merge_if_pipeline_succeeds),
            only_allow_merge_if_all_discussions_resolved = COALESCE($18, only_allow_merge_if_all_discussions_resolved),
            remove_source_branch_after_merge = COALESCE($19, remove_source_branch_after_merge),
            enable_merge_commit = COALESCE($20, enable_merge_commit),
            enable_squash_merge = COALESCE($21, enable_squash_merge),
            enable_fast_forward_merge = COALESCE($22, enable_fast_forward_merge),
            squash_merge_template = COALESCE($23, squash_merge_template),
            build_timeout = COALESCE($24, build_timeout),
            build_coverage_regex = COALESCE($25, build_coverage_regex),
            updated_at = NOW(),
            issue_setting = COALESCE($26, issue_setting),
            pull_request_setting = COALESCE($27, pull_request_setting),
            build_setting = COALESCE($28, build_setting),
            pack_setting = COALESCE($29, pack_setting),
            workspace_setting = COALESCE($30, workspace_setting),
            code_indexing_setting = COALESCE($31, code_indexing_setting),
            wiki_setting = COALESCE($32, wiki_setting),
            webhooks = COALESCE($33, webhooks),
            branch_protections = COALESCE($34, branch_protections),
            tag_protections = COALESCE($35, tag_protections)
        WHERE id = $1
        RETURNING id, uuid, name, path, description, readme, visibility, status,
                  owner_id, owner_type, parent_id, forked_from_id, default_branch,
                  avatar_url, issues_enabled, merge_requests_enabled, builds_enabled,
                  wiki_enabled, snippets_enabled, packs_enabled, pages_enabled,
                  container_registry_enabled, only_allow_merge_if_pipeline_succeeds,
                  only_allow_merge_if_all_discussions_resolved, remove_source_branch_after_merge,
                  enable_merge_commit, enable_squash_merge, enable_fast_forward_merge,
                  squash_merge_template, build_timeout, build_coverage_regex,
                  created_at, updated_at, deleted_at, last_activity_at,
                  issue_setting, pull_request_setting, build_setting, pack_setting,
                  workspace_setting, code_indexing_setting, wiki_setting, webhooks,
                  branch_protections, tag_protections
        "#
    }

    pub fn delete() -> &'static str {
        "UPDATE projects SET deleted_at = NOW(), updated_at = NOW(), status = 'deleted' WHERE id = $1"
    }

    pub fn hard_delete() -> &'static str {
        "DELETE FROM projects WHERE id = $1"
    }

    pub fn list_with_pagination() -> &'static str {
        r#"
        SELECT id, uuid, name, path, description, readme, visibility, status,
               owner_id, owner_type, parent_id, forked_from_id, default_branch,
               avatar_url, issues_enabled, merge_requests_enabled, builds_enabled,
               wiki_enabled, snippets_enabled, packs_enabled, pages_enabled,
               container_registry_enabled, only_allow_merge_if_pipeline_succeeds,
               only_allow_merge_if_all_discussions_resolved, remove_source_branch_after_merge,
               enable_merge_commit, enable_squash_merge, enable_fast_forward_merge,
               squash_merge_template, build_timeout, build_coverage_regex,
               created_at, updated_at, deleted_at, last_activity_at,
               issue_setting, pull_request_setting, build_setting, pack_setting,
               workspace_setting, code_indexing_setting, wiki_setting, webhooks,
               branch_protections, tag_protections
        FROM projects
        WHERE deleted_at IS NULL
        ORDER BY created_at DESC
        LIMIT $1 OFFSET $2
        "#
    }

    pub fn count() -> &'static str {
        "SELECT COUNT(*) FROM projects WHERE deleted_at IS NULL"
    }
}