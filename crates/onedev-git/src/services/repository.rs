//! Reading and creating bare repositories via gix.
//!
//! Covers the first step of the port order in
//! `docs/port-review/04-git-collaboration.md`: open a bare repository and
//! read refs, commits, trees and blobs. The pack protocol itself lives in
//! `smart_http` and delegates to the git executable, as vanilla also shells
//! out for parts of it.

use std::path::PathBuf;

use gix::objs::tree::EntryKind;
use gix::prelude::*;

use crate::errors::{GitError, GitResult};
use crate::models::{BlobInfo, CommitInfo, FileChange, FileChangeStatus, GitConfig, GitRef, GitSignature, TreeEntry, TreeEntryKind, CompareResult};
use crate::utils::{parse_lfs_pointer, repository_path, run_git};

/// Repository read/create operations for one storage root.
#[derive(Debug, Clone)]
pub struct RepositoryService {
    config: GitConfig,
}

impl RepositoryService {
    pub fn new(config: GitConfig) -> Self {
        RepositoryService { config }
    }

    pub fn config(&self) -> &GitConfig {
        &self.config
    }

    /// Absolute path of the bare repository of a project.
    pub fn repository_path(&self, project_path: &str) -> GitResult<PathBuf> {
        repository_path(&self.config.root, project_path)
    }

    /// Create a new bare repository. Fails when it already exists.
    pub fn create_repository(&self, project_path: &str) -> GitResult<PathBuf> {
        let path = self.repository_path(project_path)?;
        if path.exists() {
            return Err(GitError::AlreadyExists(path.display().to_string()));
        }
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        gix::init_bare(&path).map_err(|err| GitError::msg(err.to_string()))?;
        // Point HEAD at the configured default branch.
        std::fs::write(
            path.join("HEAD"),
            format!("ref: refs/heads/{}\n", self.config.default_branch),
        )?;
        Ok(path)
    }

    pub fn repository_exists(&self, project_path: &str) -> bool {
        self.repository_path(project_path)
            .map(|path| path.is_dir())
            .unwrap_or(false)
    }

    fn open(&self, project_path: &str) -> GitResult<gix::Repository> {
        let path = self.repository_path(project_path)?;
        if !path.is_dir() {
            return Err(GitError::NotFound(path.display().to_string()));
        }
        gix::open(&path).map_err(|err| GitError::Object(err.to_string()))
    }

    /// All refs of the repository, including the resolved `HEAD`.
    pub fn list_refs(&self, project_path: &str) -> GitResult<Vec<GitRef>> {
        let repo = self.open(project_path)?;
        let mut refs = Vec::new();

        if let Ok(head) = repo.head_id() {
            refs.push(GitRef::new("HEAD".to_owned(), head.to_string()));
        }

        let platform = repo.references().map_err(GitError::other)?;
        for reference in platform.all().map_err(GitError::other)? {
            let mut reference = reference.map_err(|err| GitError::msg(err.to_string()))?;
            let name = reference.name().as_bstr().to_string();
            let id = reference
                .peel_to_id_in_place()
                .map_err(GitError::other)?
                .to_string();
            refs.push(GitRef::new(name, id));
        }
        Ok(refs)
    }

    /// Branches only (`refs/heads/*`).
    pub fn list_branches(&self, project_path: &str) -> GitResult<Vec<GitRef>> {
        Ok(self
            .list_refs(project_path)?
            .into_iter()
            .filter(|r| r.kind == crate::models::GitRefKind::Branch)
            .collect())
    }

    /// Tags only (`refs/tags/*`).
    pub fn list_tags(&self, project_path: &str) -> GitResult<Vec<GitRef>> {
        Ok(self
            .list_refs(project_path)?
            .into_iter()
            .filter(|r| r.kind == crate::models::GitRefKind::Tag)
            .collect())
    }

    fn signature(sig: gix::actor::SignatureRef<'_>) -> GitSignature {
        GitSignature {
            name: sig.name.to_string(),
            email: sig.email.to_string(),
            timestamp: sig.time().map(|time| time.seconds).unwrap_or(0),
        }
    }

    fn commit_info(commit: &gix::objs::CommitRef<'_>) -> GitResult<CommitInfo> {
        Ok(CommitInfo {
            id: String::new(), // filled by caller
            tree_id: commit.tree.to_string(),
            parents: commit.parents.iter().map(|id| id.to_string()).collect(),
            author: Self::signature(commit.author),
            committer: Self::signature(commit.committer),
            message: commit.message.to_string(),
        })
    }

    /// Resolve a revision (ref name, `HEAD`, hex id, `rev~1`, ...) to a commit.
    pub fn resolve_commit(&self, project_path: &str, revision: &str) -> GitResult<CommitInfo> {
        let repo = self.open(project_path)?;
        let id = repo
            .rev_parse_single(revision)
            .map_err(|_| GitError::InvalidRevision(revision.to_owned()))?;
        let object = id.object().map_err(GitError::other)?;
        let commit = object
            .try_into_commit()
            .map_err(|_| GitError::InvalidRevision(revision.to_owned()))?;
        let mut info = Self::commit_info(&commit.decode().map_err(GitError::other)?)?;
        info.id = commit.id.to_string();
        Ok(info)
    }

