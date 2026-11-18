//! Build Models

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Build {
    pub id: Uuid,
    pub number: u32,
    pub pipeline: String,
    pub branch: String,
    pub commit: CommitRef,
    pub status: BuildStatus,
    pub duration: Option<u32>,
    pub queued_at: DateTime<Utc>,
    pub started_at: Option<DateTime<Utc>>,
    pub finished_at: Option<DateTime<Utc>>,
    pub triggered_by: UserRef,
    pub variables: HashMap<String, String>,
    pub artifacts_count: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BuildStatus {
    Pending,
    Running,
    Success,
    Failed,
    Cancelled,
    Skipped,
    Manual,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommitRef {
    pub sha: String,
    pub short_sha: String,
    pub message: String,
    pub author: Option<UserRef>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserRef {
    pub id: Uuid,
    pub username: String,
    pub full_name: String,
    pub avatar_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuildStage {
    pub name: String,
    pub status: BuildStatus,
    pub duration: Option<u32>,
    pub jobs: Vec<BuildJob>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuildJob {
    pub id: Uuid,
    pub name: String,
    pub stage: String,
    pub status: BuildStatus,
    pub duration: Option<u32>,
    pub started_at: Option<DateTime<Utc>>,
    pub finished_at: Option<DateTime<Utc>>,
    pub runner: String,
    pub artifacts: Vec<BuildArtifact>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuildArtifact {
    pub name: String,
    pub size: u64,
    pub path: String,
    pub download_url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Pipeline {
    pub name: String,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TriggerBuildForm {
    pub pipeline: String,
    pub branch: String,
    pub variables: HashMap<String, String>,
}

// Type aliases for compatibility
pub type Stage = BuildStage;
pub type Job = BuildJob;
pub type Artifact = BuildArtifact;
