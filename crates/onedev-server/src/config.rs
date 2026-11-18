//! Configuration for OneDev Server

use figment::{Figment, providers::{Env, Format, Toml, Json, Yaml}};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Application configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    /// Server configuration
    pub server: ServerConfig,
    /// Database configuration
    pub database: DatabaseConfig,
    /// Authentication configuration
    pub auth: AuthConfig,
    /// Build configuration
    pub build: BuildConfig,
    /// Git configuration
    pub git: GitConfig,
    /// Webhook configuration
    pub webhook: WebhookConfig,
    /// Cleanup configuration
    pub cleanup: CleanupConfig,
    /// Search configuration
    pub search: SearchConfig,
    /// Email configuration
    pub email: EmailConfig,
    /// Storage configuration
    pub storage: StorageConfig,
    /// Observability configuration
    pub observability: ObservabilityConfig,
    /// Security configuration
    pub security: SecurityConfig,
    /// Feature flags
    pub features: FeatureFlags,
}

/// Server configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    /// Bind address
    #[serde(default = "default_bind_address")]
    pub bind_address: String,
    /// Port
    #[serde(default = "default_port")]
    pub port: u16,
    /// Maximum concurrent requests
    #[serde(default = "default_max_concurrent_requests")]
    pub max_concurrent_requests: usize,
    /// Request timeout in seconds
    #[serde(default = "default_request_timeout_secs")]
    pub request_timeout_secs: u64,
    /// Enable TLS
    #[serde(default)]
    pub tls_enabled: bool,
    /// TLS cert path
    pub tls_cert_path: Option<PathBuf>,
    /// TLS key path
    pub tls_key_path: Option<PathBuf>,
    /// Environment (development, staging, production)
    #[serde(default = "default_environment")]
    pub environment: String,
    /// Sentry DSN
    pub sentry_dsn: Option<String>,
    /// OpenTelemetry enabled
    #[serde(default)]
    pub otel_enabled: bool,
    /// OTLP endpoint
    pub otlp_endpoint: Option<String>,
}

fn default_bind_address() -> String {
    "0.0.0.0".to_string()
}

fn default_port() -> u16 {
    46315
}

fn default_max_concurrent_requests() -> usize {
    1000
}

fn default_request_timeout_secs() -> u64 {
    60
}

fn default_environment() -> String {
    "development".to_string()
}

/// Database configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseConfig {
    /// Database URL
    pub url: String,
    /// Maximum connections in pool
    #[serde(default = "default_max_connections")]
    pub max_connections: u32,
    /// Minimum connections in pool
    #[serde(default = "default_min_connections")]
    pub min_connections: u32,
    /// Connection timeout in seconds
    #[serde(default = "default_connect_timeout")]
    pub connect_timeout_secs: u64,
    /// Idle timeout in seconds
    #[serde(default = "default_idle_timeout")]
    pub idle_timeout_secs: u64,
    /// Max lifetime in seconds
    #[serde(default = "default_max_lifetime")]
    pub max_lifetime_secs: u64,
    /// Enable SQLx query logging
    #[serde(default)]
    pub sqlx_logging: bool,
}

fn default_max_connections() -> u32 {
    20
}

fn default_min_connections() -> u32 {
    5
}

fn default_connect_timeout() -> u64 {
    30
}

fn default_idle_timeout() -> u64 {
    600
}

fn default_max_lifetime() -> u64 {
    1800
}

