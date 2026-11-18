//! Settings Domain Model

use super::*;

/// System-wide settings
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct SystemSetting {
    // General
    pub site_name: String,
    pub site_description: Option<String>,
    pub site_url: String,
    pub support_email: String,
    pub admin_email: String,
    pub no_reply_email: String,
    // Authentication
    pub signup_enabled: bool,
    pub login_enabled: bool,
    pub ldap_enabled: bool,
    pub oauth_enabled: bool,
    pub saml_enabled: bool,
    pub two_factor_required: bool,
    pub password_min_length: u32,
    pub password_max_length: u32,
    pub password_require_uppercase: bool,
    pub password_require_lowercase: bool,
    pub password_require_digit: bool,
    pub password_require_symbol: bool,
    pub session_timeout: u32,     // minutes
    pub remember_me_timeout: u32, // days
    pub max_failed_login_attempts: u32,
    pub lockout_duration: u32, // minutes
    // Email
    pub smtp_enabled: bool,
    pub smtp_host: Option<String>,
    pub smtp_port: Option<u16>,
    pub smtp_username: Option<String>,
    pub smtp_password: Option<String>, // Encrypted
    pub smtp_auth: Option<String>,
    pub smtp_starttls: bool,
    pub smtp_tls: bool,
    pub email_from: String,
    pub email_reply_to: Option<String>,
    // Git
    pub git_http_enabled: bool,
    pub git_ssh_enabled: bool,
    pub git_ssh_host: Option<String>,
    pub git_ssh_port: Option<u16>,
    pub max_file_size: u64, // bytes
    pub max_repo_size: u64, // bytes
    pub repo_path: String,
    pub lfs_enabled: bool,
    pub lfs_max_file_size: u64,
    // CI/CD
    pub builds_enabled: bool,
    pub max_build_timeout: u32, // minutes
    pub max_concurrent_builds: u32,
    pub default_build_image: String,
    pub shared_runners_enabled: bool,
    pub shared_runners_minutes: u64,
    pub artifacts_enabled: bool,
    pub artifacts_max_size: u64,
    pub artifacts_expire_in: String,
    pub cache_enabled: bool,
    pub cache_max_size: u64,
    // Packages
    pub packages_enabled: bool,
    pub packages_max_file_size: u64,
    // Container Registry
    pub container_registry_enabled: bool,
    pub container_registry_max_size: u64,
    // Pages
    pub pages_enabled: bool,
    pub pages_max_size: u64,
    pub pages_domain: Option<String>,
    // Security
    pub enforce_https: bool,
    pub hsts_enabled: bool,
    pub hsts_max_age: u32,
    pub content_security_policy: Option<String>,
    pub cors_allowed_origins: Vec<String>,
    pub rate_limit_enabled: bool,
    pub rate_limit_requests_per_minute: u32,
    // Monitoring
    pub metrics_enabled: bool,
    pub tracing_enabled: bool,
    pub tracing_endpoint: Option<String>,
    pub sentry_dsn: Option<String>,
    // Features
    pub wiki_enabled: bool,
    pub snippets_enabled: bool,
    pub workspaces_enabled: bool,
    pub merge_trains_enabled: bool,
    pub code_owners_enabled: bool,
    pub protected_branches_enabled: bool,
    pub protected_tags_enabled: bool,
    pub push_rules_enabled: bool,
    // Housekeeping
    pub job_artifacts_expire_in: String,
    pub pipeline_artifacts_expire_in: String,
    pub build_logs_expire_in: String,
    pub cleanup_inactive_projects: bool,
    pub inactive_project_timeout: u32, // days
    // Limits
    pub max_projects_per_user: u32,
    pub max_groups_per_user: u32,
    pub max_members_per_project: u32,
    pub max_members_per_group: u32,
    pub max_webhooks_per_project: u32,
    pub max_deploy_keys_per_project: u32,
    pub max_protected_branches_per_project: u32,
    pub max_protected_tags_per_project: u32,
    // Timestamps
    pub updated_at: DateTime<Utc>,
    pub updated_by_id: i64,
}

/// Security settings
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct SecuritySetting {
    pub enable_anonymous_access: bool,
    pub enable_internal_login: bool,
    pub enable_ldap: bool,
    pub enable_oauth: bool,
    pub enable_saml: bool,
    pub enable_kerberos: bool,
    pub enable_openid: bool,
    pub two_factor_required_for_admin: bool,
    pub two_factor_required_for_all: bool,
    pub password_min_length: u32,
    pub password_max_age: u32,    // days, 0 = never expire
    pub password_history: u32,    // number of previous passwords to remember
    pub session_timeout: u32,     // minutes
    pub remember_me_timeout: u32, // days
    pub max_failed_login_attempts: u32,
    pub lockout_duration: u32, // minutes
    pub ip_whitelist: Vec<String>,
    pub ip_blacklist: Vec<String>,
    pub allowed_domains: Vec<String>,
    pub blocked_domains: Vec<String>,
    pub require_approved_email_domain: bool,
    pub allowed_email_domains: Vec<String>,
    pub blocked_email_domains: Vec<String>,
}

/// Email settings
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct EmailSetting {
    pub smtp_enabled: bool,
    pub smtp_host: String,
    pub smtp_port: u16,
    pub smtp_username: String,
    pub smtp_password: String, // Encrypted
    pub smtp_auth: String,
    pub smtp_starttls: bool,
    pub smtp_tls: bool,
    pub email_from: String,
    pub email_reply_to: Option<String>,
    pub email_cc: Vec<String>,
    pub email_bcc: Vec<String>,
    pub test_recipient: Option<String>,
}

