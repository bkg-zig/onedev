//! Vanilla privilege model (1-to-1 port of
//! `server/security/permission/*` from OneDev v17.2.1).
//!
//! Each Java permission class is one variant here; `implies` translates
//! the Java `implies()` methods one-to-one. A `Role` (see `models::role`)
//! is a free-form record whose flags expand to a set of privileges.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

/// Space-separated include/exclude patterns (`*`, `?`, `-` excludes),
/// a port of `io.onedev.server.util.patternset.PatternSet` for plain
/// string values.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct PatternSet {
    pub includes: BTreeSet<String>,
    pub excludes: BTreeSet<String>,
}

impl PatternSet {
    pub fn parse(input: &str) -> Self {
        let mut set = PatternSet::default();
        for token in split_patterns(input) {
            if let Some(excluded) = token.strip_prefix('-') {
                if !excluded.is_empty() {
                    set.excludes.insert(excluded.to_owned());
                }
            } else {
                set.includes.insert(token);
            }
        }
        set
    }

    /// Port of `PatternSet.matches(StringMatcher, value)`.
    pub fn matches(&self, value: &str) -> bool {
        for exclude in &self.excludes {
            if glob_matches(exclude, value) {
                return false;
            }
        }
        for include in &self.includes {
            if glob_matches(include, value) {
                return true;
            }
        }
        if self.excludes.is_empty() {
            false
        } else {
            self.includes.is_empty()
        }
    }
}

/// Split on whitespace, keeping double-quoted segments together.
fn split_patterns(input: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut current = String::new();
    let mut quoted = false;
    for ch in input.chars() {
        match ch {
            '"' => quoted = !quoted,
            c if c.is_whitespace() && !quoted => {
                if !current.is_empty() {
                    out.push(std::mem::take(&mut current));
                }
            }
            c => current.push(c),
        }
    }
    if !current.is_empty() {
        out.push(current);
    }
    out
}

/// Wildcard match with `*` and `?` (same semantics as the git layer).
pub fn glob_matches(pattern: &str, value: &str) -> bool {
    let pattern: Vec<char> = pattern.chars().collect();
    let value: Vec<char> = value.chars().collect();
    let mut prev = vec![false; value.len() + 1];
    prev[0] = true;
    for ch in pattern {
        let mut current = vec![false; value.len() + 1];
        if ch == '*' {
            current[0] = prev[0];
            for i in 1..=value.len() {
                current[i] = prev[i] || current[i - 1];
            }
        } else {
            for i in 1..=value.len() {
                current[i] = prev[i - 1] && (ch == '?' || ch == value[i - 1]);
            }
        }
        prev = current;
    }
    prev[value.len()]
}

/// A privilege, mirroring one Java permission class.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Privilege {
    AccessProject,
    MaintainProject,
    ManageProject,
    CreateChildren,
    ManagePullRequests,
    ManageCodeComments,
    ReadCode,
    WriteCode,
    ReadPack,
    WritePack,
    ManageIssues,
    AccessConfidentialIssues,
    AccessTimeTracking,
    ScheduleIssues,
    /// `None` means all fields (vanilla: `EditIssueField(null)`).
    EditIssueField(Option<BTreeSet<String>>),
    EditFieldsOfOtherIssues,
    /// `None` means any link (vanilla: `EditIssueLink(null)`); otherwise the link name.
    EditIssueLink(Option<String>),
    ManageBuilds,
    UploadCache,
    ManageWorkspaces,
    CreateWorkspaces,
    /// Space-separated site patterns.
    AccessSites(String),
    /// Job-scoped privilege: job name patterns + inner privilege.
    Job {
        job_names: String,
        privilege: Box<Privilege>,
    },
    ManageJob,
    RunJob,
    AccessBuildLog,
    AccessBuildPipeline,
    /// Space-separated report patterns.
    AccessBuildReports(String),
    /// Access to one confidential issue (by id).
    ConfidentialIssue(i64),
    /// Access to a set of confidential issues (by id).
    ConfidentialIssues(BTreeSet<i64>),
    CreateRootProjects,
    SystemAdministration,
    /// Administration of one user (by id).
    UserAdministration(i64),
}

/// A privilege bound to a project (vanilla `ProjectPermission`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectPrivilege {
    pub project_id: i64,
    pub privilege: Privilege,
}

impl ProjectPrivilege {
    pub fn new(project_id: i64, privilege: Privilege) -> Self {
        Self { project_id, privilege }
    }

