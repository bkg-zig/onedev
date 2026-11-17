//! Permissions System

use crate::models::*;

/// Permission trait - all permissions implement this
pub trait Permission: Send + Sync + std::fmt::Debug + 'static {
    fn name(&self) -> &'static str;
    fn implies(&self, other: &dyn Permission) -> bool;
    fn is_write(&self) -> bool;
    fn required_roles(&self) -> Vec<&'static str>;
    /// Needed to downcast `&dyn Permission` inside `implies`.
    fn as_any(&self) -> &dyn std::any::Any;
}

/// Base permission for project-level access
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ProjectPermission<P: Permission> {
    pub project_id: i64,
    pub permission: P,
}

impl<P: Permission> Permission for ProjectPermission<P> {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn name(&self) -> &'static str {
        self.permission.name()
    }

    fn implies(&self, other: &dyn Permission) -> bool {
        if let Some(other_proj) = other.as_any().downcast_ref::<ProjectPermission<P>>() {
            self.project_id == other_proj.project_id && self.permission.implies(&other_proj.permission)
        } else {
            false
        }
    }

    fn is_write(&self) -> bool {
        self.permission.is_write()
    }

    fn required_roles(&self) -> Vec<&'static str> {
        self.permission.required_roles()
    }
}

impl<P: Permission> ProjectPermission<P> {
    pub fn new(project_id: i64, permission: P) -> Self {
        Self { project_id, permission }
    }
}

/// Job-specific permission
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct JobPermission<P: Permission> {
    pub job_name: String,
    pub permission: P,
}

impl<P: Permission> Permission for JobPermission<P> {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn name(&self) -> &'static str {
        self.permission.name()
    }

    fn implies(&self, other: &dyn Permission) -> bool {
        if let Some(other_job) = other.as_any().downcast_ref::<JobPermission<P>>() {
            self.job_name == other_job.job_name && self.permission.implies(&other_job.permission)
        } else {
            false
        }
    }

    fn is_write(&self) -> bool {
        self.permission.is_write()
    }

    fn required_roles(&self) -> Vec<&'static str> {
        self.permission.required_roles()
    }
}

/// Base permissions (read/write)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum BasePermission {
    Read,
    Write,
    Admin,
}

impl Permission for BasePermission {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn name(&self) -> &'static str {
        match self {
            BasePermission::Read => "read",
            BasePermission::Write => "write",
            BasePermission::Admin => "admin",
        }
    }

    fn implies(&self, other: &dyn Permission) -> bool {
        if let Some(other_base) = other.as_any().downcast_ref::<BasePermission>() {
            match (self, other_base) {
                (BasePermission::Admin, _) => true,
                (BasePermission::Write, BasePermission::Read) => true,
                (BasePermission::Write, BasePermission::Write) => true,
                (BasePermission::Read, BasePermission::Read) => true,
                _ => false,
            }
        } else {
            false
        }
    }

    fn is_write(&self) -> bool {
        matches!(self, BasePermission::Write | BasePermission::Admin)
    }

    fn required_roles(&self) -> Vec<&'static str> {
        match self {
            BasePermission::Read => vec!["Guest"],
            BasePermission::Write => vec!["Developer"],
            BasePermission::Admin => vec!["Maintainer"],
        }
    }
}

/// Specific permissions
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ProjectPermissionType {
    // Project
    ViewProject,
    ManageProject,
    AdminProject,
    CreateProject,
    DeleteProject,
    RenameProject,
    TransferProject,
    ManageMembers,
    ManageHooks,
    ManageDeployKeys,
    ManageProtectedBranches,
    ManageProtectedTags,
    ManagePages,
    ManageWiki,
    ManageSnippets,
    ManageSettings,
    ManageVariables,
    ManageFeatureFlags,

    // Repository
    ReadCode,
    WriteCode,
    PushCode,
    ForcePush,
    CreateBranch,
    DeleteBranch,
    CreateTag,
    DeleteTag,
    ManageBranchRules,
    ManageTagRules,

    // Issues
    ViewIssues,
    CreateIssue,
    EditIssue,
    CloseIssue,
    ReopenIssue,
    DeleteIssue,
    MoveIssue,
    ManageIssueLabels,
    ManageIssueBoard,
    SetConfidential,
    ViewConfidentialIssues,
    ManageConfidentialIssues,

    // Merge Requests
    ViewMergeRequests,
    CreateMergeRequest,
    EditMergeRequest,
    MergeMergeRequest,
    CloseMergeRequest,
    ReopenMergeRequest,
    ApproveMergeRequest,
    ManageReviewers,
    ManageMergeSettings,

    // CI/CD
    ViewBuilds,
    TriggerBuild,
    CancelBuild,
    RetryBuild,
    EraseBuild,
    ViewBuildLogs,
    DownloadArtifacts,
    ManageBuildVariables,
    ManageBuildSchedules,
    ManageRunners,
    ManageClusters,
    ManageKubernetes,

    // Packages
    ViewPackages,
    UploadPackage,
    DeletePackage,

    // Container Registry
    ViewContainerImages,
    PushContainerImage,
    DeleteContainerImage,

    // Workspaces
    CreateWorkspace,
    ViewWorkspace,
    ManageWorkspace,
    DeleteWorkspace,

    // Code Review
    ViewCodeReviews,
    CreateCodeReview,
    ManageCodeReviews,

    // Security
    ViewSecurityDashboard,
    ManageVulnerabilities,
    ManagePolicies,
}

