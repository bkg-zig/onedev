use figment::{Figment, providers::{Env, Format, Json, Toml, Yaml}};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    pub bind_address: String,
    pub port: u16,
    #[serde(default = "default_max_concurrent_requests")]
    pub max_concurrent_requests: usize,
    #[serde(default = "default_request_timeout_secs")]
    pub request_timeout_secs: u64,
    #[serde(default)]
    pub tls_enabled: bool,
    pub tls_cert_path: Option<PathBuf>,
    pub tls_key_path: Option<PathBuf>,
    #[serde(default = "default_environment")]
    pub environment: String,
    pub sentry_dsn: Option<String>,
    #[serde(default)]
    pub otel_enabled: bool,
    pub otlp_endpoint: Option<String>,
}

fn default_max_concurrent_requests() -> usize { 1000 }
fn default_request_timeout_secs() -> u64 { 60 }
fn default_environment() -> String { "development".to_string() }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseConfig {
    pub url: String,
    #[serde(default = "default_max_connections")]
    pub max_connections: u32,
    #[serde(default = "default_min_connections")]
    pub min_connections: u32,
    #[serde(default = "default_connect_timeout")]
    pub connect_timeout_secs: u64,
    #[serde(default = "default_idle_timeout")]
    pub idle_timeout_secs: u64,
    #[serde(default = "default_max_lifetime")]
    pub max_lifetime_secs: u64,
    #[serde(default)]
    pub sqlx_logging: bool,
}

fn default_max_connections() -> u32 { 20 }
fn default_min_connections() -> u32 { 5 }
fn default_connect_timeout() -> u64 { 30 }
fn default_idle_timeout() -> u64 { 600 }
fn default_max_lifetime() -> u64 { 1800 }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub server: ServerConfig,
    pub database: DatabaseConfig,
}

fn main() {
    let config_dir = "/tmp/onedev-config";
    
    // Test without .nested()
    let figment1 = Figment::new()
        .merge(Yaml::file(format!("{}/config.yaml", config_dir)))
        .merge(Env::prefixed("ONEDEV_").split("__").global());
    
    println!("=== Test without .nested() ===");
    match figment1.extract::<AppConfig>() {
        Ok(config) => println!("Config loaded: {:?}", config),
        Err(e) => {
            println!("Error: {:?}", e);
            for (i, err) in e.iter().enumerate() {
                println!("  Error {}: {}", i, err);
            }
        }
    }
    
    // Test with .nested()
    let figment2 = Figment::new()
        .merge(Yaml::file(format!("{}/config.yaml", config_dir)).nested())
        .merge(Env::prefixed("ONEDEV_").split("__").global());
    
    println!("\n=== Test with .nested() ===");
    match figment2.extract::<AppConfig>() {
        Ok(config) => println!("Config loaded: {:?}", config),
        Err(e) => {
            println!("Error: {:?}", e);
            for (i, err) in e.iter().enumerate() {
                println!("  Error {}: {}", i, err);
            }
        }
    }
}