/// Authentication configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthConfig {
    /// JWT secret key (base64 encoded)
    pub jwt_secret: String,
    /// Token expiry in hours
    #[serde(default = "default_token_expiry_hours")]
    pub token_expiry_hours: i64,
    /// Refresh token expiry in days
    #[serde(default = "default_refresh_expiry_days")]
    pub refresh_expiry_days: i64,
    /// Argon2 memory cost (KB)
    #[serde(default = "default_argon2_memory")]
    pub argon2_memory_kb: u32,
    /// Argon2 iterations
    #[serde(default = "default_argon2_iterations")]
    pub argon2_iterations: u32,
    /// Argon2 parallelism
    #[serde(default = "default_argon2_parallelism")]
    pub argon2_parallelism: u32,
    /// Enable 2FA
    #[serde(default = "default_true")]
    pub enable_2fa: bool,
    /// 2FA issuer name
    #[serde(default = "default_2fa_issuer")]
    pub two_fa_issuer: String,
    /// Password min length
    #[serde(default = "default_password_min_length")]
    pub password_min_length: usize,
    /// Password require uppercase
    #[serde(default = "default_true")]
    pub password_require_uppercase: bool,
    /// Password require lowercase
    #[serde(default = "default_true")]
    pub password_require_lowercase: bool,
    /// Password require digits
    #[serde(default = "default_true")]
    pub password_require_digits: bool,
    /// Password require special chars
    #[serde(default = "default_true")]
    pub password_require_special: bool,
    /// Session timeout in minutes
    #[serde(default = "default_session_timeout_minutes")]
    pub session_timeout_minutes: i64,
    /// Max failed login attempts before lockout
    #[serde(default = "default_max_failed_attempts")]
    pub max_failed_attempts: u32,
    /// Lockout duration in minutes
    #[serde(default = "default_lockout_duration_minutes")]
    pub lockout_duration_minutes: i64,
    /// OAuth providers
    pub oauth: OAuthConfig,
}

fn default_token_expiry_hours() -> i64 {
    24
}

fn default_refresh_expiry_days() -> i64 {
    30
}

fn default_argon2_memory() -> u32 {
    65536
}

fn default_argon2_iterations() -> u32 {
    3
}

fn default_argon2_parallelism() -> u32 {
    4
}

fn default_true() -> bool {
    true
}

fn default_2fa_issuer() -> String {
    "OneDev".to_string()
}

fn default_password_min_length() -> usize {
    12
}

fn default_session_timeout_minutes() -> i64 {
    60
}

fn default_max_failed_attempts() -> u32 {
    5
}

fn default_lockout_duration_minutes() -> i64 {
    15
}

/// OAuth configuration
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct OAuthConfig {
    pub github: Option<OAuthProviderConfig>,
    pub gitlab: Option<OAuthProviderConfig>,
    pub google: Option<OAuthProviderConfig>,
    pub microsoft: Option<OAuthProviderConfig>,
    pub generic: Vec<OAuthProviderConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OAuthProviderConfig {
    pub client_id: String,
    pub client_secret: String,
    pub redirect_url: String,
    pub scopes: Vec<String>,
}

/// Build configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuildConfig {
    /// Default build timeout in minutes
    #[serde(default = "default_build_timeout_minutes")]
    pub default_timeout_minutes: i64,
    /// Max concurrent builds
    #[serde(default = "default_max_concurrent_builds")]
    pub max_concurrent_builds: usize,
    /// Build log retention days
    #[serde(default = "default_build_log_retention_days")]
    pub log_retention_days: i64,
    /// Artifact retention days
    #[serde(default = "default_artifact_retention_days")]
    pub artifact_retention_days: i64,
    /// Default Docker image for builds
    #[serde(default = "default_build_image")]
    pub default_image: String,
    /// Build executor type
    #[serde(default = "default_executor_type")]
    pub executor_type: String,
    /// Shared volume path
    pub shared_volume: Option<PathBuf>,
    /// Enable build cache
    #[serde(default = "default_true")]
    pub enable_cache: bool,
    /// Cache directory
    pub cache_dir: Option<PathBuf>,
    /// Resource limits
    pub resources: BuildResources,
}

fn default_build_timeout_minutes() -> i64 {
    60
}

fn default_max_concurrent_builds() -> usize {
    4
}

fn default_build_log_retention_days() -> i64 {
    30
}

fn default_artifact_retention_days() -> i64 {
    90
}

fn default_build_image() -> String {
    "docker:latest".to_string()
}

