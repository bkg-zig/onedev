//! Configuration for the OneDev server.
//!
//! Layering (later wins):
//! 1. built-in defaults,
//! 2. `config.toml` / `config.yaml` / `config.yml` / `config.json` inside
//!    `ONEDEV_CONFIG_DIR` (default `/etc/onedev`),
//! 3. `ONEDEV_*` environment variables, `__` separates nesting levels
//!    (`ONEDEV_DATABASE__URL`, `ONEDEV_SECURITY__CORS_ORIGINS='["https://x"]'`).
//!
//! Every section has defaults, so a minimal production setup only needs the
//! database URL (and the JWT key material when it should not be generated).

use std::collections::BTreeMap;
use std::fmt;
use std::net::IpAddr;
use std::path::{Path, PathBuf};
use std::time::Duration;

use figment::providers::{Env, Format, Json, Serialized, Toml, Yaml};
use figment::Figment;
use ipnet::IpNet;
use serde::{Deserialize, Serialize};

const REDACTED: &str = "[redacted]";

/// Complete application configuration.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct AppConfig {
    pub server: ServerConfig,
    pub database: DatabaseConfig,
    pub auth: AuthConfig,
    pub build: BuildConfig,
    pub git: GitConfig,
    pub webhook: WebhookConfig,
    pub cleanup: CleanupConfig,
    pub search: SearchConfig,
    pub storage: StorageConfig,
    pub observability: ObservabilityConfig,
    pub security: SecurityConfig,
    pub features: FeatureFlags,
}

/// HTTP server settings.
#[derive(Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ServerConfig {
    pub bind_address: String,
    pub port: u16,
    /// Upper bound of concurrently processed requests (0 = unlimited).
    pub max_concurrent_requests: usize,
    /// Timeout for regular requests; git, LFS and artifact transfers are exempt.
    pub request_timeout_secs: u64,
    /// Maximum request body in MiB for transfers (git pushes, LFS, artifacts).
    pub max_body_mb: u64,
    /// Maximum request body in MiB for regular JSON / form endpoints.
    pub max_api_body_mb: u64,
    /// Seconds to wait for in-flight requests and background jobs on shutdown.
    pub shutdown_grace_secs: u64,
    /// In-process TLS is not supported: terminate TLS at the gateway.
    pub tls_enabled: bool,
    pub tls_cert_path: Option<PathBuf>,
    pub tls_key_path: Option<PathBuf>,
    /// `development`, `staging` or `production`.
    pub environment: String,
    /// Externally visible base URL (used in links, e.g. webhook payloads).
    pub public_url: Option<String>,
    pub sentry_dsn: Option<String>,
    pub sentry_traces_sample_rate: f32,
    /// Export traces over OTLP/HTTP (JSON) to `otlp_endpoint`.
    pub otel_enabled: bool,
    /// Base URL of the OTLP/HTTP collector, e.g. `http://otel-collector:4318`.
    pub otlp_endpoint: Option<String>,
    /// Extra request headers for the collector (e.g. `authorization`).
    pub otlp_headers: BTreeMap<String, String>,
    /// Fraction of traces that are exported (0.0 - 1.0).
    pub otel_sample_rate: f64,
    /// `service.name` resource attribute of exported traces.
    pub service_name: String,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            bind_address: "127.0.0.1".to_string(),
            port: 18080,
            max_concurrent_requests: 1000,
            request_timeout_secs: 60,
            max_body_mb: 512,
            max_api_body_mb: 16,
            shutdown_grace_secs: 30,
            tls_enabled: false,
            tls_cert_path: None,
            tls_key_path: None,
            environment: "development".to_string(),
            public_url: None,
            sentry_dsn: None,
            sentry_traces_sample_rate: 0.1,
            otel_enabled: false,
            otlp_endpoint: None,
            otlp_headers: BTreeMap::new(),
            otel_sample_rate: 0.1,
            service_name: "onedev-server".to_string(),
        }
    }
}

