//! Role & Permission Domain Model

use super::*;

/// Built-in role names
pub const ROLE_GUEST: &str = "Guest";
pub const ROLE_REPORTER: &str = "Reporter";
pub const ROLE_DEVELOPER: &str = "Developer";
pub const ROLE_MAINTAINER: &str = "Maintainer";
pub const ROLE_OWNER: &str = "Owner";
pub const ROLE_ADMIN: &str = "Admin";

/// Core Role entity
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Role {
    pub id: i64,
    pub uuid: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub is_builtin: bool,
    pub is_system: bool, // System roles (cannot be deleted)
    // Permissions stored as bitflags
    pub permissions: PermissionSet,
    // Project/Group scoping
    pub project_id: Option<i64>,
    pub group_id: Option<i64>,
    // Base authorizations (for default roles)
    pub base_authorizations: Vec<BaseAuthorization>,
    // Timestamps
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

bitflags::bitflags! {
    /// Permission set as bitflags
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
    pub struct PermissionSet: u64 {
        // Project permissions
        const VIEW_PROJECT           = 1 << 0;
        const READ_CODE              = 1 << 1;
        const WRITE_CODE             = 1 << 2;
        const MANAGE_PROJECT         = 1 << 3;
        const ADMIN_PROJECT          = 1 << 4;
        const CREATE_PROJECT         = 1 << 5;
        const DELETE_PROJECT         = 1 << 6;
        const RENAME_PROJECT         = 1 << 7;
        const TRANSFER_PROJECT       = 1 << 8;
        const MANAGE_MEMBERS         = 1 << 9;
        const MANAGE_HOOKS           = 1 << 10;
        const MANAGE_DEPLOY_KEYS     = 1 << 11;
        const MANAGE_PROTECTED_BRANCHES = 1 << 12;
        const MANAGE_PROTECTED_TAGS  = 1 << 13;
        const MANAGE_PAGES           = 1 << 14;
        const MANAGE_WIKI            = 1 << 15;
        const MANAGE_SNIPPETS        = 1 << 16;

        // Issue permissions
        const VIEW_ISSUES            = 1 << 17;
        const CREATE_ISSUE           = 1 << 18;
        const EDIT_ISSUE             = 1 << 19;
        const CLOSE_ISSUE            = 1 << 20;
        const REOPEN_ISSUE           = 1 << 21;
        const DELETE_ISSUE           = 1 << 22;
        const MANAGE_ISSUE_LABELS    = 1 << 23;
        const MANAGE_ISSUE_BOARD     = 1 << 24;
        const SET_ISSUE_CONFIDENTIAL = 1 << 25;
        const VIEW_CONFIDENTIAL_ISSUES = 1 << 26;

        // Merge Request permissions
        const VIEW_MERGE_REQUESTS    = 1 << 27;
        const CREATE_MERGE_REQUEST   = 1 << 28;
        const EDIT_MERGE_REQUEST     = 1 << 29;
        const MERGE_MERGE_REQUEST    = 1 << 30;
        const CLOSE_MERGE_REQUEST    = 1 << 31;
        const REOPEN_MERGE_REQUEST   = 1 << 32;
        const APPROVE_MERGE_REQUEST  = 1 << 33;
        const MANAGE_MERGE_REQUEST_REVIEWERS = 1 << 34;

        // Build/CI permissions
        const VIEW_BUILDS            = 1 << 35;
        const TRIGGER_BUILD          = 1 << 36;
        const CANCEL_BUILD           = 1 << 37;
        const RETRY_BUILD            = 1 << 38;
        const ERASE_BUILD            = 1 << 39;
        const VIEW_BUILD_LOGS        = 1 << 40;
        const DOWNLOAD_ARTIFACTS     = 1 << 41;
        const MANAGE_BUILD_VARIABLES = 1 << 42;
        const MANAGE_BUILD_SCHEDULES = 1 << 43;
        const MANAGE_RUNNERS         = 1 << 44;

        // Package permissions
        const VIEW_PACKAGES          = 1 << 45;
        const UPLOAD_PACKAGE         = 1 << 46;
        const DELETE_PACKAGE         = 1 << 47;

        // Container Registry permissions
        const VIEW_CONTAINER_IMAGES  = 1 << 48;
        const PUSH_CONTAINER_IMAGE   = 1 << 49;
        const DELETE_CONTAINER_IMAGE = 1 << 50;

        // Workspace permissions
        const CREATE_WORKSPACE       = 1 << 51;
        const VIEW_WORKSPACE         = 1 << 52;
        const MANAGE_WORKSPACE       = 1 << 53;
        const DELETE_WORKSPACE       = 1 << 54;

        // Code Review permissions
        const VIEW_CODE_REVIEWS      = 1 << 55;
        const CREATE_CODE_REVIEW     = 1 << 56;
        const MANAGE_CODE_REVIEWS    = 1 << 57;

        // Admin permissions
        const ADMIN_SYSTEM           = 1 << 58;
        const ADMIN_USERS            = 1 << 59;
        const ADMIN_GROUPS           = 1 << 60;
        const ADMIN_SETTINGS         = 1 << 61;
        const VIEW_AUDIT_LOG         = 1 << 62;
        const MANAGE_LICENSE         = 1 << 63;
    }
}

/// Base authorization (for default roles)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BaseAuthorization {
    pub id: i64,
    pub uuid: Uuid,
    pub role_id: i64,
    pub project_id: Option<i64>,
    pub group_id: Option<i64>,
    pub created_at: DateTime<Utc>,
}

