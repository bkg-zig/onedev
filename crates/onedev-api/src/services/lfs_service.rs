//! LFS Service — backed by `onedev-git`'s `LfsService`.

use std::path::PathBuf;

use onedev_db::DbPool;
use onedev_git::models::{GitConfig, LfsBatchRequest, LfsBatchResponse, LfsLock, LfsObject, LfsOperation};
use onedev_git::services::lfs::LfsService as InnerLfsService;

pub struct LfsService {
    inner: InnerLfsService,
}

impl LfsService {
    pub fn new(_db_pool: DbPool) -> Self {
        Self::with_root(git_root())
    }

    pub fn with_root(root: PathBuf) -> Self {
        Self {
            inner: InnerLfsService::new(GitConfig {
                root,
                ..GitConfig::default()
            }),
        }
    }

    pub fn batch(
        &self,
        project_path: &str,
        operation: &str,
        objects: Vec<(String, u64)>,
        url_for: impl Fn(&str, &str, LfsOperation) -> String,
    ) -> Result<LfsBatchResponse, onedev_git::GitError> {
        let operation = LfsOperation::parse(operation)?;
        self.inner.batch(
            project_path,
            &LfsBatchRequest {
                operation,
                objects: objects
                    .into_iter()
                    .map(|(oid, size)| LfsObject { oid, size })
                    .collect(),
                transfers: None,
            },
            url_for,
        )
    }

    pub fn download(&self, project_path: &str, oid: &str) -> Result<Vec<u8>, onedev_git::GitError> {
        self.inner.load(project_path, oid)
    }

    pub fn upload(&self, project_path: &str, oid: &str, content: &[u8]) -> Result<(), onedev_git::GitError> {
        self.inner.store(project_path, oid, content)
    }

    pub fn locks(&self, project_path: &str, path: Option<&str>) -> Result<Vec<LfsLock>, onedev_git::GitError> {
        self.inner.locks(project_path, path)
    }

    pub fn create_lock(&self, project_path: &str, path: &str, owner: &str) -> Result<LfsLock, onedev_git::GitError> {
        self.inner.create_lock(project_path, path, owner)
    }

    pub fn unlock(&self, project_path: &str, id: &str, owner: &str) -> Result<(), onedev_git::GitError> {
        self.inner.unlock(project_path, id, owner)
    }
}

fn git_root() -> PathBuf {
    std::env::var("ONEDEV_GIT_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("/var/lib/onedev/repos"))
}