impl fmt::Debug for ServerConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ServerConfig")
            .field("bind_address", &self.bind_address)
            .field("port", &self.port)
            .field("max_concurrent_requests", &self.max_concurrent_requests)
            .field("request_timeout_secs", &self.request_timeout_secs)
            .field("max_body_mb", &self.max_body_mb)
            .field("max_api_body_mb", &self.max_api_body_mb)
            .field("shutdown_grace_secs", &self.shutdown_grace_secs)
            .field("tls_enabled", &self.tls_enabled)
            .field("environment", &self.environment)
            .field("public_url", &self.public_url)
            .field("sentry_dsn", &self.sentry_dsn.as_ref().map(|_| REDACTED))
            .field("sentry_traces_sample_rate", &self.sentry_traces_sample_rate)
            .field("otel_enabled", &self.otel_enabled)
            .field("otlp_endpoint", &self.otlp_endpoint)
            .field(
                "otlp_headers",
                &self.otlp_headers.keys().collect::<Vec<_>>(),
            )
            .field("otel_sample_rate", &self.otel_sample_rate)
            .field("service_name", &self.service_name)
            .finish()
    }
}

/// PostgreSQL connection settings.
#[derive(Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct DatabaseConfig {
    pub url: String,
    pub max_connections: u32,
    pub min_connections: u32,
    pub connect_timeout_secs: u64,
    pub idle_timeout_secs: u64,
    pub max_lifetime_secs: u64,
    pub sqlx_logging: bool,
    /// Apply embedded migrations on startup.
    pub run_migrations: bool,
}

impl Default for DatabaseConfig {
    fn default() -> Self {
        Self {
            url: "postgresql://onedev:onedev@localhost:5432/onedev".to_string(),
            max_connections: 20,
            min_connections: 2,
            connect_timeout_secs: 30,
            idle_timeout_secs: 600,
            max_lifetime_secs: 1800,
            sqlx_logging: false,
            run_migrations: true,
        }
    }
}

impl fmt::Debug for DatabaseConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DatabaseConfig")
            .field("url", &REDACTED)
            .field("max_connections", &self.max_connections)
            .field("min_connections", &self.min_connections)
            .field("connect_timeout_secs", &self.connect_timeout_secs)
            .field("idle_timeout_secs", &self.idle_timeout_secs)
            .field("max_lifetime_secs", &self.max_lifetime_secs)
            .field("run_migrations", &self.run_migrations)
            .finish()
    }
}

/// Authentication settings.
///
/// Access tokens are RS256 JWTs. The key pair is read from the configured PEM
/// files; when none is configured a key pair is generated once and persisted
/// below `storage.local_path/keys` (mode 0600).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AuthConfig {
    pub jwt_private_key_file: Option<PathBuf>,
    pub jwt_public_key_file: Option<PathBuf>,
    pub jwt_issuer: String,
    pub jwt_audience: String,
    pub access_token_ttl_minutes: i64,
    pub refresh_token_ttl_days: i64,
    pub two_fa_issuer: String,
    pub session_timeout_minutes: i64,
    pub remember_me_days: i64,
    pub max_failed_attempts: u32,
    pub lockout_duration_minutes: i64,
    pub password_reset_ttl_minutes: i64,
}

impl Default for AuthConfig {
    fn default() -> Self {
        Self {
            jwt_private_key_file: None,
            jwt_public_key_file: None,
            jwt_issuer: "onedev".to_string(),
            jwt_audience: "onedev-api".to_string(),
            access_token_ttl_minutes: 15,
            refresh_token_ttl_days: 7,
            two_fa_issuer: "OneDev".to_string(),
            session_timeout_minutes: 24 * 60,
            remember_me_days: 30,
            max_failed_attempts: 5,
            lockout_duration_minutes: 15,
            password_reset_ttl_minutes: 60,
        }
    }
}

