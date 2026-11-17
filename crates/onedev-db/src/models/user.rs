//! User Database Model

use super::*;
use crate::DbResult;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, Postgres, Row};
use uuid::Uuid;

/// User type in the database
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "user_type", rename_all = "lowercase")]
pub enum DbUserType {
    Ordinary,
    Service,
    Ai,
}

impl From<DbUserType> for onedev_core::models::user::UserType {
    fn from(t: DbUserType) -> Self {
        match t {
            DbUserType::Ordinary => onedev_core::models::user::UserType::Ordinary,
            DbUserType::Service => onedev_core::models::user::UserType::Service,
            DbUserType::Ai => onedev_core::models::user::UserType::Ai,
        }
    }
}

impl From<onedev_core::models::user::UserType> for DbUserType {
    fn from(t: onedev_core::models::user::UserType) -> Self {
        match t {
            onedev_core::models::user::UserType::Ordinary => DbUserType::Ordinary,
            onedev_core::models::user::UserType::Service => DbUserType::Service,
            onedev_core::models::user::UserType::Ai => DbUserType::Ai,
        }
    }
}

/// User status in the database
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "user_status", rename_all = "snake_case")] // 'pending_verification'
pub enum DbUserStatus {
    Active,
    Disabled,
    PendingVerification,
    Locked,
}

impl From<DbUserStatus> for onedev_core::models::user::UserStatus {
    fn from(s: DbUserStatus) -> Self {
        match s {
            DbUserStatus::Active => onedev_core::models::user::UserStatus::Active,
            DbUserStatus::Disabled => onedev_core::models::user::UserStatus::Disabled,
            DbUserStatus::PendingVerification => onedev_core::models::user::UserStatus::PendingVerification,
            DbUserStatus::Locked => onedev_core::models::user::UserStatus::Locked,
        }
    }
}

impl From<onedev_core::models::user::UserStatus> for DbUserStatus {
    fn from(s: onedev_core::models::user::UserStatus) -> Self {
        match s {
            onedev_core::models::user::UserStatus::Active => DbUserStatus::Active,
            onedev_core::models::user::UserStatus::Disabled => DbUserStatus::Disabled,
            onedev_core::models::user::UserStatus::PendingVerification => DbUserStatus::PendingVerification,
            onedev_core::models::user::UserStatus::Locked => DbUserStatus::Locked,
        }
    }
}

/// User row from database
#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct DbUser {
    pub id: i64,
    pub uuid: Uuid,
    pub name: String,
    pub full_name: Option<String>,
    pub email: Option<String>,
    pub password_hash: Option<String>,
    pub user_type: DbUserType,
    pub status: DbUserStatus,
    pub avatar_url: Option<String>,
    pub two_factor_enabled: bool,
    pub two_factor_secret: Option<String>, // Encrypted
    #[sqlx(json)] // JSONB column holding an array of hashed backup codes
    pub backup_codes: Vec<String>,
    pub last_login_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
    // Settings
    pub notify_own_events: bool,
    pub keep_emails_private: bool,
    pub timezone: String,
    pub locale: String,
    // AI settings (stored as JSON)
    pub ai_setting: Option<serde_json::Value>,
    // 2FA settings (stored as JSON)
    pub two_factor_auth: Option<serde_json::Value>,
}

/// User creation input for database
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DbCreateUser {
    pub name: String,
    pub full_name: Option<String>,
    pub email: Option<String>,
    pub password_hash: Option<String>,
    pub user_type: DbUserType,
    pub avatar_url: Option<String>,
    pub notify_own_events: bool,
    pub keep_emails_private: bool,
    pub timezone: String,
    pub locale: String,
}

/// User update input for database
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DbUpdateUser {
    pub full_name: Option<String>,
    pub email: Option<String>,
    pub avatar_url: Option<String>,
    pub user_type: Option<DbUserType>,
    pub status: Option<DbUserStatus>,
    pub password_hash: Option<String>,
    pub two_factor_enabled: Option<bool>,
    pub two_factor_secret: Option<String>,
    pub backup_codes: Option<Vec<String>>,
    pub last_login_at: Option<DateTime<Utc>>,
    pub notify_own_events: Option<bool>,
    pub keep_emails_private: Option<bool>,
    pub timezone: Option<String>,
    pub locale: Option<String>,
    pub ai_setting: Option<serde_json::Value>,
    pub two_factor_auth: Option<serde_json::Value>,
}

impl DbUser {
    /// Convert to core domain model
    pub fn to_core(&self) -> onedev_core::models::user::User {
        onedev_core::models::user::User {
            id: self.id,
            uuid: self.uuid,
            name: self.name.clone(),
            full_name: self.full_name.clone(),
            email: self.email.clone(),
            password_hash: self.password_hash.clone(),
            user_type: self.user_type.into(),
            status: self.status.into(),
            avatar_url: self.avatar_url.clone(),
            two_factor_enabled: self.two_factor_enabled,
            two_factor_secret: self.two_factor_secret.clone(),
            backup_codes: self.backup_codes.clone(),
            last_login_at: self.last_login_at,
            created_at: self.created_at,
            updated_at: self.updated_at,
            deleted_at: self.deleted_at,
            notify_own_events: self.notify_own_events,
            keep_emails_private: self.keep_emails_private,
            timezone: self.timezone.clone(),
            locale: self.locale.clone(),
        }
    }

    /// Check if user is root
    pub fn is_root(&self) -> bool {
        self.id == 1
    }

    /// Check if user is system
    pub fn is_system(&self) -> bool {
        self.id == -1
    }

    /// Check if user is unknown
    pub fn is_unknown(&self) -> bool {
        self.id == -2
    }

