//! Web UI Configuration

use figment::{
    Figment,
    providers::{Env, Format, Json, Toml, Yaml},
};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Web UI configuration
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct WebConfig {
    /// Server configuration
    pub server: ServerConfig,
    /// Template configuration
    pub templates: TemplateConfig,
    /// Static assets configuration
    pub static_assets: StaticAssetsConfig,
    /// API configuration
    pub api: ApiConfig,
    /// Authentication configuration
    pub auth: AuthConfig,
    /// Features
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
    /// Enable TLS
    #[serde(default)]
    pub tls_enabled: bool,
    /// TLS cert path
    pub tls_cert_path: Option<PathBuf>,
    /// TLS key path
    pub tls_key_path: Option<PathBuf>,
    /// Request timeout in seconds
    #[serde(default = "default_request_timeout_secs")]
    pub request_timeout_secs: u64,
    /// Max request body size (bytes)
    #[serde(default = "default_max_body_size")]
    pub max_body_size: usize,
}

fn default_bind_address() -> String {
    "0.0.0.0".to_string()
}

fn default_port() -> u16 {
    46315
}

fn default_request_timeout_secs() -> u64 {
    60
}

fn default_max_body_size() -> usize {
    50 * 1024 * 1024 // 50MB
}

/// Template configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemplateConfig {
    /// Template directory
    #[serde(default = "default_template_dir")]
    pub template_dir: PathBuf,
    /// Enable template auto-reload in development
    #[serde(default = "default_true")]
    pub auto_reload: bool,
    /// Cache templates
    #[serde(default = "default_true")]
    pub cache: bool,
}

fn default_template_dir() -> PathBuf {
    PathBuf::from("templates")
}

fn default_true() -> bool {
    true
}

/// Static assets configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StaticAssetsConfig {
    /// Static files directory
    #[serde(default = "default_static_dir")]
    pub static_dir: PathBuf,
    /// Enable cache headers
    #[serde(default = "default_true")]
    pub cache_headers: bool,
    /// Cache max age (seconds)
    #[serde(default = "default_cache_max_age")]
    pub cache_max_age: u64,
}

fn default_static_dir() -> PathBuf {
    PathBuf::from("static")
}

fn default_cache_max_age() -> u64 {
    86400 // 24 hours
}

/// API configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiConfig {
    /// Base URL for API calls
    #[serde(default = "default_api_base_url")]
    pub base_url: String,
    /// Request timeout
    #[serde(default = "default_api_timeout_secs")]
    pub timeout_secs: u64,
    /// Enable request/response logging
    #[serde(default)]
    pub debug_logging: bool,
}

fn default_api_base_url() -> String {
    "http://localhost:46315".to_string()
}

fn default_api_timeout_secs() -> u64 {
    30
}

/// Authentication configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthConfig {
    /// Session cookie name
    #[serde(default = "default_session_cookie_name")]
    pub session_cookie_name: String,
    /// Session secret (base64 encoded 64 bytes)
    pub session_secret: String,
    /// Session max age (seconds)
    #[serde(default = "default_session_max_age")]
    pub session_max_age: u64,
    /// Secure cookies (HTTPS only)
    #[serde(default = "default_true")]
    pub secure_cookies: bool,
    /// SameSite policy
    #[serde(default = "default_same_site")]
    pub same_site: String,
}

fn default_session_cookie_name() -> String {
    "onedev_session".to_string()
}

fn default_session_max_age() -> u64 {
    24 * 60 * 60 // 24 hours
}

fn default_same_site() -> String {
    "Lax".to_string()
}

/// Feature flags
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeatureFlags {
    /// Enable SSR
    #[serde(default = "default_true")]
    pub ssr: bool,
    /// Enable dark mode
    #[serde(default = "default_true")]
    pub dark_mode: bool,
    /// Enable syntax highlighting
    #[serde(default = "default_true")]
    pub syntax_highlight: bool,
    /// Enable markdown rendering
    #[serde(default = "default_true")]
    pub markdown: bool,
    /// Enable real-time updates
    #[serde(default)]
    pub realtime: bool,
}

impl WebConfig {
    /// Load configuration from files and environment
    pub fn load() -> Result<Self, Box<figment::Error>> {
        let config_dir = std::env::var("ONEDEV_WEB_CONFIG_DIR")
            .unwrap_or_else(|_| "/etc/onedev/web".to_string());

        Figment::new()
            .merge(Toml::file(format!("{}/config.toml", config_dir)))
            .merge(Yaml::file(format!("{}/config.yaml", config_dir)))
            .merge(Json::file(format!("{}/config.json", config_dir)))
            .merge(Env::prefixed("ONEDEV_WEB_").split("__").global())
            .extract()
            .map_err(Box::new)
    }
}

/// WebConfig implements Default via derive
impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            bind_address: default_bind_address(),
            port: default_port(),
            tls_enabled: false,
            tls_cert_path: None,
            tls_key_path: None,
            request_timeout_secs: default_request_timeout_secs(),
            max_body_size: default_max_body_size(),
        }
    }
}

impl Default for TemplateConfig {
    fn default() -> Self {
        Self {
            template_dir: default_template_dir(),
            auto_reload: default_true(),
            cache: default_true(),
        }
    }
}

impl Default for StaticAssetsConfig {
    fn default() -> Self {
        Self {
            static_dir: default_static_dir(),
            cache_headers: default_true(),
            cache_max_age: default_cache_max_age(),
        }
    }
}

impl Default for ApiConfig {
    fn default() -> Self {
        Self {
            base_url: default_api_base_url(),
            timeout_secs: default_api_timeout_secs(),
            debug_logging: false,
        }
    }
}

impl Default for AuthConfig {
    fn default() -> Self {
        Self {
            session_cookie_name: default_session_cookie_name(),
            session_secret: "change-me-in-production-use-strong-random-secret-64-bytes!!"
                .to_string(),
            session_max_age: default_session_max_age(),
            secure_cookies: default_true(),
            same_site: default_same_site(),
        }
    }
}

impl Default for FeatureFlags {
    fn default() -> Self {
        Self {
            ssr: default_true(),
            dark_mode: default_true(),
            syntax_highlight: default_true(),
            markdown: default_true(),
            realtime: false,
        }
    }
}