impl Permission for ProjectPermissionType {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn name(&self) -> &'static str {
        match self {
            ProjectPermissionType::ViewProject => "view_project",
            ProjectPermissionType::ManageProject => "manage_project",
            ProjectPermissionType::AdminProject => "admin_project",
            ProjectPermissionType::CreateProject => "create_project",
            ProjectPermissionType::DeleteProject => "delete_project",
            ProjectPermissionType::RenameProject => "rename_project",
            ProjectPermissionType::TransferProject => "transfer_project",
            ProjectPermissionType::ManageMembers => "manage_members",
            ProjectPermissionType::ManageHooks => "manage_hooks",
            ProjectPermissionType::ManageDeployKeys => "manage_deploy_keys",
            ProjectPermissionType::ManageProtectedBranches => "manage_protected_branches",
            ProjectPermissionType::ManageProtectedTags => "manage_protected_tags",
            ProjectPermissionType::ManagePages => "manage_pages",
            ProjectPermissionType::ManageWiki => "manage_wiki",
            ProjectPermissionType::ManageSnippets => "manage_snippets",
            ProjectPermissionType::ManageSettings => "manage_settings",
            ProjectPermissionType::ManageVariables => "manage_variables",
            ProjectPermissionType::ManageFeatureFlags => "manage_feature_flags",
            ProjectPermissionType::ReadCode => "read_code",
            ProjectPermissionType::WriteCode => "write_code",
            ProjectPermissionType::PushCode => "push_code",
            ProjectPermissionType::ForcePush => "force_push",
            ProjectPermissionType::CreateBranch => "create_branch",
            ProjectPermissionType::DeleteBranch => "delete_branch",
            ProjectPermissionType::CreateTag => "create_tag",
            ProjectPermissionType::DeleteTag => "delete_tag",
            ProjectPermissionType::ManageBranchRules => "manage_branch_rules",
            ProjectPermissionType::ManageTagRules => "manage_tag_rules",
            ProjectPermissionType::ViewIssues => "view_issues",
            ProjectPermissionType::CreateIssue => "create_issue",
            ProjectPermissionType::EditIssue => "edit_issue",
            ProjectPermissionType::CloseIssue => "close_issue",
            ProjectPermissionType::ReopenIssue => "reopen_issue",
            ProjectPermissionType::DeleteIssue => "delete_issue",
            ProjectPermissionType::MoveIssue => "move_issue",
            ProjectPermissionType::ManageIssueLabels => "manage_issue_labels",
            ProjectPermissionType::ManageIssueBoard => "manage_issue_board",
            ProjectPermissionType::SetConfidential => "set_confidential",
            ProjectPermissionType::ViewConfidentialIssues => "view_confidential_issues",
            ProjectPermissionType::ManageConfidentialIssues => "manage_confidential_issues",
            ProjectPermissionType::ViewMergeRequests => "view_merge_requests",
            ProjectPermissionType::CreateMergeRequest => "create_merge_request",
            ProjectPermissionType::EditMergeRequest => "edit_merge_request",
            ProjectPermissionType::MergeMergeRequest => "merge_merge_request",
            ProjectPermissionType::CloseMergeRequest => "close_merge_request",
            ProjectPermissionType::ReopenMergeRequest => "reopen_merge_request",
            ProjectPermissionType::ApproveMergeRequest => "approve_merge_request",
            ProjectPermissionType::ManageReviewers => "manage_reviewers",
            ProjectPermissionType::ManageMergeSettings => "manage_merge_settings",
            ProjectPermissionType::ViewBuilds => "view_builds",
            ProjectPermissionType::TriggerBuild => "trigger_build",
            ProjectPermissionType::CancelBuild => "cancel_build",
            ProjectPermissionType::RetryBuild => "retry_build",
            ProjectPermissionType::EraseBuild => "erase_build",
            ProjectPermissionType::ViewBuildLogs => "view_build_logs",
            ProjectPermissionType::DownloadArtifacts => "download_artifacts",
            ProjectPermissionType::ManageBuildVariables => "manage_build_variables",
            ProjectPermissionType::ManageBuildSchedules => "manage_build_schedules",
            ProjectPermissionType::ManageRunners => "manage_runners",
            ProjectPermissionType::ManageClusters => "manage_clusters",
            ProjectPermissionType::ManageKubernetes => "manage_kubernetes",
            ProjectPermissionType::ViewPackages => "view_packages",
            ProjectPermissionType::UploadPackage => "upload_package",
            ProjectPermissionType::DeletePackage => "delete_package",
            ProjectPermissionType::ViewContainerImages => "view_container_images",
            ProjectPermissionType::PushContainerImage => "push_container_image",
            ProjectPermissionType::DeleteContainerImage => "delete_container_image",
            ProjectPermissionType::CreateWorkspace => "create_workspace",
            ProjectPermissionType::ViewWorkspace => "view_workspace",
            ProjectPermissionType::ManageWorkspace => "manage_workspace",
            ProjectPermissionType::DeleteWorkspace => "delete_workspace",
            ProjectPermissionType::ViewCodeReviews => "view_code_reviews",
            ProjectPermissionType::CreateCodeReview => "create_code_review",
            ProjectPermissionType::ManageCodeReviews => "manage_code_reviews",
            ProjectPermissionType::ViewSecurityDashboard => "view_security_dashboard",
            ProjectPermissionType::ManageVulnerabilities => "manage_vulnerabilities",
            ProjectPermissionType::ManagePolicies => "manage_policies",
        }
    }

    fn implies(&self, other: &dyn Permission) -> bool {
        if let Some(other_perm) = other.as_any().downcast_ref::<ProjectPermissionType>() {
            self.implies_permission(*other_perm)
        } else {
            false
        }
    }

    fn is_write(&self) -> bool {
        !matches!(
            self,
            ProjectPermissionType::ViewProject
                | ProjectPermissionType::ViewIssues
                | ProjectPermissionType::ViewMergeRequests
                | ProjectPermissionType::ViewBuilds
                | ProjectPermissionType::ViewBuildLogs
                | ProjectPermissionType::DownloadArtifacts
                | ProjectPermissionType::ViewPackages
                | ProjectPermissionType::ViewContainerImages
                | ProjectPermissionType::ViewWorkspace
                | ProjectPermissionType::ViewCodeReviews
                | ProjectPermissionType::ViewSecurityDashboard
                | ProjectPermissionType::ViewConfidentialIssues
                | ProjectPermissionType::ReadCode
        )
    }

    fn required_roles(&self) -> Vec<&'static str> {
        match self {
            // Read permissions - Guest
            ProjectPermissionType::ViewProject
            | ProjectPermissionType::ViewIssues
            | ProjectPermissionType::ViewMergeRequests
            | ProjectPermissionType::ViewBuilds
            | ProjectPermissionType::ViewBuildLogs
            | ProjectPermissionType::DownloadArtifacts
            | ProjectPermissionType::ViewPackages
            | ProjectPermissionType::ViewContainerImages
            | ProjectPermissionType::ViewWorkspace
            | ProjectPermissionType::ViewCodeReviews
            | ProjectPermissionType::ViewSecurityDashboard
            | ProjectPermissionType::ReadCode => vec!["Guest"],

            // Confidential issues are visible to Reporter and above
            ProjectPermissionType::ViewConfidentialIssues => vec!["Reporter"],

            // Write permissions - Developer
            ProjectPermissionType::CreateIssue
            | ProjectPermissionType::EditIssue
            | ProjectPermissionType::CloseIssue
            | ProjectPermissionType::ReopenIssue
            | ProjectPermissionType::CreateMergeRequest
            | ProjectPermissionType::EditMergeRequest
            | ProjectPermissionType::TriggerBuild
            | ProjectPermissionType::RetryBuild
            | ProjectPermissionType::CreateWorkspace
            | ProjectPermissionType::WriteCode
            | ProjectPermissionType::PushCode
            | ProjectPermissionType::CreateBranch
            | ProjectPermissionType::CreateTag
            | ProjectPermissionType::UploadPackage
            | ProjectPermissionType::PushContainerImage
            | ProjectPermissionType::CreateCodeReview => vec!["Developer"],

            // Maintainer permissions
            ProjectPermissionType::ManageProject
            | ProjectPermissionType::ManageMembers
            | ProjectPermissionType::ManageHooks
            | ProjectPermissionType::ManageDeployKeys
            | ProjectPermissionType::ManageProtectedBranches
            | ProjectPermissionType::ManageProtectedTags
            | ProjectPermissionType::ManagePages
            | ProjectPermissionType::ManageWiki
            | ProjectPermissionType::ManageSnippets
            | ProjectPermissionType::ManageSettings
            | ProjectPermissionType::ManageVariables
            | ProjectPermissionType::ManageFeatureFlags
            | ProjectPermissionType::ManageBranchRules
            | ProjectPermissionType::ManageTagRules
            | ProjectPermissionType::ManageIssueLabels
            | ProjectPermissionType::ManageIssueBoard
            | ProjectPermissionType::SetConfidential
            | ProjectPermissionType::ManageConfidentialIssues
            | ProjectPermissionType::MergeMergeRequest
            | ProjectPermissionType::CloseMergeRequest
            | ProjectPermissionType::ReopenMergeRequest
            | ProjectPermissionType::ApproveMergeRequest
            | ProjectPermissionType::ManageReviewers
            | ProjectPermissionType::ManageMergeSettings
            | ProjectPermissionType::CancelBuild
            | ProjectPermissionType::EraseBuild
            | ProjectPermissionType::ManageBuildVariables
            | ProjectPermissionType::ManageBuildSchedules
            | ProjectPermissionType::ManageRunners
            | ProjectPermissionType::ManageClusters
            | ProjectPermissionType::ManageKubernetes
            | ProjectPermissionType::DeletePackage
            | ProjectPermissionType::DeleteContainerImage
            | ProjectPermissionType::ManageWorkspace
            | ProjectPermissionType::DeleteWorkspace
            | ProjectPermissionType::ManageCodeReviews
            | ProjectPermissionType::ManageVulnerabilities
            | ProjectPermissionType::ManagePolicies
            | ProjectPermissionType::ForcePush
            | ProjectPermissionType::DeleteBranch
            | ProjectPermissionType::DeleteTag
            | ProjectPermissionType::DeleteIssue
            | ProjectPermissionType::MoveIssue => vec!["Maintainer"],

            // Admin permissions - Owner
            ProjectPermissionType::AdminProject
            | ProjectPermissionType::CreateProject
            | ProjectPermissionType::DeleteProject
            | ProjectPermissionType::RenameProject
            | ProjectPermissionType::TransferProject => vec!["Owner"],
        }
    }
}

