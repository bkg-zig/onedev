//! Role model — 1-to-1 port of `io.onedev.server.model.Role` (v17.2.1).
//!
//! A role is a free-form record created by an administrator, not one of a
//! fixed set of names. Its flags expand to Vanilla privileges
//! (`Role.getPermissions()`); the only built-in role is `Project Owner`
//! (`OWNER_ID = 1`, `manageProject = true`), created on first start.

use std::collections::BTreeSet;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use super::user::SortOrder;
use crate::permissions::privilege::Privilege;

/// Id of the single built-in role (vanilla `Role.OWNER_ID`).
pub const OWNER_ID: i64 = 1;

/// Code privilege of a role (vanilla `CodePrivilege`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum CodePrivilege {
    #[default]
    None,
    Read,
    Write,
}

/// Package privilege of a role (vanilla `PackPrivilege`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum PackPrivilege {
    #[default]
    None,
    Read,
    Write,
}

/// Which issue fields a role may edit (vanilla `IssueFieldSet`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum IssueFieldSet {
    /// All fields known to the issue setting (resolved by the caller).
    #[default]
    All,
    None,
    Include {
        fields: BTreeSet<String>,
    },
    Exclude {
        fields: BTreeSet<String>,
    },
}

impl IssueFieldSet {
    /// Included field names. `all_fields` is the full field list of the
    /// issue setting (vanilla resolves `All`/`Exclude` against it).
    /// `None` for the `All` variant means "unrestricted" to callers that
    /// cannot resolve the setting, matching `EditIssueField(null)`.
    pub fn include_fields(&self, all_fields: &BTreeSet<String>) -> BTreeSet<String> {
        match self {
            IssueFieldSet::All => all_fields.clone(),
            IssueFieldSet::None => BTreeSet::new(),
            IssueFieldSet::Include { fields } => fields.clone(),
            IssueFieldSet::Exclude { fields } => all_fields.difference(fields).cloned().collect(),
        }
    }
}

/// Additional privileges for specific jobs (vanilla `JobPrivilege`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct JobPrivilege {
    /// Space-separated job name patterns (`*`, `?`, `-` excludes).
    pub job_names: String,
    pub manage_job: bool,
    pub run_job: bool,
    pub access_log: bool,
    pub access_pipeline: bool,
    pub accessible_reports: Option<String>,
}

/// A free-form role record (vanilla `Role` entity, without persistence).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Role {
    pub id: i64,
    pub name: String,
    pub description: Option<String>,

    pub manage_project: bool,
    pub create_children: bool,
    pub manage_pull_requests: bool,
    pub manage_code_comments: bool,
    pub code_privilege: CodePrivilege,
    pub pack_privilege: PackPrivilege,
    pub manage_issues: bool,
    pub access_confidential_issues: bool,
    /// Vanilla default is `true`.
    pub access_time_tracking: bool,
    pub schedule_issues: bool,
    pub editable_issue_fields: IssueFieldSet,
    pub can_edit_fields_of_other_issues: bool,
    /// Names of issue links this role may edit.
    pub editable_issue_links: Vec<String>,
    pub manage_builds: bool,
    pub upload_cache: bool,
    pub job_privileges: Vec<JobPrivilege>,
    pub accessible_sites: Option<String>,
    pub manage_workspaces: bool,
    pub create_workspaces: bool,
    pub maintenance: bool,

    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl Default for Role {
    fn default() -> Self {
        let now = Utc::now();
        Role {
            id: 0,
            name: String::new(),
            description: None,
            manage_project: false,
            create_children: false,
            manage_pull_requests: false,
            manage_code_comments: false,
            code_privilege: CodePrivilege::None,
            pack_privilege: PackPrivilege::None,
            manage_issues: false,
            access_confidential_issues: false,
            access_time_tracking: true,
            schedule_issues: false,
            editable_issue_fields: IssueFieldSet::All,
            can_edit_fields_of_other_issues: false,
            editable_issue_links: Vec::new(),
            manage_builds: false,
            upload_cache: false,
            job_privileges: Vec::new(),
            accessible_sites: None,
            manage_workspaces: false,
            create_workspaces: false,
            maintenance: false,
            created_at: now,
            updated_at: now,
        }
    }
}

impl Role {
    /// The single built-in role created on first start:
    /// `Project Owner` with `manageProject = true`.
    pub fn project_owner() -> Self {
        Role {
            id: OWNER_ID,
            name: "Project Owner".to_owned(),
            manage_project: true,
            ..Role::default()
        }
    }

