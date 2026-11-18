//! Git Models

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Branch {
    pub name: String,
    pub protected: bool,
    pub commit: CommitRef,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommitRef {
    pub sha: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tag {
    pub name: String,
    pub commit: CommitRef,
    pub tagger: Tagger,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tagger {
    pub name: String,
    pub date: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Commit {
    pub sha: String,
    pub short_sha: String,
    pub message: String,
    pub author: CommitAuthor,
    pub authored_at: DateTime<Utc>,
    pub committed_at: DateTime<Utc>,
    pub verified: bool,
    pub parents: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommitAuthor {
    pub username: String,
    pub full_name: String,
    pub avatar_url: Option<String>,
    pub email: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TreeEntry {
    pub name: String,
    pub entry_type: TreeEntryType,
    pub mode: String,
    pub size: Option<u64>,
    pub path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TreeEntryType {
    Tree,
    Blob,
    Commit, // submodule
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Blob {
    pub path: String,
    pub size: u64,
    pub encoding: String,
    pub content: String,
    pub language: String,
    pub lines: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommitDetail {
    pub sha: String,
    pub short_sha: String,
    pub message: String,
    pub author: CommitAuthor,
    pub committer: CommitAuthor,
    pub authored_at: DateTime<Utc>,
    pub committed_at: DateTime<Utc>,
    pub verified: bool,
    pub parents: Vec<String>,
    pub stats: CommitStats,
    pub changes: Vec<FileChange>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommitStats {
    pub files_changed: u32,
    pub additions: u32,
    pub deletions: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileChange {
    pub old_path: String,
    pub new_path: String,
    pub status: ChangeStatus,
    pub additions: u32,
    pub deletions: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ChangeStatus {
    Added,
    Modified,
    Deleted,
    Renamed,
    Copied,
    TypeChanged,
}

// Type aliases for compatibility
pub type CommitInfo = CommitDetail;
pub type FileInfo = FileChange;