impl ProjectPermissionType {
    fn implies_permission(&self, other: ProjectPermissionType) -> bool {
        use ProjectPermissionType::*;
        match (self, other) {
            // Admin implies everything
            (AdminProject, _) => true,
            (ManageProject, p) if p.is_write() => true,
            // Manage implies write and read
            (ManageProject, ViewProject) => true,
            (ManageMembers, ViewProject) => true,
            (ManageHooks, ViewProject) => true,
            (ManageDeployKeys, ViewProject) => true,
            (ManageProtectedBranches, ViewProject) => true,
            (ManageProtectedTags, ViewProject) => true,
            (ManagePages, ViewProject) => true,
            (ManageWiki, ViewProject) => true,
            (ManageSnippets, ViewProject) => true,
            (ManageSettings, ViewProject) => true,
            (ManageVariables, ViewProject) => true,
            (ManageFeatureFlags, ViewProject) => true,
            // Write implies read
            (WriteCode, ReadCode) => true,
            (PushCode, ReadCode) => true,
            (ForcePush, PushCode) => true,
            (ForcePush, WriteCode) => true,
            (CreateBranch, WriteCode) => true,
            (CreateTag, WriteCode) => true,
            (ManageBranchRules, ViewProject) => true,
            (ManageTagRules, ViewProject) => true,
            (EditIssue, ViewIssues) => true,
            (CloseIssue, ViewIssues) => true,
            (ReopenIssue, ViewIssues) => true,
            (DeleteIssue, ViewIssues) => true,
            (MoveIssue, ViewIssues) => true,
            (ManageIssueLabels, ViewIssues) => true,
            (ManageIssueBoard, ViewIssues) => true,
            (SetConfidential, ViewIssues) => true,
            (ManageConfidentialIssues, ViewConfidentialIssues) => true,
            (ViewConfidentialIssues, ViewIssues) => true,
            (EditMergeRequest, ViewMergeRequests) => true,
            (MergeMergeRequest, ViewMergeRequests) => true,
            (CloseMergeRequest, ViewMergeRequests) => true,
            (ReopenMergeRequest, ViewMergeRequests) => true,
            (ApproveMergeRequest, ViewMergeRequests) => true,
            (ManageReviewers, ViewMergeRequests) => true,
            (ManageMergeSettings, ViewMergeRequests) => true,
            (TriggerBuild, ViewBuilds) => true,
            (CancelBuild, ViewBuilds) => true,
            (RetryBuild, ViewBuilds) => true,
            (EraseBuild, ViewBuilds) => true,
            (ManageBuildVariables, ViewBuilds) => true,
            (ManageBuildSchedules, ViewBuilds) => true,
            (ManageRunners, ViewBuilds) => true,
            (ManageClusters, ViewBuilds) => true,
            (ManageKubernetes, ViewBuilds) => true,
            (UploadPackage, ViewPackages) => true,
            (DeletePackage, ViewPackages) => true,
            (PushContainerImage, ViewContainerImages) => true,
            (DeleteContainerImage, ViewContainerImages) => true,
            (ManageWorkspace, ViewWorkspace) => true,
            (DeleteWorkspace, ViewWorkspace) => true,
            (CreateCodeReview, ViewCodeReviews) => true,
            (ManageCodeReviews, ViewCodeReviews) => true,
            (ManageVulnerabilities, ViewSecurityDashboard) => true,
            (ManagePolicies, ViewSecurityDashboard) => true,
            // Self
            (a, b) if *a == b => true,
            _ => false,
        }
    }
}

