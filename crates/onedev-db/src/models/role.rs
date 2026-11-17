//! Role Database Model — Vanilla free-form role record (Role.java).
//!
//! Privileges are flags/JSON on the role row itself; there is no separate
//! permission table and no fixed role names.

use chrono::{DateTime, Utc};
use onedev_core::models::role::{CodePrivilege, IssueFieldSet, JobPrivilege, PackPrivilege, Role};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

const COLUMNS: &str = "id, uuid, name, description, is_system, \
    manage_project, create_children, manage_pull_requests, manage_code_comments, \
    code_privilege, pack_privilege, manage_issues, access_confidential_issues, \
    access_time_tracking, schedule_issues, editable_issue_fields, \
    can_edit_fields_of_other_issues, editable_issue_links, manage_builds, \
    upload_cache, job_privileges, accessible_sites, manage_workspaces, \
    create_workspaces, maintenance, created_at, updated_at";

/// Role row from database
#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct DbRole {
    pub id: i64,
    pub uuid: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub is_system: bool,
    pub manage_project: bool,
    pub create_children: bool,
    pub manage_pull_requests: bool,
    pub manage_code_comments: bool,
    pub code_privilege: String,
    pub pack_privilege: String,
    pub manage_issues: bool,
    pub access_confidential_issues: bool,
    pub access_time_tracking: bool,
    pub schedule_issues: bool,
    pub editable_issue_fields: serde_json::Value,
    pub can_edit_fields_of_other_issues: bool,
    pub editable_issue_links: serde_json::Value,
    pub manage_builds: bool,
    pub upload_cache: bool,
    pub job_privileges: serde_json::Value,
    pub accessible_sites: Option<String>,
    pub manage_workspaces: bool,
    pub create_workspaces: bool,
    pub maintenance: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

fn parse_code_privilege(value: &str) -> CodePrivilege {
    match value {
        "read" => CodePrivilege::Read,
        "write" => CodePrivilege::Write,
        _ => CodePrivilege::None,
    }
}

fn parse_pack_privilege(value: &str) -> PackPrivilege {
    match value {
        "read" => PackPrivilege::Read,
        "write" => PackPrivilege::Write,
        _ => PackPrivilege::None,
    }
}

fn code_privilege_str(value: CodePrivilege) -> &'static str {
    match value {
        CodePrivilege::None => "none",
        CodePrivilege::Read => "read",
        CodePrivilege::Write => "write",
    }
}

fn pack_privilege_str(value: PackPrivilege) -> &'static str {
    match value {
        PackPrivilege::None => "none",
        PackPrivilege::Read => "read",
        PackPrivilege::Write => "write",
    }
}

impl DbRole {
    /// Convert to the core domain role.
    pub fn to_core(&self) -> Role {
        Role {
            id: self.id,
            name: self.name.clone(),
            description: self.description.clone(),
            manage_project: self.manage_project,
            create_children: self.create_children,
            manage_pull_requests: self.manage_pull_requests,
            manage_code_comments: self.manage_code_comments,
            code_privilege: parse_code_privilege(&self.code_privilege),
            pack_privilege: parse_pack_privilege(&self.pack_privilege),
            manage_issues: self.manage_issues,
            access_confidential_issues: self.access_confidential_issues,
            access_time_tracking: self.access_time_tracking,
            schedule_issues: self.schedule_issues,
            editable_issue_fields: serde_json::from_value(self.editable_issue_fields.clone())
                .unwrap_or_default(),
            can_edit_fields_of_other_issues: self.can_edit_fields_of_other_issues,
            editable_issue_links: serde_json::from_value(self.editable_issue_links.clone())
                .unwrap_or_default(),
            manage_builds: self.manage_builds,
            upload_cache: self.upload_cache,
            job_privileges: serde_json::from_value(self.job_privileges.clone())
                .unwrap_or_default(),
            accessible_sites: self.accessible_sites.clone(),
            manage_workspaces: self.manage_workspaces,
            create_workspaces: self.create_workspaces,
            maintenance: self.maintenance,
            created_at: self.created_at,
            updated_at: self.updated_at,
        }
    }
}

/// Role creation input for database
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DbCreateRole {
    pub name: String,
    pub description: Option<String>,
    pub is_system: bool,
    pub manage_project: bool,
    pub create_children: bool,
    pub manage_pull_requests: bool,
    pub manage_code_comments: bool,
    pub code_privilege: CodePrivilege,
    pub pack_privilege: PackPrivilege,
    pub manage_issues: bool,
    pub access_confidential_issues: bool,
    pub access_time_tracking: bool,
    pub schedule_issues: bool,
    pub editable_issue_fields: IssueFieldSet,
    pub can_edit_fields_of_other_issues: bool,
    pub editable_issue_links: Vec<String>,
    pub manage_builds: bool,
    pub upload_cache: bool,
    pub job_privileges: Vec<JobPrivilege>,
    pub accessible_sites: Option<String>,
    pub manage_workspaces: bool,
    pub create_workspaces: bool,
    pub maintenance: bool,
}

