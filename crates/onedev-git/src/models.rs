//! Value objects shared by the Git services.
//!
//! Naming follows OneDev (`PullRequest`, `Build`) rather than the GitLab-style
//! names that are still present in parts of `onedev-core`.

use std::path::PathBuf;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::errors::{GitError, GitResult};

/// The two Git smart-HTTP services.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum GitService {
    /// Fetch/clone (`git-upload-pack`).
    UploadPack,
    /// Push (`git-receive-pack`).
    ReceivePack,
}

impl GitService {
    pub fn as_str(self) -> &'static str {
        match self {
            GitService::UploadPack => "git-upload-pack",
            GitService::ReceivePack => "git-receive-pack",
        }
    }

    pub fn parse(value: &str) -> GitResult<Self> {
        match value {
            "git-upload-pack" => Ok(GitService::UploadPack),
            "git-receive-pack" => Ok(GitService::ReceivePack),
            other => Err(GitError::Protocol(format!("unknown git service: {other}"))),
        }
    }

    /// Content type Git clients expect for the ref advertisement.
    pub fn advertisement_type(self) -> &'static str {
        match self {
            GitService::UploadPack => "application/x-git-upload-pack-advertisement",
            GitService::ReceivePack => "application/x-git-receive-pack-advertisement",
        }
    }

    /// Content type Git clients expect for the RPC result.
    pub fn result_type(self) -> &'static str {
        match self {
            GitService::UploadPack => "application/x-git-upload-pack-result",
            GitService::ReceivePack => "application/x-git-receive-pack-result",
        }
    }
}

impl std::fmt::Display for GitService {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Configuration of the Git storage layer.
#[derive(Debug, Clone, PartialEq)]
pub struct GitConfig {
    /// Root directory holding bare repositories (vanilla: `gitDir` setting).
    pub root: PathBuf,
    /// Branch used when a repository is created and for `HEAD`.
    pub default_branch: String,
    /// Git executable used for the pack protocol and housekeeping.
    pub git_binary: PathBuf,
    /// Directory for LFS objects; defaults to `<repo>/.git/lfs` when `None`.
    pub lfs_root: Option<PathBuf>,
    /// Maximum accepted LFS object size in bytes.
    pub max_lfs_object_size: u64,
}

impl Default for GitConfig {
    fn default() -> Self {
        GitConfig {
            root: PathBuf::from("/var/opt/onedev/git"),
            default_branch: "main".to_owned(),
            git_binary: PathBuf::from("git"),
            lfs_root: None,
            max_lfs_object_size: 512 * 1024 * 1024,
        }
    }
}

/// Kind of a ref, derived from its namespace.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GitRefKind {
    Head,
    Branch,
    Tag,
    Note,
    PullRequest,
    Other,
}

/// A ref as exposed to the UI and to the ref advertisement.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GitRef {
    /// Fully qualified name, e.g. `refs/heads/main`.
    pub name: String,
    /// Hex object id the ref points at.
    pub object_id: String,
    pub kind: GitRefKind,
}

impl GitRef {
    pub fn new(name: String, object_id: String) -> Self {
        let kind = if name == "HEAD" {
            GitRefKind::Head
        } else if name.starts_with("refs/heads/") {
            GitRefKind::Branch
        } else if name.starts_with("refs/tags/") {
            GitRefKind::Tag
        } else if name.starts_with("refs/notes/") {
            GitRefKind::Note
        } else if name.starts_with("refs/pull/") {
            GitRefKind::PullRequest
        } else {
            GitRefKind::Other
        };
        GitRef { name, object_id, kind }
    }

    /// The name without its `refs/<namespace>/` prefix.
    pub fn short_name(&self) -> &str {
        for prefix in [
            "refs/heads/",
            "refs/tags/",
            "refs/notes/",
            "refs/pull/",
        ] {
            if let Some(rest) = self.name.strip_prefix(prefix) {
                return rest;
            }
        }
        &self.name
    }
}

/// A Git signature (author or committer).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GitSignature {
    pub name: String,
    pub email: String,
    /// Unix timestamp in seconds.
    pub timestamp: i64,
}

/// A commit as needed by the file browser, history and PR views.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommitInfo {
    pub id: String,
    pub tree_id: String,
    pub parents: Vec<String>,
    pub author: GitSignature,
    pub committer: GitSignature,
    pub message: String,
}

/// Entry of a tree listing.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TreeEntry {
    pub path: String,
    pub object_id: String,
    pub kind: TreeEntryKind,
    /// Blob size, when cheaply available from the object database.
    pub size: Option<u64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TreeEntryKind {
    Blob,
    Tree,
    Symlink,
    Submodule,
}

/// Content of a blob.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BlobInfo {
    pub object_id: String,
    pub content: Vec<u8>,
    /// True when the content is an LFS pointer rather than the real payload.
    pub is_lfs_pointer: bool,
}