/// System-wide permissions
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SystemPermission {
    AdminSystem,
    AdminUsers,
    AdminGroups,
    AdminSettings,
    ViewAuditLog,
    ManageLicense,
    ManageRunners,
    ManageClusters,
    ManageKubernetes,
    Impersonate,
    ViewAllProjects,
    ViewAllGroups,
}

impl Permission for SystemPermission {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn name(&self) -> &'static str {
        match self {
            SystemPermission::AdminSystem => "admin_system",
            SystemPermission::AdminUsers => "admin_users",
            SystemPermission::AdminGroups => "admin_groups",
            SystemPermission::AdminSettings => "admin_settings",
            SystemPermission::ViewAuditLog => "view_audit_log",
            SystemPermission::ManageLicense => "manage_license",
            SystemPermission::ManageRunners => "manage_runners",
            SystemPermission::ManageClusters => "manage_clusters",
            SystemPermission::ManageKubernetes => "manage_kubernetes",
            SystemPermission::Impersonate => "impersonate",
            SystemPermission::ViewAllProjects => "view_all_projects",
            SystemPermission::ViewAllGroups => "view_all_groups",
        }
    }

    fn implies(&self, other: &dyn Permission) -> bool {
        if let Some(other_sys) = other.as_any().downcast_ref::<SystemPermission>() {
            // AdminSystem implies every system permission, all others only themselves
            matches!(self, SystemPermission::AdminSystem) || self == other_sys
        } else {
            false
        }
    }

    fn is_write(&self) -> bool {
        !matches!(self, SystemPermission::ViewAuditLog | SystemPermission::ViewAllProjects | SystemPermission::ViewAllGroups)
    }

    fn required_roles(&self) -> Vec<&'static str> {
        vec!["Admin"]
    }
}

/// Permission checker
pub struct PermissionChecker;

impl PermissionChecker {
    pub fn check_user_permissions(_user_id: i64, _project_id: i64, _permissions: &[ProjectPermissionType]) -> bool {
        // Implementation would check user's role in project
        // and verify all permissions are granted
        true // Placeholder
    }

    pub fn check_group_permissions(_group_id: i64, _project_id: i64, _permissions: &[ProjectPermissionType]) -> bool {
        // Implementation would check group's role in project
        true // Placeholder
    }

    pub fn check_system_permissions(_user_id: i64, _permissions: &[SystemPermission]) -> bool {
        // Implementation would check if user is admin
        true // Placeholder
    }
}