fn default_executor_type() -> String {
    "docker".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuildResources {
    /// CPU limit (cores)
    #[serde(default = "default_cpu_limit")]
    pub cpu_limit: f32,
    /// Memory limit (MB)
    #[serde(default = "default_memory_limit")]
    pub memory_limit_mb: u64,
    /// Disk limit (GB)
    #[serde(default = "default_disk_limit")]
    pub disk_limit_gb: u64,
    /// Max processes
    #[serde(default = "default_max_processes")]
    pub max_processes: u32,
}

fn default_cpu_limit() -> f32 {
    2.0
}

fn default_memory_limit() -> u64 {
    4096
}

fn default_disk_limit() -> u64 {
    10
}

fn default_max_processes() -> u32 {
    256
}

/// Git configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitConfig {
    /// Repositories root directory
    pub repos_root: PathBuf,
    /// Git HTTP backend
    #[serde(default = "default_git_http_backend")]
    pub http_backend: String,
    /// Max pack size (MB)
    #[serde(default = "default_max_pack_size")]
    pub max_pack_size_mb: u64,
    /// Receive pack timeout (seconds)
    #[serde(default = "default_receive_timeout")]
    pub receive_timeout_secs: u64,
    /// Enable git LFS
    #[serde(default = "default_true")]
    pub enable_lfs: bool,
    /// LFS storage path
    pub lfs_storage_path: Option<PathBuf>,
    /// Enable git hooks
    #[serde(default = "default_true")]
    pub enable_hooks: bool,
    /// Hook timeout (seconds)
    #[serde(default = "default_hook_timeout")]
    pub hook_timeout_secs: u64,
    /// GC enabled
    #[serde(default = "default_true")]
    pub gc_enabled: bool,
    /// GC interval (hours)
    #[serde(default = "default_gc_interval")]
    pub gc_interval_hours: u64,
}

fn default_git_http_backend() -> String {
    "smart-http".to_string()
}

fn default_max_pack_size() -> u64 {
    100
}

fn default_receive_timeout() -> u64 {
    300
}

fn default_hook_timeout() -> u64 {
    60
}

fn default_gc_interval() -> u64 {
    24
}

/// Webhook configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebhookConfig {
    /// Max retries
    #[serde(default = "default_webhook_max_retries")]
    pub max_retries: u32,
    /// Retry delay base (seconds)
    #[serde(default = "default_webhook_retry_delay")]
    pub retry_delay_secs: u64,
    /// Max retry delay (seconds)
    #[serde(default = "default_webhook_max_retry_delay")]
    pub max_retry_delay_secs: u64,
    /// Timeout (seconds)
    #[serde(default = "default_webhook_timeout")]
    pub timeout_secs: u64,
    /// Enable SSL verification
    #[serde(default = "default_true")]
    pub verify_ssl: bool,
    /// Allowed hosts (empty = all)
    pub allowed_hosts: Vec<String>,
    /// Blocked hosts
    pub blocked_hosts: Vec<String>,
    /// Secret header name
    #[serde(default = "default_webhook_secret_header")]
    pub secret_header: String,
}

/// Cleanup configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CleanupConfig {
    /// Build log retention in days
    #[serde(default = "default_build_log_retention_days")]
    pub build_log_retention_days: i64,
    /// Artifact retention in days
    #[serde(default = "default_artifact_retention_days")]
    pub artifact_retention_days: i64,
    /// Temp file retention in hours
    #[serde(default = "default_temp_file_retention_hours")]
    pub temp_file_retention_hours: i64,
    /// Audit log retention in days
    #[serde(default = "default_audit_log_retention_days")]
    pub audit_log_retention_days: i64,
    /// Webhook log retention in days
    #[serde(default = "default_webhook_log_retention_days")]
    pub webhook_log_retention_days: i64,
    /// Search index cleanup in days
    #[serde(default = "default_search_index_cleanup_days")]
    pub search_index_cleanup_days: i64,
    /// Run interval in hours
    #[serde(default = "default_cleanup_run_interval_hours")]
    pub run_interval_hours: u64,
}