    /// Vanilla: the granting project must be the required project itself
    /// or one of its (settings) ancestors, and the privilege must imply.
    pub fn implies(&self, other: &ProjectPrivilege, ancestors_of_other: &[i64]) -> bool {
        (self.project_id == other.project_id || ancestors_of_other.contains(&self.project_id))
            && self.privilege.implies(&other.privilege)
    }
}

impl Privilege {
    /// Is this a write privilege (vanilla `WritePermission` marker)?
    pub fn is_write(&self) -> bool {
        use Privilege::*;
        matches!(
            self,
            ManageProject
                | CreateChildren
                | ManagePullRequests
                | ManageCodeComments
                | WriteCode
                | WritePack
                | ManageIssues
                | ScheduleIssues
                | EditIssueField(_)
                | EditFieldsOfOtherIssues
                | EditIssueLink(_)
                | ManageBuilds
                | UploadCache
                | ManageWorkspaces
                | CreateWorkspaces
                | ManageJob
                | RunJob
        )
    }

    /// One-to-one translation of the Java `implies()` methods.
    pub fn implies(&self, other: &Privilege) -> bool {
        use Privilege::*;
        match self {
            // ManageProject.implies: always true. SystemAdministration too.
            ManageProject | SystemAdministration => true,

            AccessProject => matches!(other, AccessProject),

            MaintainProject => matches!(other, MaintainProject),

            CreateChildren => {
                matches!(other, CreateChildren) || AccessProject.implies(other)
            }

            ManagePullRequests => {
                matches!(other, ManagePullRequests) || WriteCode.implies(other)
            }
            ManageCodeComments => {
                matches!(other, ManageCodeComments) || WriteCode.implies(other)
            }

            // WriteCode -> CreateWorkspaces -> ReadCode -> AccessProject
            WriteCode => matches!(other, WriteCode) || CreateWorkspaces.implies(other),
            CreateWorkspaces => {
                matches!(other, CreateWorkspaces) || ReadCode.implies(other)
            }
            ReadCode => matches!(other, ReadCode) || AccessProject.implies(other),

            WritePack => matches!(other, WritePack) || ReadPack.implies(other),
            ReadPack => matches!(other, ReadPack),

            ManageIssues => {
                matches!(other, ManageIssues)
                    || AccessTimeTracking.implies(other)
                    || AccessConfidentialIssues.implies(other)
                    || EditIssueField(None).implies(other)
                    || EditIssueLink(None).implies(other)
                    || EditFieldsOfOtherIssues.implies(other)
                    || ScheduleIssues.implies(other)
            }
            AccessConfidentialIssues => {
                matches!(other, AccessConfidentialIssues | ConfidentialIssue(_))
                    || AccessProject.implies(other)
            }
            AccessTimeTracking => {
                matches!(other, AccessTimeTracking) || AccessProject.implies(other)
            }
            ScheduleIssues => {
                matches!(other, ScheduleIssues) || AccessProject.implies(other)
            }
            EditIssueField(fields) => match other {
                EditIssueField(other_fields) => match (fields, other_fields) {
                    (None, _) => true,
                    (Some(_), None) => false,
                    (Some(mine), Some(theirs)) => theirs.is_subset(mine),
                },
                _ => AccessProject.implies(other),
            },
            EditFieldsOfOtherIssues => matches!(other, EditFieldsOfOtherIssues),
            EditIssueLink(link) => match other {
                EditIssueLink(other_link) => {
                    link.is_none() || link == other_link
                }
                _ => AccessProject.implies(other),
            },

            ManageBuilds => {
                matches!(other, ManageBuilds)
                    || UploadCache.implies(other)
                    || Job {
                        job_names: "*".to_owned(),
                        privilege: Box::new(ManageJob),
                    }
                    .implies(other)
            }
            UploadCache => matches!(other, UploadCache),

            ManageWorkspaces => {
                matches!(other, ManageWorkspaces)
                    || CreateWorkspaces.implies(other)
                    || WriteCode.implies(other)
            }

            AccessSites(sites) => match other {
                AccessSites(other_sites) => PatternSet::parse(sites).matches(other_sites),
                _ => AccessProject.implies(other),
            },

            Job { job_names, privilege } => match other {
                Job {
                    job_names: other_names,
                    privilege: other_privilege,
                } => {
                    PatternSet::parse(job_names).matches(other_names)
                        && privilege.implies(other_privilege)
                }
                _ => false,
            },
            ManageJob => matches!(other, ManageJob) || RunJob.implies(other),
            RunJob => {
                matches!(other, RunJob)
                    || AccessBuildLog.implies(other)
                    || AccessBuildPipeline.implies(other)
                    || AccessBuildReports("*".to_owned()).implies(other)
            }
            AccessBuildLog => matches!(other, AccessBuildLog),
            AccessBuildPipeline => matches!(other, AccessBuildPipeline),
            AccessBuildReports(reports) => match other {
                AccessBuildReports(other_reports) => {
                    PatternSet::parse(reports).matches(other_reports)
                }
                _ => AccessProject.implies(other),
            },

            ConfidentialIssue(id) => match other {
                ConfidentialIssue(other_id) => id == other_id,
                _ => false,
            },
            ConfidentialIssues(ids) => match other {
                ConfidentialIssue(id) => ids.contains(id),
                ConfidentialIssues(other_ids) => other_ids.is_subset(ids),
                _ => false,
            },

            CreateRootProjects => matches!(other, CreateRootProjects),
            UserAdministration(id) => match other {
                UserAdministration(other_id) => id == other_id,
                _ => false,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn set(items: &[&str]) -> BTreeSet<String> {
        items.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn manage_project_implies_everything() {
        assert!(Privilege::ManageProject.implies(&Privilege::WriteCode));
        assert!(Privilege::ManageProject.implies(&Privilege::AccessProject));
        assert!(Privilege::ManageProject.implies(&Privilege::ManageIssues));
    }

    #[test]
    fn write_code_chain_reaches_access_project() {
        // WriteCode -> CreateWorkspaces -> ReadCode -> AccessProject
        assert!(Privilege::WriteCode.implies(&Privilege::CreateWorkspaces));
        assert!(Privilege::WriteCode.implies(&Privilege::ReadCode));
        assert!(Privilege::WriteCode.implies(&Privilege::AccessProject));
        assert!(!Privilege::ReadCode.implies(&Privilege::WriteCode));
        assert!(!Privilege::AccessProject.implies(&Privilege::ReadCode));
    }

    #[test]
    fn manage_issues_implies_field_and_schedule_privileges() {
        assert!(Privilege::ManageIssues.implies(&Privilege::ScheduleIssues));
        assert!(Privilege::ManageIssues.implies(&Privilege::AccessTimeTracking));
        assert!(Privilege::ManageIssues
            .implies(&Privilege::EditIssueField(Some(set(&["Title"])))));
        assert!(Privilege::ManageIssues.implies(&Privilege::AccessConfidentialIssues));
        assert!(Privilege::ManageIssues.implies(&Privilege::ConfidentialIssue(7)));
    }

    #[test]
    fn edit_issue_field_subset_semantics() {
        let all = Privilege::EditIssueField(None);
        let some = Privilege::EditIssueField(Some(set(&["A", "B"])));
        assert!(all.implies(&some));
        assert!(!some.implies(&all));
        assert!(some.implies(&Privilege::EditIssueField(Some(set(&["A"])))));
        assert!(!some.implies(&Privilege::EditIssueField(Some(set(&["C"])))));
    }

    #[test]
    fn manage_builds_implies_job_privileges_for_any_job() {
        let run_ci = Privilege::Job {
            job_names: "ci".to_owned(),
            privilege: Box::new(Privilege::RunJob),
        };
        assert!(Privilege::ManageBuilds.implies(&run_ci));
        assert!(Privilege::ManageBuilds.implies(&Privilege::UploadCache));

        let only_ci = Privilege::Job {
            job_names: "ci".to_owned(),
            privilege: Box::new(Privilege::ManageJob),
        };
        assert!(only_ci.implies(&run_ci));
        assert!(!run_ci.implies(&only_ci));
    }

    #[test]
    fn pattern_set_excludes_win() {
        let patterns = PatternSet::parse("* -secret-*");
        assert!(patterns.matches("public-report"));
        assert!(!patterns.matches("secret-report"));
        assert!(!PatternSet::parse("ci cd").matches("deploy"));
    }

    #[test]
    fn project_privilege_respects_ancestry() {
        let granted = ProjectPrivilege::new(1, Privilege::ReadCode);
        let required_child = ProjectPrivilege::new(3, Privilege::ReadCode);
        // Project 1 is an ancestor of project 3.
        assert!(granted.implies(&required_child, &[1, 2]));
        assert!(!granted.implies(&required_child, &[2]));
        let required_other = ProjectPrivilege::new(9, Privilege::WriteCode);
        assert!(!granted.implies(&required_other, &[1]));
    }
}