    /// Entries of the tree at `revision:path` (root tree when `path` is empty).
    pub fn list_tree(
        &self,
        project_path: &str,
        revision: &str,
        path: &str,
    ) -> GitResult<Vec<TreeEntry>> {
        let repo = self.open(project_path)?;
        let id = repo
            .rev_parse_single(revision)
            .map_err(|_| GitError::InvalidRevision(revision.to_owned()))?;
        let commit = id
            .object()
            .map_err(GitError::other)?
            .try_into_commit()
            .map_err(|_| GitError::InvalidRevision(revision.to_owned()))?;
        let root = commit.tree().map_err(GitError::other)?;

        let tree = if path.is_empty() {
            root
        } else {
            let entry = root
                .lookup_entry(path.split('/'))
                .map_err(GitError::other)?
                .ok_or_else(|| GitError::NotFound(format!("{revision}:{path}")))?;
            entry
                .object()
                .map_err(GitError::other)?
                .try_into_tree()
                .map_err(|_| GitError::NotFound(format!("{revision}:{path} is not a tree")))?
        };

        let prefix = if path.is_empty() {
            String::new()
        } else {
            format!("{path}/")
        };
        let mut entries = Vec::new();
        for entry in tree.iter() {
            let entry = entry.map_err(GitError::other)?;
            let kind = match entry.mode().kind() {
                EntryKind::Tree => TreeEntryKind::Tree,
                EntryKind::Blob | EntryKind::BlobExecutable => TreeEntryKind::Blob,
                EntryKind::Link => TreeEntryKind::Symlink,
                EntryKind::Commit => TreeEntryKind::Submodule,
            };
            let size = if kind == TreeEntryKind::Blob {
                repo.find_object(entry.oid())
                    .ok()
                    .map(|obj| obj.data.len() as u64)
            } else {
                None
            };
            entries.push(TreeEntry {
                path: format!("{prefix}{}", entry.filename()),
                object_id: entry.oid().to_string(),
                kind,
                size,
            });
        }
        Ok(entries)
    }

    /// Blob content at `revision:path`.
    pub fn read_blob(&self, project_path: &str, revision: &str, path: &str) -> GitResult<BlobInfo> {
        let repo = self.open(project_path)?;
        let id = repo
            .rev_parse_single(revision)
            .map_err(|_| GitError::InvalidRevision(revision.to_owned()))?;
        let commit = id
            .object()
            .map_err(GitError::other)?
            .try_into_commit()
            .map_err(|_| GitError::InvalidRevision(revision.to_owned()))?;
        let tree = commit.tree().map_err(GitError::other)?;
        let entry = tree
            .lookup_entry(path.split('/'))
            .map_err(GitError::other)?
            .ok_or_else(|| GitError::NotFound(format!("{revision}:{path}")))?;
        let object = entry.object().map_err(GitError::other)?;
        Ok(BlobInfo {
            object_id: object.id.to_string(),
            is_lfs_pointer: parse_lfs_pointer(&object.data).is_some(),
            content: object.data.clone(),
        })
    }

    /// Blob content by object id.
    pub fn read_blob_by_id(&self, project_path: &str, oid: &str) -> GitResult<BlobInfo> {
        let repo = self.open(project_path)?;
        let id = gix::ObjectId::from_hex(oid.as_bytes())
            .map_err(|_| GitError::Object(format!("invalid object id: {oid}")))?;
        let object = repo.find_object(id).map_err(GitError::other)?;
        Ok(BlobInfo {
            object_id: oid.to_owned(),
            is_lfs_pointer: parse_lfs_pointer(&object.data).is_some(),
            content: object.data.clone(),
        })
    }

    /// Commit history starting at `revision`, newest first, at most `limit`.
    pub fn history(&self, project_path: &str, revision: &str, limit: usize) -> GitResult<Vec<CommitInfo>> {
        let repo = self.open(project_path)?;
        let id = repo
            .rev_parse_single(revision)
            .map_err(|_| GitError::InvalidRevision(revision.to_owned()))?;
        let walk = repo.rev_walk([id]).all()
            .map_err(GitError::other)?;

        let mut commits = Vec::new();
        for info in walk {
            if commits.len() >= limit {
                break;
            }
            let info = info.map_err(GitError::other)?;
            let object = repo.find_object(info.id).map_err(GitError::other)?;
            let commit = object
                .try_into_commit()
                .map_err(|_| GitError::InvalidRevision(revision.to_owned()))?;
            let mut entry = Self::commit_info(&commit.decode().map_err(GitError::other)?)?;
            entry.id = info.id.to_string();
            commits.push(entry);
        }
        Ok(commits)
    }

    /// Compare two revisions: commits in `to` but not in `from`, plus changed files.
    pub fn compare(&self, project_path: &str, from: &str, to: &str) -> GitResult<CompareResult> {
        let ancestors: std::collections::HashSet<String> = self
            .history(project_path, from, usize::MAX / 2)?
            .into_iter()
            .map(|c| c.id)
            .collect();
        let commits: Vec<CommitInfo> = self
            .history(project_path, to, usize::MAX / 2)?
            .into_iter()
            .filter(|c| !ancestors.contains(&c.id))
            .collect();

        let path = self.repository_path(project_path)?;
        let out = run_git(
            &self.config.git_binary,
            &path,
            &["diff", "--name-status", from, to],
            None,
        )?;
        let mut files = Vec::new();
        for line in String::from_utf8_lossy(&out).lines() {
            let mut parts = line.splitn(2, '\t');
            let (Some(status), Some(file)) = (parts.next(), parts.next()) else {
                continue;
            };
            files.push(FileChange {
                path: file.to_owned(),
                status: match status.chars().next() {
                    Some('A') => FileChangeStatus::Added,
                    Some('D') => FileChangeStatus::Deleted,
                    _ => FileChangeStatus::Modified,
                },
                old_id: None,
                new_id: None,
            });
        }

        Ok(CompareResult {
            from: from.to_owned(),
            to: to.to_owned(),
            commits,
            files,
        })
    }
}