/// Build scheduler settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct BuildConfig {
    /// Fallback build timeout (a project's `build_timeout` wins).
    pub default_timeout_minutes: i64,
    pub max_concurrent_builds: usize,
    /// Seconds between scheduler polls (LISTEN/NOTIFY wakes it earlier).
    pub poll_interval_secs: u64,
    /// A running build whose heartbeat is older than this is failed.
    pub stale_after_secs: u64,
    /// Maximum stored log lines per build.
    pub max_log_lines: u64,
    pub log_retention_days: i64,
    pub artifact_retention_days: i64,
    /// Override of `<storage.local_path>/builds`.
    pub work_dir: Option<PathBuf>,
    /// Override of `<storage.local_path>/artifacts`.
    pub artifact_dir: Option<PathBuf>,
    /// How command steps are executed (`host`, `docker` or `youki`).
    pub executor: BuildExecutorKind,
    /// Docker CLI used by the `docker` executor.
    pub docker_binary: PathBuf,
    /// OCI runtime used by the `youki` executor.
    pub youki_binary: PathBuf,
    /// Runtime state directory of the `youki` executor
    /// (default `<storage.local_path>/youki`).
    pub youki_root: Option<PathBuf>,
    /// Pulled image layers and unpacked root filesystems of the `youki`
    /// executor (default `<storage.local_path>/images`).
    pub image_cache_dir: Option<PathBuf>,
    /// Network of `youki` step containers: `host` shares the server's
    /// network (builds can reach the internet, but also local services),
    /// `none` gives every container an empty network namespace.
    pub youki_network: YoukiNetworkKind,
    /// Absolute cgroup (v2) directory below which `youki` creates the
    /// container cgroups, e.g. `/system.slice/onedev.service/builds`. The
    /// server user must own it (systemd `Delegate=yes`). Default: below the
    /// cgroup of the server process.
    pub youki_cgroup_parent: Option<String>,
    /// Docker-style `config.json` (`{"auths": {"<registry>": {"auth": "<base64>"}}}`)
    /// with registry credentials; the secrets stay out of this file.
    pub registry_credentials_file: Option<PathBuf>,
    /// Image of command steps that do not name one (container executors).
    pub default_image: Option<String>,
    /// Memory limit per step container, e.g. `2g`.
    pub container_memory: Option<String>,
    /// CPU limit per step container, e.g. `2`.
    pub container_cpus: Option<String>,
    /// Maximum number of processes per step container.
    pub container_pids_limit: Option<u32>,
    /// Grace period between SIGTERM and SIGKILL when stopping a step.
    pub term_grace_secs: u64,
    /// Maximum size of the artifacts one job may publish.
    pub max_artifact_mb: u64,
}

/// Where command steps run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum BuildExecutorKind {
    /// Directly on the server host. No isolation: only for trusted projects.
    #[default]
    Host,
    /// In a container started through the Docker CLI.
    Docker,
    /// In a rootless OCI container run by `youki`.
    Youki,
}

/// Network mode of `youki` step containers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum YoukiNetworkKind {
    #[default]
    Host,
    None,
}

impl BuildExecutorKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Host => "host",
            Self::Docker => "docker",
            Self::Youki => "youki",
        }
    }
}

impl Default for BuildConfig {
    fn default() -> Self {
        Self {
            default_timeout_minutes: 60,
            max_concurrent_builds: 4,
            poll_interval_secs: 5,
            stale_after_secs: 120,
            max_log_lines: 200_000,
            log_retention_days: 30,
            artifact_retention_days: 90,
            work_dir: None,
            artifact_dir: None,
            executor: BuildExecutorKind::Host,
            docker_binary: PathBuf::from("docker"),
            youki_binary: PathBuf::from("youki"),
            youki_root: None,
            image_cache_dir: None,
            youki_network: YoukiNetworkKind::Host,
            youki_cgroup_parent: None,
            registry_credentials_file: None,
            default_image: None,
            container_memory: None,
            container_cpus: None,
            container_pids_limit: Some(4096),
            term_grace_secs: 5,
            max_artifact_mb: 1024,
        }
    }
}

/// Git storage and maintenance settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct GitConfig {
    /// Directory containing the bare repositories.
    pub repos_root: PathBuf,
    pub git_binary: PathBuf,
    pub default_branch: String,
    pub max_pack_size_mb: u64,
    pub max_lfs_object_mb: u64,
    pub lfs_storage_path: Option<PathBuf>,
    pub gc_enabled: bool,
    pub gc_interval_hours: u64,
    /// Repositories processed per GC cycle.
    pub gc_batch_size: usize,
    /// Loose objects above which a repository is repacked.
    pub gc_loose_object_threshold: u64,
    /// `git prune --expire` value.
    pub gc_prune_expire: String,
    /// Timeout of a single git maintenance command.
    pub gc_command_timeout_secs: u64,
}

impl Default for GitConfig {
    fn default() -> Self {
        Self {
            repos_root: PathBuf::from("/var/opt/onedev/git"),
            git_binary: PathBuf::from("git"),
            default_branch: "main".to_string(),
            max_pack_size_mb: 512,
            max_lfs_object_mb: 512,
            lfs_storage_path: None,
            gc_enabled: true,
            gc_interval_hours: 24,
            gc_batch_size: 10,
            gc_loose_object_threshold: 1000,
            gc_prune_expire: "2.weeks.ago".to_string(),
            gc_command_timeout_secs: 1800,
        }
    }
}