/// User-Project authorization
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct UserAuthorization {
    pub id: i64,
    pub uuid: Uuid,
    pub user_id: i64,
    pub project_id: i64,
    pub role_id: i64,
    pub expires_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub created_by_id: i64,
    // Relations
    pub user: Option<UserRef>,
    pub project: Option<ProjectRef>,
    pub role: Option<Role>,
}

/// Group-Project authorization
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GroupAuthorization {
    pub id: i64,
    pub uuid: Uuid,
    pub group_id: i64,
    pub project_id: i64,
    pub role_id: i64,
    pub expires_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub created_by_id: i64,
    // Relations
    pub group: Option<GroupRef>,
    pub project: Option<ProjectRef>,
    pub role: Option<Role>,
}

/// Issue-specific authorization
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct IssueAuthorization {
    pub id: i64,
    pub uuid: Uuid,
    pub issue_id: i64,
    pub user_id: i64,
    pub access_level: IssueAccessLevel,
    pub created_at: DateTime<Utc>,
    pub created_by_id: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, strum::EnumString, strum::Display)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case")]
pub enum IssueAccessLevel {
    View,
    Edit,
    Admin,
}

/// Role creation input
#[derive(Debug, Clone, Serialize, Deserialize, validator::Validate)]
pub struct CreateRole {
    #[validate(length(min = 1, max = 100))]
    pub name: String,

    pub description: Option<String>,
    pub permissions: PermissionSet,
    pub project_id: Option<i64>,
    pub group_id: Option<i64>,
}

/// Role update input
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UpdateRole {
    pub name: Option<String>,
    pub description: Option<String>,
    pub permissions: Option<PermissionSet>,
}

/// Role query params
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RoleQuery {
    pub project_id: Option<i64>,
    pub group_id: Option<i64>,
    pub is_builtin: Option<bool>,
    pub sort: Option<String>,
    pub order: Option<SortOrder>,
    pub page: u32,
    pub per_page: u32,
}

impl Role {
    pub fn has_permission(&self, permission: PermissionSet) -> bool {
        self.permissions.contains(permission)
    }

    pub fn is_global(&self) -> bool {
        self.project_id.is_none() && self.group_id.is_none()
    }

    pub fn is_project_role(&self) -> bool {
        self.project_id.is_some()
    }