fn default_temp_file_retention_hours() -> i64 {
    24
}

fn default_audit_log_retention_days() -> i64 {
    365
}

fn default_webhook_log_retention_days() -> i64 {
    30
}

fn default_search_index_cleanup_days() -> i64 {
    7
}

fn default_cleanup_run_interval_hours() -> u64 {
    1
}

/// Search configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchConfig {
    /// Batch size for indexing
    #[serde(default = "default_search_batch_size")]
    pub batch_size: usize,
    /// Index interval in seconds
    #[serde(default = "default_search_index_interval_secs")]
    pub index_interval_secs: u64,
    /// Full reindex interval in hours
    #[serde(default = "default_search_full_reindex_interval_hours")]
    pub full_reindex_interval_hours: u64,
    /// Enable code search
    #[serde(default = "default_true")]
    pub enable_code_search: bool,
    /// Enable issue search
    #[serde(default = "default_true")]
    pub enable_issue_search: bool,
    /// Enable wiki search
    #[serde(default = "default_true")]
    pub enable_wiki_search: bool,
}

fn default_search_batch_size() -> usize {
    100
}

fn default_search_index_interval_secs() -> u64 {
    60
}

fn default_search_full_reindex_interval_hours() -> u64 {
    24
}

fn default_webhook_max_retries() -> u32 {
    3
}

fn default_webhook_retry_delay() -> u64 {
    10
}

fn default_webhook_max_retry_delay() -> u64 {
    300
}

fn default_webhook_timeout() -> u64 {
    30
}

fn default_webhook_secret_header() -> String {
    "X-OneDev-Signature".to_string()
}

/// Email configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmailConfig {
    /// SMTP host
    pub smtp_host: String,
    /// SMTP port
    #[serde(default = "default_smtp_port")]
    pub smtp_port: u16,
    /// SMTP username
    pub smtp_username: Option<String>,
    /// SMTP password
    pub smtp_password: Option<String>,
    /// Use STARTTLS
    #[serde(default = "default_true")]
    pub starttls: bool,
    /// Use SSL/TLS
    #[serde(default)]
    pub ssl: bool,
    /// From address
    pub from_address: String,
    /// From name
    #[serde(default = "default_from_name")]
    pub from_name: String,
    /// Enable email
    #[serde(default)]
    pub enabled: bool,
    /// Queue directory
    pub queue_dir: Option<PathBuf>,
}

fn default_smtp_port() -> u16 {
    587
}

fn default_from_name() -> String {
    "OneDev".to_string()
}

/// Storage configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageConfig {
    /// Type: local, s3, gcs, azure
    #[serde(default = "default_storage_type")]
    pub storage_type: String,
    /// Local storage path
    pub local_path: Option<PathBuf>,
    /// S3 configuration
    pub s3: Option<S3Config>,
    /// GCS configuration
    pub gcs: Option<GcsConfig>,
    /// Azure configuration
    pub azure: Option<AzureConfig>,
    /// Max file size (MB)
    #[serde(default = "default_max_file_size")]
    pub max_file_size_mb: u64,
    /// Allowed MIME types
    pub allowed_mime_types: Vec<String>,
}

fn default_storage_type() -> String {
    "local".to_string()
}