/// Webhook delivery settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct WebhookConfig {
    pub max_retries: u32,
    pub retry_delay_secs: u64,
    pub max_retry_delay_secs: u64,
    pub timeout_secs: u64,
    pub poll_interval_secs: u64,
    pub batch_size: usize,
    pub verify_ssl: bool,
    /// When non-empty only these hosts (`example.com` or `*.example.com`) may be called.
    pub allowed_hosts: Vec<String>,
    /// Hosts that are never called.
    pub blocked_hosts: Vec<String>,
    /// Allow targets in private / loopback / link-local ranges (SSRF guard off).
    pub allow_private_targets: bool,
    /// Maximum stored response body per delivery.
    pub max_response_bytes: usize,
}

impl Default for WebhookConfig {
    fn default() -> Self {
        Self {
            max_retries: 5,
            retry_delay_secs: 10,
            max_retry_delay_secs: 3600,
            timeout_secs: 30,
            poll_interval_secs: 10,
            batch_size: 50,
            verify_ssl: true,
            allowed_hosts: Vec::new(),
            blocked_hosts: Vec::new(),
            allow_private_targets: false,
            max_response_bytes: 8192,
        }
    }
}

/// Retention settings of the cleanup service.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct CleanupConfig {
    pub enabled: bool,
    pub run_interval_hours: u64,
    pub build_log_retention_days: i64,
    pub artifact_retention_days: i64,
    pub temp_file_retention_hours: i64,
    pub audit_log_retention_days: i64,
    pub webhook_log_retention_days: i64,
    pub revoked_token_retention_days: i64,
}

impl Default for CleanupConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            run_interval_hours: 1,
            build_log_retention_days: 30,
            artifact_retention_days: 90,
            temp_file_retention_hours: 24,
            audit_log_retention_days: 365,
            webhook_log_retention_days: 30,
            revoked_token_retention_days: 30,
        }
    }
}

/// Search indexer settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct SearchConfig {
    pub batch_size: usize,
    pub index_interval_secs: u64,
    pub full_reindex_interval_hours: u64,
    pub enable_code_search: bool,
    pub enable_issue_search: bool,
    /// Files larger than this are not indexed (KiB).
    pub max_indexed_file_kb: u64,
    /// Maximum files indexed per project and cycle.
    pub max_files_per_project: usize,
}

impl Default for SearchConfig {
    fn default() -> Self {
        Self {
            batch_size: 200,
            index_interval_secs: 60,
            full_reindex_interval_hours: 24,
            enable_code_search: true,
            enable_issue_search: true,
            max_indexed_file_kb: 512,
            max_files_per_project: 20_000,
        }
    }
}

/// Local storage settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct StorageConfig {
    pub local_path: PathBuf,
    pub max_file_size_mb: u64,
}

impl Default for StorageConfig {
    fn default() -> Self {
        Self {
            local_path: PathBuf::from("/var/opt/onedev/storage"),
            max_file_size_mb: 512,
        }
    }
}

/// Logging / metrics / tracing settings.
#[derive(Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ObservabilityConfig {
    pub log_level: String,
    /// `json` or `text`.
    pub log_format: String,
    pub metrics_enabled: bool,
    /// When set, `/metrics` requires `Authorization: Bearer <token>`.
    pub metrics_token: Option<String>,
    pub db_metrics_interval_secs: u64,
}

impl Default for ObservabilityConfig {
    fn default() -> Self {
        Self {
            log_level: "info".to_string(),
            log_format: "json".to_string(),
            metrics_enabled: true,
            metrics_token: None,
            db_metrics_interval_secs: 15,
        }
    }
}

impl fmt::Debug for ObservabilityConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ObservabilityConfig")
            .field("log_level", &self.log_level)
            .field("log_format", &self.log_format)
            .field("metrics_enabled", &self.metrics_enabled)
            .field("metrics_token", &self.metrics_token.as_ref().map(|_| REDACTED))
            .field("db_metrics_interval_secs", &self.db_metrics_interval_secs)
            .finish()
    }
}

/// HTTP hardening settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct SecurityConfig {
    /// Allowed CORS origins (`*` is rejected in production).
    pub cors_origins: Vec<String>,
    /// Proxies (IP or CIDR) whose `X-Forwarded-For` / `X-Real-IP` is trusted.
    pub trusted_proxies: Vec<String>,
    pub csp: Option<String>,
    pub hsts_enabled: bool,
    pub hsts_max_age: u64,
    pub referrer_policy: String,
    pub permissions_policy: Option<String>,
    pub rate_limit: RateLimitConfig,
}

