//! Admin Models

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdminStats {
    pub total_users: u64,
    pub active_users: u64,
    pub total_projects: u64,
    pub public_projects: u64,
    pub total_builds_today: u64,
    pub running_builds: u64,
    pub pending_builds: u64,
    pub webhook_deliveries_24h: u64,
    pub failed_webhooks_24h: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemInfo {
    pub version: String,
    pub rust_version: String,
    pub uptime: String,
    pub cpu_usage: f64,
    pub memory_usage: f64,
    pub disk_usage: f64,
    pub network_in: f64,
    pub network_out: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecentUser {
    pub id: Uuid,
    pub username: String,
    pub email: String,
    pub full_name: String,
    pub created_at: DateTime<Utc>,
    pub last_sign_in: Option<DateTime<Utc>>,
    pub state: String,
    pub admin: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecentProject {
    pub id: Uuid,
    pub name: String,
    pub path: String,
    pub owner: String,
    pub visibility: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    pub id: Uuid,
    pub username: String,
    pub email: String,
    pub full_name: String,
    pub avatar_url: Option<String>,
    pub state: String,
    pub admin: bool,
    pub created_at: DateTime<Utc>,
    pub last_sign_in: Option<DateTime<Utc>>,
    pub projects_count: u32,
    pub ssh_keys_count: u32,
    pub gpg_keys_count: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdminProject {
    pub id: Uuid,
    pub name: String,
    pub path: String,
    pub description: Option<String>,
    pub visibility: String,
    pub owner: String,
    pub stars: u32,
    pub forks: u32,
    pub open_issues: u32,
    pub open_mrs: u32,
    pub last_activity: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
    pub archived: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseInfo {
    pub size: String,
    pub tables: u32,
    pub connections_active: u32,
    pub connections_idle: u32,
    pub max_connections: u32,
    pub cache_hit_ratio: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageInfo {
    pub storage_type: String,
    pub path: String,
    pub total_space: String,
    pub used_space: String,
    pub available_space: String,
    pub repositories_size: String,
    pub artifacts_size: String,
    pub lfs_size: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuildQueueInfo {
    pub running: u64,
    pub pending: u64,
    pub max_concurrent: u64,
    pub avg_wait_time: String,
    pub completed_today: u64,
    pub failed_today: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditLogEntry {
    pub id: Uuid,
    pub user_id: Option<Uuid>,
    pub username: Option<String>,
    pub action: String,
    pub entity_type: String,
    pub entity_id: Option<Uuid>,
    pub details: serde_json::Value,
    pub ip: Option<String>,
    pub user_agent: Option<String>,
    pub created_at: DateTime<Utc>,
}
