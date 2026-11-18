//! User Domain Model

use super::*;

/// User types in the system
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Hash,
    Serialize,
    Deserialize,
    strum::EnumString,
    strum::Display,
)]
#[serde(rename_all = "lowercase")]
#[strum(serialize_all = "lowercase")]
pub enum UserType {
    /// Regular human user
    Ordinary,
    /// Service account for automation
    Service,
    /// AI assistant user
    Ai,
}

/// User status
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, strum::EnumString, strum::Display,
)]
#[serde(rename_all = "lowercase")]
#[strum(serialize_all = "lowercase")]
pub enum UserStatus {
    Active,
    Disabled,
    PendingVerification,
    Locked,
}

/// Special system user IDs
pub const ROOT_USER_ID: i64 = 1;
pub const SYSTEM_USER_ID: i64 = -1;
pub const UNKNOWN_USER_ID: i64 = -2;
pub const SYSTEM_USER_NAME: &str = "OneDev";
pub const UNKNOWN_USER_NAME: &str = "unknown";

/// Core User entity
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct User {
    pub id: i64,
    pub uuid: Uuid,
    pub name: String,
    pub full_name: Option<String>,
    pub email: Option<String>,
    pub password_hash: Option<String>, // Argon2 hash, never exposed in API
    pub user_type: UserType,
    pub status: UserStatus,
    pub avatar_url: Option<String>,
    pub two_factor_enabled: bool,
    pub two_factor_secret: Option<String>, // Encrypted TOTP secret
    pub backup_codes: Vec<String>,         // Hashed backup codes
    pub last_login_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
    // Settings
    pub notify_own_events: bool,
    pub keep_emails_private: bool,
    pub timezone: String,
    pub locale: String,
}

/// User principal for authentication
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct UserPrincipal {
    pub user_id: i64,
    pub username: String,
    pub user_type: UserType,
}

/// Lightweight user reference for lists
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct UserRef {
    pub id: i64,
    pub uuid: Uuid,
    pub name: String,
    pub full_name: Option<String>,
    pub avatar_url: Option<String>,
    pub user_type: UserType,
}

impl From<&User> for UserRef {
    fn from(user: &User) -> Self {
        Self {
            id: user.id,
            uuid: user.uuid,
            name: user.name.clone(),
            full_name: user.full_name.clone(),
            avatar_url: user.avatar_url.clone(),
            user_type: user.user_type,
        }
    }
}

/// User creation input
#[derive(Debug, Clone, Serialize, Deserialize, validator::Validate)]
pub struct CreateUser {
    #[validate(length(min = 3, max = 100))]
    #[validate(regex(path = *crate::utils::validation::USERNAME_REGEX))]
    pub name: String,

    #[validate(length(max = 200))]
    pub full_name: Option<String>,

    #[validate(email)]
    pub email: Option<String>,

    #[validate(length(min = 8, max = 128))]
    pub password: Option<String>, // Optional for service accounts / SSO

    pub user_type: UserType,
    pub send_invite_email: bool,
}

/// User update input
#[derive(Debug, Clone, Serialize, Deserialize, validator::Validate, Default)]
pub struct UpdateUser {
    #[validate(length(max = 200))]
    pub full_name: Option<String>,

    #[validate(email)]
    pub email: Option<String>,

    pub avatar_url: Option<String>,
    pub notify_own_events: Option<bool>,
    pub keep_emails_private: Option<bool>,
    pub timezone: Option<String>,
    pub locale: Option<String>,
}

/// Password change input
#[derive(Debug, Clone, Serialize, Deserialize, validator::Validate)]
pub struct ChangePassword {
    #[validate(length(min = 8, max = 128))]
    pub current_password: String,

    #[validate(length(min = 8, max = 128))]
    pub new_password: String,
}

/// Password reset request
#[derive(Debug, Clone, Serialize, Deserialize, validator::Validate)]
pub struct RequestPasswordReset {
    #[validate(email)]
    pub email: String,
}

/// Password reset confirmation
#[derive(Debug, Clone, Serialize, Deserialize, validator::Validate)]
pub struct ConfirmPasswordReset {
    pub token: String,
    #[validate(length(min = 8, max = 128))]
    pub new_password: String,
}

/// 2FA setup response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TwoFactorSetup {
    pub secret: String,
    pub qr_code_url: String,
    pub backup_codes: Vec<String>,
}

/// 2FA verification
#[derive(Debug, Clone, Serialize, Deserialize, validator::Validate)]
pub struct VerifyTwoFactor {
    #[validate(length(equal = 6))]
    pub code: String,
}

/// User query params
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UserQuery {
    pub term: Option<String>,
    pub user_type: Option<UserType>,
    pub status: Option<UserStatus>,
    pub page: u32,
    pub per_page: u32,
    pub sort: Option<String>,
    pub order: Option<SortOrder>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum SortOrder {
    #[default]
    Asc,
    Desc,
}

impl User {
    pub fn is_root(&self) -> bool {
        self.id == ROOT_USER_ID
    }

    pub fn is_system(&self) -> bool {
        self.id == SYSTEM_USER_ID
    }

    pub fn is_unknown(&self) -> bool {
        self.id == UNKNOWN_USER_ID
    }

    pub fn is_special(&self) -> bool {
        self.is_root() || self.is_system() || self.is_unknown()
    }

    pub fn can_login(&self) -> bool {
        matches!(self.status, UserStatus::Active) && self.password_hash.is_some()
    }

    pub fn display_name(&self) -> String {
        self.full_name.clone().unwrap_or_else(|| self.name.clone())
    }

    pub fn to_principal(&self) -> UserPrincipal {
        UserPrincipal {
            user_id: self.id,
            username: self.name.clone(),
            user_type: self.user_type,
        }
    }
}