impl Default for SecurityConfig {
    fn default() -> Self {
        Self {
            cors_origins: Vec::new(),
            trusted_proxies: Vec::new(),
            csp: None,
            hsts_enabled: true,
            hsts_max_age: 31_536_000,
            referrer_policy: "strict-origin-when-cross-origin".to_string(),
            permissions_policy: None,
            rate_limit: RateLimitConfig::default(),
        }
    }
}

/// Per-client request rate limit.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct RateLimitConfig {
    pub enabled: bool,
    pub requests_per_minute: u32,
    pub burst: u32,
}

impl Default for RateLimitConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            requests_per_minute: 1200,
            burst: 200,
        }
    }
}

/// Feature switches for background services.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct FeatureFlags {
    pub builds: bool,
    pub webhooks: bool,
    pub code_search: bool,
    pub issue_search: bool,
    pub git_gc: bool,
    pub cleanup: bool,
}

impl Default for FeatureFlags {
    fn default() -> Self {
        Self {
            builds: true,
            webhooks: true,
            code_search: true,
            issue_search: true,
            git_gc: true,
            cleanup: true,
        }
    }
}

/// Configuration loading / validation failure.
#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("failed to load configuration: {0}")]
    Load(String),
    #[error("invalid configuration:\n  - {}", .0.join("\n  - "))]
    Invalid(Vec<String>),
}

impl AppConfig {
    /// Load the configuration (defaults → files → environment).
    pub fn load() -> Result<Self, ConfigError> {
        let dir = std::env::var("ONEDEV_CONFIG_DIR").unwrap_or_else(|_| "/etc/onedev".to_string());
        Self::load_from(Path::new(&dir))
    }

    /// Load the configuration with `dir` as configuration directory.
    pub fn load_from(dir: &Path) -> Result<Self, ConfigError> {
        let mut figment = Figment::from(Serialized::defaults(AppConfig::default()));
        let toml = dir.join("config.toml");
        let yaml = dir.join("config.yaml");
        let yml = dir.join("config.yml");
        let json = dir.join("config.json");
        if toml.is_file() {
            figment = figment.merge(Toml::file(toml));
        }
        if yaml.is_file() {
            figment = figment.merge(Yaml::file(yaml));
        }
        if yml.is_file() {
            figment = figment.merge(Yaml::file(yml));
        }
        if json.is_file() {
            figment = figment.merge(Json::file(json));
        }
        figment = figment.merge(Env::prefixed("ONEDEV_").split("__"));
        figment
            .extract::<AppConfig>()
            .map_err(|e| ConfigError::Load(e.to_string()))
    }

    pub fn is_production(&self) -> bool {
        self.server.environment.eq_ignore_ascii_case("production")
    }

    pub fn request_timeout(&self) -> Duration {
        Duration::from_secs(self.server.request_timeout_secs.max(1))
    }

    /// Maximum request body in bytes.
    pub fn max_body_bytes(&self) -> usize {
        let mb = self
            .server
            .max_body_mb
            .max(self.git.max_pack_size_mb)
            .max(self.storage.max_file_size_mb);
        usize::try_from(mb.saturating_mul(1024 * 1024)).unwrap_or(usize::MAX)
    }

    /// Parsed `security.trusted_proxies` (single addresses become host networks).
    pub fn trusted_proxy_nets(&self) -> Result<Vec<IpNet>, ConfigError> {
        self.security
            .trusted_proxies
            .iter()
            .map(|entry| parse_net(entry))
            .collect::<Result<_, _>>()
    }

    /// Directory below which build work trees are created.
    pub fn build_work_dir(&self) -> PathBuf {
        self.build
            .work_dir
            .clone()
            .unwrap_or_else(|| self.storage.local_path.join("builds"))
    }

    /// Directory below which build artifacts are stored.
    pub fn build_artifact_dir(&self) -> PathBuf {
        self.build
            .artifact_dir
            .clone()
            .unwrap_or_else(|| self.storage.local_path.join("artifacts"))
    }

    /// Runtime state directory of the `youki` executor.
    pub fn youki_root(&self) -> PathBuf {
        self.build
            .youki_root
            .clone()
            .unwrap_or_else(|| self.storage.local_path.join("youki"))
    }