    /// Check if user can login
    pub fn can_login(&self) -> bool {
        matches!(self.status, DbUserStatus::Active) && self.password_hash.is_some()
    }

    /// Get display name
    pub fn display_name(&self) -> String {
        self.full_name.clone().unwrap_or_else(|| self.name.clone())
    }
}

impl DbCreateUser {
    /// Create from core CreateUser
    pub fn from_core(create: &onedev_core::models::user::CreateUser, password_hash: Option<String>) -> Self {
        Self {
            name: create.name.clone(),
            full_name: create.full_name.clone(),
            email: create.email.clone(),
            password_hash,
            user_type: create.user_type.into(),
            avatar_url: None,
            notify_own_events: true,
            keep_emails_private: false,
            timezone: "UTC".to_string(),
            locale: "en".to_string(),
        }
    }
}

/// Database query builder for users
pub struct UserQueryBuilder;

impl UserQueryBuilder {
    pub fn select_all() -> &'static str {
        r#"
        SELECT id, uuid, name, full_name, email, password_hash, user_type, status,
               avatar_url, two_factor_enabled, two_factor_secret, backup_codes,
               last_login_at, created_at, updated_at, deleted_at,
               notify_own_events, keep_emails_private, timezone, locale,
               ai_setting, two_factor_auth
        FROM users
        "#
    }

    pub fn select_by_id() -> &'static str {
        r#"
        SELECT id, uuid, name, full_name, email, password_hash, user_type, status,
               avatar_url, two_factor_enabled, two_factor_secret, backup_codes,
               last_login_at, created_at, updated_at, deleted_at,
               notify_own_events, keep_emails_private, timezone, locale,
               ai_setting, two_factor_auth
        FROM users WHERE id = $1
        "#
    }

    pub fn select_by_name() -> &'static str {
        r#"
        SELECT id, uuid, name, full_name, email, password_hash, user_type, status,
               avatar_url, two_factor_enabled, two_factor_secret, backup_codes,
               last_login_at, created_at, updated_at, deleted_at,
               notify_own_events, keep_emails_private, timezone, locale,
               ai_setting, two_factor_auth
        FROM users WHERE name = $1
        "#
    }

    pub fn select_by_email() -> &'static str {
        r#"
        SELECT id, uuid, name, full_name, email, password_hash, user_type, status,
               avatar_url, two_factor_enabled, two_factor_secret, backup_codes,
               last_login_at, created_at, updated_at, deleted_at,
               notify_own_events, keep_emails_private, timezone, locale,
               ai_setting, two_factor_auth
        FROM users WHERE email = $1
        "#
    }

    pub fn select_by_uuid() -> &'static str {
        r#"
        SELECT id, uuid, name, full_name, email, password_hash, user_type, status,
               avatar_url, two_factor_enabled, two_factor_secret, backup_codes,
               last_login_at, created_at, updated_at, deleted_at,
               notify_own_events, keep_emails_private, timezone, locale,
               ai_setting, two_factor_auth
        FROM users WHERE uuid = $1
        "#
    }

    pub fn insert() -> &'static str {
        r#"
        INSERT INTO users (uuid, name, full_name, email, password_hash, user_type, status,
                           avatar_url, two_factor_enabled, two_factor_secret, backup_codes,
                           created_at, updated_at, notify_own_events, keep_emails_private,
                           timezone, locale, ai_setting, two_factor_auth)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18, $19)
        RETURNING id
        "#
    }

    pub fn update() -> &'static str {
        r#"
        UPDATE users SET
            full_name = COALESCE($2, full_name),
            email = COALESCE($3, email),
            avatar_url = COALESCE($4, avatar_url),
            user_type = COALESCE($5, user_type),
            status = COALESCE($6, status),
            password_hash = COALESCE($7, password_hash),
            two_factor_enabled = COALESCE($8, two_factor_enabled),
            two_factor_secret = COALESCE($9, two_factor_secret),
            backup_codes = COALESCE($10, backup_codes),
            last_login_at = COALESCE($11, last_login_at),
            notify_own_events = COALESCE($12, notify_own_events),
            keep_emails_private = COALESCE($13, keep_emails_private),
            timezone = COALESCE($14, timezone),
            locale = COALESCE($15, locale),
            ai_setting = COALESCE($16, ai_setting),
            two_factor_auth = COALESCE($17, two_factor_auth),
            updated_at = NOW()
        WHERE id = $1
        RETURNING id, uuid, name, full_name, email, password_hash, user_type, status,
                  avatar_url, two_factor_enabled, two_factor_secret, backup_codes,
                  last_login_at, created_at, updated_at, deleted_at,
                  notify_own_events, keep_emails_private, timezone, locale,
                  ai_setting, two_factor_auth
        "#
    }

    pub fn delete() -> &'static str {
        "UPDATE users SET deleted_at = NOW(), updated_at = NOW() WHERE id = $1"
    }

    pub fn hard_delete() -> &'static str {
        "DELETE FROM users WHERE id = $1"
    }

    pub fn list_with_pagination() -> &'static str {
        r#"
        SELECT id, uuid, name, full_name, email, password_hash, user_type, status,
               avatar_url, two_factor_enabled, two_factor_secret, backup_codes,
               last_login_at, created_at, updated_at, deleted_at,
               notify_own_events, keep_emails_private, timezone, locale,
               ai_setting, two_factor_auth
        FROM users
        WHERE deleted_at IS NULL
        ORDER BY created_at DESC
        LIMIT $1 OFFSET $2
        "#
    }

    pub fn count() -> &'static str {
        "SELECT COUNT(*) FROM users WHERE deleted_at IS NULL"
    }
}