fn default_max_file_size() -> u64 {
    100
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct S3Config {
    pub bucket: String,
    pub region: String,
    pub endpoint: Option<String>,
    pub access_key: String,
    pub secret_key: String,
    pub force_path_style: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GcsConfig {
    pub bucket: String,
    pub credentials_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AzureConfig {
    pub container: String,
    pub account: String,
    pub key: String,
}

/// Observability configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObservabilityConfig {
    /// Log level
    #[serde(default = "default_log_level")]
    pub log_level: String,
    /// Log format: json, text
    #[serde(default = "default_log_format")]
    pub log_format: String,
    /// Metrics enabled
    #[serde(default = "default_true")]
    pub metrics_enabled: bool,
    /// Metrics port
    #[serde(default = "default_metrics_port")]
    pub metrics_port: u16,
    /// Tracing enabled
    #[serde(default = "default_true")]
    pub tracing_enabled: bool,
    /// Tracing sample rate
    #[serde(default = "default_trace_sample_rate")]
    pub trace_sample_rate: f64,
    /// Profiling enabled
    #[serde(default)]
    pub profiling_enabled: bool,
}

fn default_log_level() -> String {
    "info".to_string()
}

fn default_log_format() -> String {
    "json".to_string()
}

fn default_metrics_port() -> u16 {
    9090
}

fn default_trace_sample_rate() -> f64 {
    0.1
}

/// Security configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityConfig {
    /// Allowed origins for CORS
    pub cors_origins: Vec<String>,
    /// Trusted proxies
    pub trusted_proxies: Vec<String>,
    /// Content Security Policy
    pub csp: Option<String>,
    /// HSTS enabled
    #[serde(default = "default_true")]
    pub hsts_enabled: bool,
    /// HSTS max age (seconds)
    #[serde(default = "default_hsts_max_age")]
    pub hsts_max_age: u64,
    /// Referrer policy
    #[serde(default = "default_referrer_policy")]
    pub referrer_policy: String,
    /// Permissions policy
    pub permissions_policy: Option<String>,
    /// Rate limiting
    pub rate_limit: RateLimitConfig,
}

fn default_hsts_max_age() -> u64 {
    31536000
}

fn default_referrer_policy() -> String {
    "strict-origin-when-cross-origin".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateLimitConfig {
    /// Requests per minute
    #[serde(default = "default_rpm")]
    pub requests_per_minute: u32,
    /// Burst allowance
    #[serde(default = "default_burst")]
    pub burst: u32,
    /// Enable rate limiting
    #[serde(default = "default_true")]
    pub enabled: bool,
}

fn default_rpm() -> u32 {
    1000
}

fn default_burst() -> u32 {
    200
}

/// Feature flags
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeatureFlags {
    /// Enable CI/CD builds
    #[serde(default = "default_true")]
    pub builds: bool,
    /// Enable merge requests
    #[serde(default = "default_true")]
    pub merge_requests: bool,
    /// Enable issues
    #[serde(default = "default_true")]
    pub issues: bool,
    /// Enable wiki
    #[serde(default = "default_true")]
    pub wiki: bool,
    /// Enable packages
    #[serde(default = "default_true")]
    pub packages: bool,
    /// Enable code search
    #[serde(default = "default_true")]
    pub code_search: bool,
    /// Enable code navigation
    #[serde(default = "default_true")]
    pub code_navigation: bool,
    /// Enable webhooks
    #[serde(default = "default_true")]
    pub webhooks: bool,
    /// Enable LDAP
    #[serde(default)]
    pub ldap: bool,
    /// Enable SAML
    #[serde(default)]
    pub saml: bool,
    /// Enable OIDC
    #[serde(default)]
    pub oidc: bool,
    /// Enable 2FA
    #[serde(default = "default_true")]
    pub two_fa: bool,
    /// Enable audit logs
    #[serde(default = "default_true")]
    pub audit_logs: bool,
    /// Enable project templates
    #[serde(default = "default_true")]
    pub project_templates: bool,
}

impl AppConfig {
    /// Load configuration from files and environment
    pub fn load() -> Result<Self, figment::Error> {
        let config_dir = std::env::var("ONEDEV_CONFIG_DIR")
            .unwrap_or_else(|_| "/etc/onedev".to_string());

        Figment::new()
            .merge(Toml::file(format!("{}/config.toml", config_dir)).nested())
            .merge(Yaml::file(format!("{}/config.yaml", config_dir)).nested())
            .merge(Json::file(format!("{}/config.json", config_dir)).nested())
            .merge(Env::prefixed("ONEDEV_").split("__").global())
            .extract()
    }
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            server: ServerConfig::default(),
            database: DatabaseConfig::default(),
            auth: AuthConfig::default(),
            build: BuildConfig::default(),
            git: GitConfig::default(),
            webhook: WebhookConfig::default(),
            cleanup: CleanupConfig::default(),
            search: SearchConfig::default(),
            email: EmailConfig::default(),
            storage: StorageConfig::default(),
            observability: ObservabilityConfig::default(),
            security: SecurityConfig::default(),
            features: FeatureFlags::default(),
        }
    }
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            bind_address: default_bind_address(),
            port: default_port(),
            max_concurrent_requests: default_max_concurrent_requests(),
            request_timeout_secs: default_request_timeout_secs(),
            tls_enabled: false,
            tls_cert_path: None,
            tls_key_path: None,
            environment: default_environment(),
            sentry_dsn: None,
            otel_enabled: false,
            otlp_endpoint: None,
        }
    }
}