    pub fn is_group_role(&self) -> bool {
        self.group_id.is_some()
    }
}

/// Built-in roles with default permissions
impl Role {
    pub fn guest() -> PermissionSet {
        PermissionSet::VIEW_PROJECT
            | PermissionSet::VIEW_ISSUES
            | PermissionSet::VIEW_MERGE_REQUESTS
            | PermissionSet::VIEW_BUILDS
            | PermissionSet::VIEW_PACKAGES
            | PermissionSet::VIEW_CONTAINER_IMAGES
    }

    pub fn reporter() -> PermissionSet {
        Self::guest()
            | PermissionSet::CREATE_ISSUE
            | PermissionSet::EDIT_ISSUE
            | PermissionSet::CLOSE_ISSUE
            | PermissionSet::REOPEN_ISSUE
            | PermissionSet::MANAGE_ISSUE_LABELS
            | PermissionSet::CREATE_MERGE_REQUEST
            | PermissionSet::TRIGGER_BUILD
            | PermissionSet::DOWNLOAD_ARTIFACTS
    }

    pub fn developer() -> PermissionSet {
        Self::reporter()
            | PermissionSet::READ_CODE
            | PermissionSet::WRITE_CODE
            | PermissionSet::CREATE_MERGE_REQUEST
            | PermissionSet::MERGE_MERGE_REQUEST
            | PermissionSet::APPROVE_MERGE_REQUEST
            | PermissionSet::TRIGGER_BUILD
            | PermissionSet::RETRY_BUILD
            | PermissionSet::VIEW_BUILD_LOGS
            | PermissionSet::DOWNLOAD_ARTIFACTS
            | PermissionSet::UPLOAD_PACKAGE
            | PermissionSet::PUSH_CONTAINER_IMAGE
            | PermissionSet::CREATE_WORKSPACE
            | PermissionSet::VIEW_WORKSPACE
    }

    pub fn maintainer() -> PermissionSet {
        Self::developer()
            | PermissionSet::MANAGE_PROJECT
            | PermissionSet::MANAGE_MEMBERS
            | PermissionSet::MANAGE_HOOKS
            | PermissionSet::MANAGE_DEPLOY_KEYS
            | PermissionSet::MANAGE_PROTECTED_BRANCHES
            | PermissionSet::MANAGE_PROTECTED_TAGS
            | PermissionSet::MANAGE_PAGES
            | PermissionSet::MANAGE_WIKI
            | PermissionSet::MANAGE_SNIPPETS
            | PermissionSet::DELETE_ISSUE
            | PermissionSet::MANAGE_ISSUE_BOARD
            | PermissionSet::SET_ISSUE_CONFIDENTIAL
            | PermissionSet::VIEW_CONFIDENTIAL_ISSUES
            | PermissionSet::CLOSE_MERGE_REQUEST
            | PermissionSet::REOPEN_MERGE_REQUEST
            | PermissionSet::MANAGE_MERGE_REQUEST_REVIEWERS
            | PermissionSet::CANCEL_BUILD
            | PermissionSet::ERASE_BUILD
            | PermissionSet::MANAGE_BUILD_VARIABLES
            | PermissionSet::MANAGE_BUILD_SCHEDULES
            | PermissionSet::MANAGE_RUNNERS
            | PermissionSet::DELETE_PACKAGE
            | PermissionSet::DELETE_CONTAINER_IMAGE
            | PermissionSet::MANAGE_WORKSPACE
            | PermissionSet::DELETE_WORKSPACE
            | PermissionSet::MANAGE_CODE_REVIEWS
    }

    pub fn owner() -> PermissionSet {
        Self::maintainer()
            | PermissionSet::ADMIN_PROJECT
            | PermissionSet::DELETE_PROJECT
            | PermissionSet::RENAME_PROJECT
            | PermissionSet::TRANSFER_PROJECT
    }

    pub fn admin() -> PermissionSet {
        PermissionSet::all()
    }
}