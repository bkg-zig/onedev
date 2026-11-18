//! Profile Models

use serde::{Deserialize, Serialize};
use uuid::Uuid;
use chrono::{DateTime, Utc};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserProfile {
    pub id: Uuid,
    pub username: String,
    pub email: String,
    pub full_name: Option<String>,
    pub avatar_url: Option<String>,
    pub bio: Option<String>,
    pub website: Option<String>,
    pub location: Option<String>,
    pub theme: String,
    pub language: String,
    pub timezone: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub two_fa_enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserStats {
    pub projects_count: u32,
    pub stars_given: u32,
    pub followers: u32,
    pub following: u32,
    pub contributions_this_year: u32,
    pub longest_streak: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActivityItem {
    pub activity_type: ActivityType,
    pub project: String,
    pub message: String,
    pub date: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ActivityType {
    Commit,
    Issue,
    MergeRequest,
    Build,
    Release,
    Comment,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SshKey {
    pub id: Uuid,
    pub title: String,
    pub key_type: String,
    pub fingerprint: String,
    pub created_at: DateTime<Utc>,
    pub last_used_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GpgKey {
    pub id: Uuid,
    pub key_id: String,
    pub name: String,
    pub email: String,
    pub created_at: DateTime<Utc>,
    pub expires_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccessToken {
    pub id: Uuid,
    pub name: String,
    pub scopes: Vec<String>,
    pub created_at: DateTime<Utc>,
    pub expires_at: Option<DateTime<Utc>>,
    pub last_used_at: Option<DateTime<Utc>>,
    pub revoked: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub id: String,
    pub device: String,
    pub ip: String,
    pub location: Option<String>,
    pub created_at: DateTime<Utc>,
    pub last_active: DateTime<Utc>,
    pub current: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateSettingsForm {
    pub full_name: String,
    pub email: String,
    pub username: String,
    pub bio: Option<String>,
    pub website: Option<String>,
    pub location: Option<String>,
    pub theme: String,
    pub language: String,
    pub timezone: String,
}