impl Default for DatabaseConfig {
    fn default() -> Self {
        Self {
            url: "postgresql://onedev:onedev@localhost:5432/onedev".to_string(),
            max_connections: default_max_connections(),
            min_connections: default_min_connections(),
            connect_timeout_secs: default_connect_timeout(),
            idle_timeout_secs: default_idle_timeout(),
            max_lifetime_secs: default_max_lifetime(),
            sqlx_logging: false,
        }
    }
}

impl Default for AuthConfig {
    fn default() -> Self {
        Self {
            jwt_secret: "change-me-in-production-use-strong-random-secret".to_string(),
            token_expiry_hours: default_token_expiry_hours(),
            refresh_expiry_days: default_refresh_expiry_days(),
            argon2_memory_kb: default_argon2_memory(),
            argon2_iterations: default_argon2_iterations(),
            argon2_parallelism: default_argon2_parallelism(),
            enable_2fa: default_true(),
            two_fa_issuer: default_2fa_issuer(),
            password_min_length: default_password_min_length(),
            password_require_uppercase: default_true(),
            password_require_lowercase: default_true(),
            password_require_digits: default_true(),
            password_require_special: default_true(),
            session_timeout_minutes: default_session_timeout_minutes(),
            max_failed_attempts: default_max_failed_attempts(),
            lockout_duration_minutes: default_lockout_duration_minutes(),
            oauth: OAuthConfig::default(),
        }
    }
}

impl Default for BuildConfig {
    fn default() -> Self {
        Self {
            default_timeout_minutes: default_build_timeout_minutes(),
            max_concurrent_builds: default_max_concurrent_builds(),
            log_retention_days: default_build_log_retention_days(),
            artifact_retention_days: default_artifact_retention_days(),
            default_image: default_build_image(),
            executor_type: default_executor_type(),
            shared_volume: None,
            enable_cache: default_true(),
            cache_dir: None,
            resources: BuildResources::default(),
        }
    }
}

impl Default for BuildResources {
    fn default() -> Self {
        Self {
            cpu_limit: default_cpu_limit(),
            memory_limit_mb: default_memory_limit(),
            disk_limit_gb: default_disk_limit(),
            max_processes: default_max_processes(),
        }
    }
}

impl Default for GitConfig {
    fn default() -> Self {
        Self {
            repos_root: PathBuf::from("/var/lib/onedev/repos"),
            http_backend: default_git_http_backend(),
            max_pack_size_mb: default_max_pack_size(),
            receive_timeout_secs: default_receive_timeout(),
            enable_lfs: default_true(),
            lfs_storage_path: None,
            enable_hooks: default_true(),
            hook_timeout_secs: default_hook_timeout(),
            gc_enabled: default_true(),
            gc_interval_hours: default_gc_interval(),
        }
    }
}

impl Default for WebhookConfig {
    fn default() -> Self {
        Self {
            max_retries: default_webhook_max_retries(),
            retry_delay_secs: default_webhook_retry_delay(),
            max_retry_delay_secs: default_webhook_max_retry_delay(),
            timeout_secs: default_webhook_timeout(),
            verify_ssl: default_true(),
            allowed_hosts: Vec::new(),
            blocked_hosts: Vec::new(),
            secret_header: default_webhook_secret_header(),
        }
    }
}