    pub fn is_owner(&self) -> bool {
        self.id == OWNER_ID
    }

    /// Expand the role's flags to privileges — a one-to-one port of
    /// `Role.getPermissions()`. `all_issue_fields` resolves the
    /// `All`/`Exclude` field sets; pass an empty set when the issue
    /// setting is unavailable (the resulting `EditIssueField` then only
    /// implies field edits within that set).
    pub fn permissions(&self) -> Vec<Privilege> {
        self.permissions_with_fields(&BTreeSet::new())
    }

    pub fn permissions_with_fields(&self, all_issue_fields: &BTreeSet<String>) -> Vec<Privilege> {
        let mut permissions = vec![Privilege::AccessProject];

        if self.maintenance {
            permissions.push(Privilege::MaintainProject);
        }
        if self.manage_project {
            permissions.push(Privilege::ManageProject);
        }
        if self.create_children {
            permissions.push(Privilege::CreateChildren);
        }
        if self.manage_code_comments {
            permissions.push(Privilege::ManageCodeComments);
        }
        if self.manage_pull_requests {
            permissions.push(Privilege::ManagePullRequests);
        }
        match self.code_privilege {
            CodePrivilege::Read => permissions.push(Privilege::ReadCode),
            CodePrivilege::Write => permissions.push(Privilege::WriteCode),
            CodePrivilege::None => {}
        }
        match self.pack_privilege {
            PackPrivilege::Read => permissions.push(Privilege::ReadPack),
            PackPrivilege::Write => permissions.push(Privilege::WritePack),
            PackPrivilege::None => {}
        }
        if self.manage_issues {
            permissions.push(Privilege::ManageIssues);
        }
        if self.access_confidential_issues {
            permissions.push(Privilege::AccessConfidentialIssues);
        }
        if self.access_time_tracking {
            permissions.push(Privilege::AccessTimeTracking);
        }
        if self.schedule_issues {
            permissions.push(Privilege::ScheduleIssues);
        }
        // Vanilla always adds EditIssueField with the included fields;
        // an empty include set (NoneIssueFields) edits nothing.
        let fields = self.editable_issue_fields.include_fields(all_issue_fields);
        let fields = if matches!(self.editable_issue_fields, IssueFieldSet::All)
            && all_issue_fields.is_empty()
        {
            None // unresolved All behaves like EditIssueField(null)
        } else {
            Some(fields)
        };
        permissions.push(Privilege::EditIssueField(fields));
        if self.can_edit_fields_of_other_issues {
            permissions.push(Privilege::EditFieldsOfOtherIssues);
        }
        for link in &self.editable_issue_links {
            permissions.push(Privilege::EditIssueLink(Some(link.clone())));
        }
        if self.manage_builds {
            permissions.push(Privilege::ManageBuilds);
        }
        if self.manage_workspaces {
            permissions.push(Privilege::ManageWorkspaces);
        }
        if self.create_workspaces {
            permissions.push(Privilege::CreateWorkspaces);
        }
        if let Some(sites) = &self.accessible_sites {
            permissions.push(Privilege::AccessSites(sites.clone()));
        }
        if self.upload_cache {
            permissions.push(Privilege::UploadCache);
        }
        for job in &self.job_privileges {
            let scoped = |privilege: Privilege| Privilege::Job {
                job_names: job.job_names.clone(),
                privilege: Box::new(privilege),
            };
            if job.manage_job {
                permissions.push(scoped(Privilege::ManageJob));
            }
            if job.run_job {
                permissions.push(scoped(Privilege::RunJob));
            }
            if job.access_log {
                permissions.push(scoped(Privilege::AccessBuildLog));
            }
            if job.access_pipeline {
                permissions.push(scoped(Privilege::AccessBuildPipeline));
            }
            if let Some(reports) = &job.accessible_reports {
                permissions.push(scoped(Privilege::AccessBuildReports(reports.clone())));
            }
        }

        permissions
    }

    /// Vanilla `Role.implies`: any expanded privilege implies `required`.
    pub fn implies(&self, required: &Privilege) -> bool {
        self.permissions().iter().any(|p| p.implies(required))
    }
}

/// `BaseAuthorization`: a role granted on a project for everyone
/// (vanilla entity: project + role).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BaseAuthorization {
    pub id: i64,
    pub project_id: i64,
    pub role_id: i64,
}

/// `UserAuthorization`: a role granted to a user on a project.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct UserAuthorization {
    pub id: i64,
    pub project_id: i64,
    pub user_id: i64,
    pub role_id: i64,
}