    /// Image layer / root filesystem cache of the `youki` executor.
    pub fn image_cache_dir(&self) -> PathBuf {
        self.build
            .image_cache_dir
            .clone()
            .unwrap_or_else(|| self.storage.local_path.join("images"))
    }

    /// Directory for temporary files (cleaned up by the cleanup service).
    pub fn temp_dir(&self) -> PathBuf {
        self.storage.local_path.join("tmp")
    }

    /// Directory holding generated key material.
    pub fn key_dir(&self) -> PathBuf {
        self.storage.local_path.join("keys")
    }

    /// Check the configuration, reporting every problem at once.
    pub fn validate(&self) -> Result<(), ConfigError> {
        let mut problems = Vec::new();

        if !["development", "staging", "production"]
            .contains(&self.server.environment.to_ascii_lowercase().as_str())
        {
            problems.push(format!(
                "server.environment must be development, staging or production (got `{}`)",
                self.server.environment
            ));
        }
        if self.server.bind_address.parse::<IpAddr>().is_err() {
            problems.push(format!(
                "server.bind_address `{}` is not an IP address",
                self.server.bind_address
            ));
        }
        if self.server.port == 0 {
            problems.push("server.port must not be 0".to_string());
        }
        if self.server.tls_enabled {
            problems.push(
                "server.tls_enabled is not supported: terminate TLS at the gateway".to_string(),
            );
        }
        if self.database.url.trim().is_empty() {
            problems.push("database.url must not be empty".to_string());
        }
        if self.database.max_connections == 0 {
            problems.push("database.max_connections must be at least 1".to_string());
        }
        if self.database.min_connections > self.database.max_connections {
            problems.push("database.min_connections exceeds database.max_connections".to_string());
        }
        if self.build.max_concurrent_builds == 0 {
            problems.push("build.max_concurrent_builds must be at least 1".to_string());
        }
        if self.build.term_grace_secs == 0 || self.build.max_artifact_mb == 0 {
            problems.push("build.term_grace_secs and build.max_artifact_mb must be at least 1".to_string());
        }
        if let Some(memory) = &self.build.container_memory {
            let digits = memory.trim_end_matches(|c: char| c.is_ascii_alphabetic());
            let unit = &memory[digits.len()..];
            if digits.is_empty()
                || !digits.chars().all(|c| c.is_ascii_digit())
                || !["", "b", "k", "m", "g", "kb", "mb", "gb", "K", "M", "G", "KB", "MB", "GB"].contains(&unit)
            {
                problems.push(format!(
                    "build.container_memory must look like `512m` or `2g` (got `{memory}`)"
                ));
            }
        }
        if let Some(cpus) = &self.build.container_cpus {
            if cpus.parse::<f64>().map_or(true, |value| !(value > 0.0)) {
                problems.push(format!("build.container_cpus must be a positive number (got `{cpus}`)"));
            }
        }
        if self.webhook.max_retries == 0 {
            problems.push("webhook.max_retries must be at least 1".to_string());
        }
        if self.security.rate_limit.enabled
            && (self.security.rate_limit.requests_per_minute == 0 || self.security.rate_limit.burst == 0)
        {
            problems.push(
                "security.rate_limit.requests_per_minute and burst must be positive".to_string(),
            );
        }
        if self.auth.jwt_private_key_file.is_some() != self.auth.jwt_public_key_file.is_some() {
            problems.push(
                "auth.jwt_private_key_file and auth.jwt_public_key_file must be set together"
                    .to_string(),
            );
        }
        if self.auth.access_token_ttl_minutes <= 0 || self.auth.refresh_token_ttl_days <= 0 {
            problems.push("auth token lifetimes must be positive".to_string());
        }
        if !["json", "text"].contains(&self.observability.log_format.to_ascii_lowercase().as_str()) {
            problems.push(format!(
                "observability.log_format must be json or text (got `{}`)",
                self.observability.log_format
            ));
        }
        if let Err(e) = tracing_subscriber::EnvFilter::try_new(&self.observability.log_level) {
            problems.push(format!("observability.log_level is not a valid filter: {e}"));
        }
        if self.server.otel_enabled {
            match self.server.otlp_endpoint.as_deref() {
                Some(endpoint) if endpoint.starts_with("http://") || endpoint.starts_with("https://") => {}
                _ => problems.push(
                    "server.otlp_endpoint must be an http(s) URL when server.otel_enabled is set"
                        .to_string(),
                ),
            }
        }
        if !(0.0..=1.0).contains(&self.server.otel_sample_rate)
            || !(0.0..=1.0).contains(&self.server.sentry_traces_sample_rate)
        {
            problems.push("trace sample rates must be between 0.0 and 1.0".to_string());
        }
        if let Err(e) = self.trusted_proxy_nets() {
            problems.push(e.to_string());
        }
        for origin in &self.security.cors_origins {
            if origin != "*" && origin.parse::<axum::http::HeaderValue>().is_err() {
                problems.push(format!("security.cors_origins contains invalid origin `{origin}`"));
            }
        }

        if self.is_production() {
            if self.security.cors_origins.iter().any(|o| o == "*") {
                problems.push("security.cors_origins must not contain `*` in production".to_string());
            }
            if self.database.url.contains("onedev:onedev@") {
                problems.push(
                    "database.url uses the default onedev:onedev credentials in production"
                        .to_string(),
                );
            }
            if self.webhook.allow_private_targets {
                problems.push(
                    "webhook.allow_private_targets must be false in production".to_string(),
                );
            }
            if !self.webhook.verify_ssl {
                problems.push("webhook.verify_ssl must be true in production".to_string());
            }
        }

        if problems.is_empty() {
            Ok(())
        } else {
            Err(ConfigError::Invalid(problems))
        }
    }
}