impl Default for CleanupConfig {
    fn default() -> Self {
        Self {
            build_log_retention_days: default_build_log_retention_days(),
            artifact_retention_days: default_artifact_retention_days(),
            temp_file_retention_hours: default_temp_file_retention_hours(),
            audit_log_retention_days: default_audit_log_retention_days(),
            webhook_log_retention_days: default_webhook_log_retention_days(),
            search_index_cleanup_days: default_search_index_cleanup_days(),
            run_interval_hours: default_cleanup_run_interval_hours(),
        }
    }
}

impl Default for SearchConfig {
    fn default() -> Self {
        Self {
            batch_size: default_search_batch_size(),
            index_interval_secs: default_search_index_interval_secs(),
            full_reindex_interval_hours: default_search_full_reindex_interval_hours(),
            enable_code_search: default_true(),
            enable_issue_search: default_true(),
            enable_wiki_search: default_true(),
        }
    }
}

impl Default for EmailConfig {
    fn default() -> Self {
        Self {
            smtp_host: "localhost".to_string(),
            smtp_port: default_smtp_port(),
            smtp_username: None,
            smtp_password: None,
            starttls: default_true(),
            ssl: false,
            from_address: "noreply@onedev.local".to_string(),
            from_name: default_from_name(),
            enabled: false,
            queue_dir: None,
        }
    }
}

impl Default for StorageConfig {
    fn default() -> Self {
        Self {
            storage_type: default_storage_type(),
            local_path: Some(PathBuf::from("/var/lib/onedev/storage")),
            s3: None,
            gcs: None,
            azure: None,
            max_file_size_mb: default_max_file_size(),
            allowed_mime_types: vec![
                "application/octet-stream".to_string(),
                "text/plain".to_string(),
                "image/png".to_string(),
                "image/jpeg".to_string(),
                "image/gif".to_string(),
                "application/pdf".to_string(),
                "application/zip".to_string(),
                "application/x-tar".to_string(),
                "application/gzip".to_string(),
            ],
        }
    }
}

impl Default for ObservabilityConfig {
    fn default() -> Self {
        Self {
            log_level: default_log_level(),
            log_format: default_log_format(),
            metrics_enabled: default_true(),
            metrics_port: default_metrics_port(),
            tracing_enabled: default_true(),
            trace_sample_rate: default_trace_sample_rate(),
            profiling_enabled: false,
        }
    }
}

impl Default for SecurityConfig {
    fn default() -> Self {
        Self {
            cors_origins: vec!["*".to_string()],
            trusted_proxies: Vec::new(),
            csp: Some(
                "default-src 'self'; script-src 'self' 'unsafe-inline' 'unsafe-eval'; style-src 'self' 'unsafe-inline'; img-src 'self' data: https:; font-src 'self' data:; connect-src 'self' wss: https:;".to_string()
            ),
            hsts_enabled: default_true(),
            hsts_max_age: default_hsts_max_age(),
            referrer_policy: default_referrer_policy(),
            permissions_policy: None,
            rate_limit: RateLimitConfig::default(),
        }
    }
}

impl Default for RateLimitConfig {
    fn default() -> Self {
        Self {
            requests_per_minute: default_rpm(),
            burst: default_burst(),
            enabled: default_true(),
        }
    }
}

impl Default for FeatureFlags {
    fn default() -> Self {
        Self {
            builds: default_true(),
            merge_requests: default_true(),
            issues: default_true(),
            wiki: default_true(),
            packages: default_true(),
            code_search: default_true(),
            code_navigation: default_true(),
            webhooks: default_true(),
            ldap: false,
            saml: false,
            oidc: false,
            two_fa: default_true(),
            audit_logs: default_true(),
            project_templates: default_true(),
        }
    }
}