impl BlobInfo {
    pub fn text(&self) -> Option<&str> {
        std::str::from_utf8(&self.content).ok()
    }
}

/// One changed path of a commit range comparison.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileChange {
    pub path: String,
    pub status: FileChangeStatus,
    pub old_id: Option<String>,
    pub new_id: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FileChangeStatus {
    Added,
    Modified,
    Deleted,
}

/// Result of comparing two revisions (`A...B` semantics for commit lists).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CompareResult {
    pub from: String,
    pub to: String,
    pub commits: Vec<CommitInfo>,
    pub files: Vec<FileChange>,
}

/// A single ref update contained in a push.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RefUpdate {
    pub ref_name: String,
    /// `None` for a new ref.
    pub old_id: Option<String>,
    /// `None` for a deleted ref.
    pub new_id: Option<String>,
}

impl RefUpdate {
    pub fn is_create(&self) -> bool {
        self.old_id.is_none() && self.new_id.is_some()
    }

    pub fn is_delete(&self) -> bool {
        self.new_id.is_none()
    }

    pub fn is_force(&self) -> bool {
        // A force push is not reliably detectable from the command alone; the
        // hook layer checks ancestry when a rule requires it.
        false
    }

    pub fn short_name(&self) -> &str {
        self.ref_name
            .strip_prefix("refs/heads/")
            .or_else(|| self.ref_name.strip_prefix("refs/tags/"))
            .unwrap_or(&self.ref_name)
    }
}

/// Identity of the user performing a push.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GitActor {
    pub id: String,
    pub name: String,
}

/// Context passed to the receive hooks.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PushContext {
    pub project_path: String,
    pub actor: GitActor,
    pub updates: Vec<RefUpdate>,
    pub push_options: Vec<String>,
}

/// Rule applied to pushes to a branch or tag.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProtectionRule {
    /// Glob pattern, e.g. `main` or `release/*`.
    pub pattern: String,
    pub prevent_force_push: bool,
    pub prevent_deletion: bool,
    pub prevent_creation: bool,
    pub require_pull_request: bool,
    pub required_approvals: u32,
    pub require_successful_build: bool,
    pub require_signed_commits: bool,
    /// Actor names explicitly allowed to bypass the rule.
    pub bypass_users: Vec<String>,
}

impl Default for ProtectionRule {
    fn default() -> Self {
        ProtectionRule {
            pattern: String::new(),
            prevent_force_push: true,
            prevent_deletion: true,
            prevent_creation: false,
            require_pull_request: false,
            required_approvals: 0,
            require_successful_build: false,
            require_signed_commits: false,
            bypass_users: Vec::new(),
        }
    }
}

/// Branch/tag protection configuration of a project.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProtectionRules {
    pub branches: Vec<ProtectionRule>,
    pub tags: Vec<ProtectionRule>,
}

/// Result of a pre-receive check.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HookDecision {
    Allow,
    Deny(String),
}

/// A push that passed the pre-receive hooks.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PushEvent {
    pub context: PushContext,
    pub accepted_at: DateTime<Utc>,
}

/// Extension point for post-receive side effects (build trigger, notifications).
pub trait PostReceiveListener: Send + Sync {
    fn on_push(&self, event: &PushEvent) -> GitResult<()>;
}

/// Response of a smart-HTTP operation, independent of the HTTP framework.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitHttpResponse {
    pub status: u16,
    pub content_type: &'static str,
    pub body: Vec<u8>,
}

impl GitHttpResponse {
    pub fn ok(content_type: &'static str, body: Vec<u8>) -> Self {
        GitHttpResponse { status: 200, content_type, body }
    }
}

/// LFS object identifier and size.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LfsObject {
    pub oid: String,
    pub size: u64,
}

/// Parsed LFS pointer file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LfsPointer {
    pub oid: String,
    pub size: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LfsOperation {
    Upload,
    Download,
}

impl LfsOperation {
    pub fn parse(value: &str) -> GitResult<Self> {
        match value {
            "upload" => Ok(LfsOperation::Upload),
            "download" => Ok(LfsOperation::Download),
            other => Err(GitError::Lfs(format!("unknown LFS operation: {other}"))),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LfsBatchRequest {
    pub operation: LfsOperation,
    pub objects: Vec<LfsObject>,
    pub transfers: Option<Vec<String>>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LfsBatchResponse {
    pub transfer: &'static str,
    pub objects: Vec<LfsBatchObject>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LfsBatchObject {
    pub oid: String,
    pub size: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub actions: Option<LfsActions>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<LfsErrorInfo>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LfsActions {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub upload: Option<LfsAction>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub download: Option<LfsAction>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LfsAction {
    pub href: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LfsErrorInfo {
    pub code: u16,
    pub message: String,
}

/// An LFS file lock.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LfsLock {
    pub id: String,
    pub path: String,
    pub owner: String,
    pub locked_at: DateTime<Utc>,
}