fn parse_net(entry: &str) -> Result<IpNet, ConfigError> {
    let entry = entry.trim();
    if let Ok(net) = entry.parse::<IpNet>() {
        return Ok(net);
    }
    entry
        .parse::<IpAddr>()
        .map(IpNet::from)
        .map_err(|_| ConfigError::Invalid(vec![format!("invalid trusted proxy `{entry}`")]))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_valid_in_development() {
        AppConfig::default().validate().expect("defaults must validate");
    }

    #[test]
    fn production_rejects_default_credentials_and_wildcard_cors() {
        let mut config = AppConfig::default();
        config.server.environment = "production".into();
        config.security.cors_origins = vec!["*".into()];
        let err = config.validate().unwrap_err().to_string();
        assert!(err.contains("cors_origins"), "{err}");
        assert!(err.contains("default onedev:onedev"), "{err}");
    }

    #[test]
    fn production_accepts_hardened_config() {
        let mut config = AppConfig::default();
        config.server.environment = "production".into();
        config.database.url = "postgresql://onedev:s3cret@db:5432/onedev".into();
        config.security.cors_origins = vec!["https://git.example.com".into()];
        config.validate().expect("hardened config must validate");
    }

    #[test]
    fn tls_is_rejected_because_it_is_unsupported() {
        let mut config = AppConfig::default();
        config.server.tls_enabled = true;
        assert!(config.validate().is_err());
    }

    #[test]
    fn trusted_proxies_accept_single_addresses_and_cidrs() {
        let mut config = AppConfig::default();
        config.security.trusted_proxies = vec!["10.0.0.0/8".into(), "127.0.0.1".into(), "::1".into()];
        let nets = config.trusted_proxy_nets().unwrap();
        assert!(nets[0].contains(&"10.1.2.3".parse::<IpAddr>().unwrap()));
        assert!(nets[1].contains(&"127.0.0.1".parse::<IpAddr>().unwrap()));
        config.security.trusted_proxies = vec!["not-an-ip".into()];
        assert!(config.validate().is_err());
    }

    #[test]
    fn layering_applies_file_then_environment() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("config.toml"),
            "[server]\nport = 9001\n[build]\nmax_concurrent_builds = 7\n",
        )
        .unwrap();
        let config = AppConfig::load_from(dir.path()).unwrap();
        assert_eq!(config.server.port, 9001);
        assert_eq!(config.build.max_concurrent_builds, 7);
        // untouched sections keep their defaults
        assert_eq!(config.webhook.max_retries, WebhookConfig::default().max_retries);
    }

    #[test]
    fn debug_output_never_contains_secrets() {
        let mut config = AppConfig::default();
        config.database.url = "postgresql://u:topsecret@h/db".into();
        config.server.sentry_dsn = Some("https://key@sentry.example/1".into());
        config.observability.metrics_token = Some("tok-123".into());
        let debug = format!("{config:?}");
        assert!(!debug.contains("topsecret"));
        assert!(!debug.contains("key@sentry"));
        assert!(!debug.contains("tok-123"));
    }
}