/// Notification settings
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct NotificationSetting {
    pub email_notifications_enabled: bool,
    pub web_notifications_enabled: bool,
    pub slack_enabled: bool,
    pub slack_webhook_url: Option<String>,
    pub discord_enabled: bool,
    pub discord_webhook_url: Option<String>,
    pub mattermost_enabled: bool,
    pub mattermost_webhook_url: Option<String>,
    pub teams_enabled: bool,
    pub teams_webhook_url: Option<String>,
    pub pagerduty_enabled: bool,
    pub pagerduty_key: Option<String>,
    pub opsgenie_enabled: bool,
    pub opsgenie_key: Option<String>,
    pub victorops_enabled: bool,
    pub victorops_key: Option<String>,
    pub webhook_enabled: bool,
    pub webhook_url: Option<String>,
    pub webhook_secret: Option<String>,
}

/// User preferences
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct UserPreferences {
    pub theme: Theme,
    pub timezone: String,
    pub locale: String,
    pub date_format: String,
    pub time_format: String,
    pub sidebar_collapsed: bool,
    pub syntax_highlighting_theme: String,
    pub editor_font_size: u32,
    pub editor_tab_size: u32,
    pub show_whitespace: bool,
    pub wrap_lines: bool,
    pub show_line_numbers: bool,
    pub auto_refresh: bool,
    pub refresh_interval: u32, // seconds
    pub email_notifications: EmailNotificationPrefs,
    pub web_notifications: WebNotificationPrefs,
}

#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Serialize,
    Deserialize,
    Default,
    strum::EnumString,
    strum::Display,
)]
#[serde(rename_all = "lowercase")]
#[strum(serialize_all = "lowercase")]
pub enum Theme {
    #[default]
    Light,
    Dark,
    Auto,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct EmailNotificationPrefs {
    pub issue_created: bool,
    pub issue_assigned: bool,
    pub issue_mentioned: bool,
    pub issue_commented: bool,
    pub issue_closed: bool,
    pub merge_request_created: bool,
    pub merge_request_assigned: bool,
    pub merge_request_mentioned: bool,
    pub merge_request_commented: bool,
    pub merge_request_merged: bool,
    pub merge_request_closed: bool,
    pub build_failed: bool,
    pub build_success: bool,
    pub build_canceled: bool,
    pub pipeline_failed: bool,
    pub pipeline_success: bool,
    pub mention: bool,
    pub todo: bool,
    pub milestone: bool,
    pub release: bool,
    pub security: bool,
    pub daily_summary: bool,
    pub weekly_summary: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct WebNotificationPrefs {
    pub issue_created: bool,
    pub issue_assigned: bool,
    pub issue_mentioned: bool,
    pub issue_commented: bool,
    pub issue_closed: bool,
    pub merge_request_created: bool,
    pub merge_request_assigned: bool,
    pub merge_request_mentioned: bool,
    pub merge_request_commented: bool,
    pub merge_request_merged: bool,
    pub merge_request_closed: bool,
    pub build_failed: bool,
    pub build_success: bool,
    pub build_canceled: bool,
    pub pipeline_failed: bool,
    pub pipeline_success: bool,
    pub mention: bool,
    pub todo: bool,
    pub milestone: bool,
    pub release: bool,
    pub security: bool,
}

/// Settings key for storage
#[derive(
    Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, strum::EnumString, strum::Display,
)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case")]
pub enum SettingKey {
    System,
    Security,
    Email,
    Notification,
    UserPreferences,
    Ldap,
    OAuth,
    Saml,
    Kerberos,
    OpenId,
    Custom(String),
}

impl SettingKey {
    pub fn as_str(&self) -> &str {
        match self {
            SettingKey::System => "system",
            SettingKey::Security => "security",
            SettingKey::Email => "email",
            SettingKey::Notification => "notification",
            SettingKey::UserPreferences => "user_preferences",
            SettingKey::Ldap => "ldap",
            SettingKey::OAuth => "oauth",
            SettingKey::Saml => "saml",
            SettingKey::Kerberos => "kerberos",
            SettingKey::OpenId => "openid",
            SettingKey::Custom(s) => s,
        }
    }
}

/// Settings value wrapper
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SettingValue {
    pub key: SettingKey,
    pub value: serde_json::Value,
    pub description: Option<String>,
    pub is_secret: bool,
    pub updated_at: DateTime<Utc>,
    pub updated_by_id: i64,
}

impl SettingValue {
    pub fn new<T: Serialize>(key: SettingKey, value: &T) -> Result<Self, serde_json::Error> {
        Ok(Self {
            key,
            value: serde_json::to_value(value)?,
            description: None,
            is_secret: false,
            updated_at: Utc::now(),
            updated_by_id: 0,
        })
    }

    pub fn new_secret<T: Serialize>(key: SettingKey, value: &T) -> Result<Self, serde_json::Error> {
        let mut s = Self::new(key, value)?;
        s.is_secret = true;
        Ok(s)
    }

    pub fn get<T: for<'de> Deserialize<'de>>(&self) -> Result<T, serde_json::Error> {
        serde_json::from_value(self.value.clone())
    }
}