impl From<&Role> for DbCreateRole {
    fn from(role: &Role) -> Self {
        DbCreateRole {
            name: role.name.clone(),
            description: role.description.clone(),
            is_system: role.is_owner(),
            manage_project: role.manage_project,
            create_children: role.create_children,
            manage_pull_requests: role.manage_pull_requests,
            manage_code_comments: role.manage_code_comments,
            code_privilege: role.code_privilege,
            pack_privilege: role.pack_privilege,
            manage_issues: role.manage_issues,
            access_confidential_issues: role.access_confidential_issues,
            access_time_tracking: role.access_time_tracking,
            schedule_issues: role.schedule_issues,
            editable_issue_fields: role.editable_issue_fields.clone(),
            can_edit_fields_of_other_issues: role.can_edit_fields_of_other_issues,
            editable_issue_links: role.editable_issue_links.clone(),
            manage_builds: role.manage_builds,
            upload_cache: role.upload_cache,
            job_privileges: role.job_privileges.clone(),
            accessible_sites: role.accessible_sites.clone(),
            manage_workspaces: role.manage_workspaces,
            create_workspaces: role.create_workspaces,
            maintenance: role.maintenance,
        }
    }
}

impl DbCreateRole {
    pub fn code_privilege_str(&self) -> &'static str {
        code_privilege_str(self.code_privilege)
    }
    pub fn pack_privilege_str(&self) -> &'static str {
        pack_privilege_str(self.pack_privilege)
    }
}

/// Role update input for database (COALESCE semantics)
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DbUpdateRole {
    pub name: Option<String>,
    pub description: Option<String>,
    pub manage_project: Option<bool>,
    pub create_children: Option<bool>,
    pub manage_pull_requests: Option<bool>,
    pub manage_code_comments: Option<bool>,
    pub code_privilege: Option<CodePrivilege>,
    pub pack_privilege: Option<PackPrivilege>,
    pub manage_issues: Option<bool>,
    pub access_confidential_issues: Option<bool>,
    pub access_time_tracking: Option<bool>,
    pub schedule_issues: Option<bool>,
    pub editable_issue_fields: Option<IssueFieldSet>,
    pub can_edit_fields_of_other_issues: Option<bool>,
    pub editable_issue_links: Option<Vec<String>>,
    pub manage_builds: Option<bool>,
    pub upload_cache: Option<bool>,
    pub job_privileges: Option<Vec<JobPrivilege>>,
    pub accessible_sites: Option<String>,
    pub manage_workspaces: Option<bool>,
    pub create_workspaces: Option<bool>,
    pub maintenance: Option<bool>,
}

impl DbUpdateRole {
    pub fn code_privilege_str(&self) -> Option<&'static str> {
        self.code_privilege.map(code_privilege_str)
    }
    pub fn pack_privilege_str(&self) -> Option<&'static str> {
        self.pack_privilege.map(pack_privilege_str)
    }
}

/// Database query builder for roles
pub struct RoleQueryBuilder;

impl RoleQueryBuilder {
    pub fn select_all() -> String {
        format!("SELECT {COLUMNS} FROM roles")
    }

    pub fn select_by_id() -> String {
        format!("SELECT {COLUMNS} FROM roles WHERE id = $1")
    }

    pub fn select_by_name() -> String {
        format!("SELECT {COLUMNS} FROM roles WHERE name = $1")
    }

    pub fn insert() -> &'static str {
        r#"
        INSERT INTO roles (
            uuid, name, description, is_system,
            manage_project, create_children, manage_pull_requests, manage_code_comments,
            code_privilege, pack_privilege, manage_issues, access_confidential_issues,
            access_time_tracking, schedule_issues, editable_issue_fields,
            can_edit_fields_of_other_issues, editable_issue_links, manage_builds,
            upload_cache, job_privileges, accessible_sites, manage_workspaces,
            create_workspaces, maintenance, created_at, updated_at)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16,
                $17, $18, $19, $20, $21, $22, $23, $24, $25, $26)
        RETURNING id
        "#
    }

    pub fn update() -> String {
        format!(
            r#"
        UPDATE roles SET
            name = COALESCE($2, name),
            description = COALESCE($3, description),
            manage_project = COALESCE($4, manage_project),
            create_children = COALESCE($5, create_children),
            manage_pull_requests = COALESCE($6, manage_pull_requests),
            manage_code_comments = COALESCE($7, manage_code_comments),
            code_privilege = COALESCE($8, code_privilege),
            pack_privilege = COALESCE($9, pack_privilege),
            manage_issues = COALESCE($10, manage_issues),
            access_confidential_issues = COALESCE($11, access_confidential_issues),
            access_time_tracking = COALESCE($12, access_time_tracking),
            schedule_issues = COALESCE($13, schedule_issues),
            editable_issue_fields = COALESCE($14, editable_issue_fields),
            can_edit_fields_of_other_issues = COALESCE($15, can_edit_fields_of_other_issues),
            editable_issue_links = COALESCE($16, editable_issue_links),
            manage_builds = COALESCE($17, manage_builds),
            upload_cache = COALESCE($18, upload_cache),
            job_privileges = COALESCE($19, job_privileges),
            accessible_sites = COALESCE($20, accessible_sites),
            manage_workspaces = COALESCE($21, manage_workspaces),
            create_workspaces = COALESCE($22, create_workspaces),
            maintenance = COALESCE($23, maintenance),
            updated_at = NOW()
        WHERE id = $1
        RETURNING {COLUMNS}
        "#
        )
    }

    pub fn delete() -> &'static str {
        "DELETE FROM roles WHERE id = $1 AND is_system = false"
    }
}