/// `GroupAuthorization`: a role granted to a group on a project.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GroupAuthorization {
    pub id: i64,
    pub project_id: i64,
    pub group_id: i64,
    pub role_id: i64,
}

/// `AccessTokenAuthorization`: a role granted to an access token on a project.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AccessTokenAuthorization {
    pub id: i64,
    pub project_id: i64,
    pub access_token_id: i64,
    pub role_id: i64,
}

/// `IssueAuthorization`: a user authorized on one issue.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct IssueAuthorization {
    pub id: i64,
    pub issue_id: i64,
    pub user_id: i64,
}

/// `LinkAuthorization`: a role may edit issues via one link spec (by name).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LinkAuthorization {
    pub id: i64,
    pub link_name: String,
    pub role_id: i64,
}

/// Role creation input (all flags default like the entity).
#[derive(Debug, Clone, Serialize, Deserialize, validator::Validate)]
pub struct CreateRole {
    #[validate(length(min = 1, max = 100))]
    pub name: String,
    pub description: Option<String>,
    #[serde(default)]
    pub manage_project: bool,
    #[serde(default)]
    pub code_privilege: CodePrivilege,
    #[serde(default)]
    pub pack_privilege: PackPrivilege,
    #[serde(default)]
    pub manage_issues: bool,
    #[serde(default)]
    pub manage_builds: bool,
    #[serde(default)]
    pub manage_pull_requests: bool,
    #[serde(default)]
    pub manage_code_comments: bool,
    #[serde(default)]
    pub manage_workspaces: bool,
}

impl From<CreateRole> for Role {
    fn from(create: CreateRole) -> Self {
        Role {
            name: create.name,
            description: create.description,
            manage_project: create.manage_project,
            code_privilege: create.code_privilege,
            pack_privilege: create.pack_privilege,
            manage_issues: create.manage_issues,
            manage_builds: create.manage_builds,
            manage_pull_requests: create.manage_pull_requests,
            manage_code_comments: create.manage_code_comments,
            manage_workspaces: create.manage_workspaces,
            ..Role::default()
        }
    }
}

/// Role update input.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UpdateRole {
    pub name: Option<String>,
    pub description: Option<String>,
    pub manage_project: Option<bool>,
    pub code_privilege: Option<CodePrivilege>,
    pub pack_privilege: Option<PackPrivilege>,
    pub manage_issues: Option<bool>,
    pub manage_builds: Option<bool>,
    pub manage_pull_requests: Option<bool>,
    pub manage_code_comments: Option<bool>,
    pub manage_workspaces: Option<bool>,
}

/// Role query params.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RoleQuery {
    pub term: Option<String>,
    pub sort: Option<String>,
    pub order: Option<SortOrder>,
    pub page: u32,
    pub per_page: u32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn project_owner_implies_code_and_issue_privileges() {
        let owner = Role::project_owner();
        assert!(owner.is_owner());
        assert!(owner.implies(&Privilege::WriteCode));
        assert!(owner.implies(&Privilege::ManageIssues));
        assert!(owner.implies(&Privilege::AccessProject));
    }

    #[test]
    fn plain_role_only_accesses_project_and_time_tracking() {
        let role = Role::default();
        assert!(role.implies(&Privilege::AccessProject));
        assert!(role.implies(&Privilege::AccessTimeTracking));
        assert!(!role.implies(&Privilege::ReadCode));
        assert!(!role.implies(&Privilege::WriteCode));
    }

    #[test]
    fn write_code_privilege_implies_read_and_workspaces() {
        let role = Role {
            code_privilege: CodePrivilege::Write,
            ..Role::default()
        };
        assert!(role.implies(&Privilege::ReadCode));
        assert!(role.implies(&Privilege::CreateWorkspaces));
        assert!(!role.implies(&Privilege::ManageProject));
    }

    #[test]
    fn job_privileges_are_scoped_by_name_pattern() {
        let role = Role {
            job_privileges: vec![JobPrivilege {
                job_names: "ci-*".to_owned(),
                run_job: true,
                ..JobPrivilege::default()
            }],
            ..Role::default()
        };
        let run_ci = Privilege::Job {
            job_names: "ci-test".to_owned(),
            privilege: Box::new(Privilege::RunJob),
        };
        let run_deploy = Privilege::Job {
            job_names: "deploy".to_owned(),
            privilege: Box::new(Privilege::RunJob),
        };
        assert!(role.implies(&run_ci));
        assert!(!role.implies(&run_deploy));
    }
}
