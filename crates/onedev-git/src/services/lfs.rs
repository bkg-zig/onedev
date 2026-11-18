//! Git LFS object storage and lock handling.
//!
//! LFS pointers live in Git blobs; the binary payloads live next to the bare
//! repository under `lfs/objects/<oid[0..2]>/<oid[2..4]>/<oid>`, matching the
//! layout used by git-lfs and by vanilla OneDev.

use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

use crate::errors::{GitError, GitResult};
use crate::models::{
    GitConfig, LfsAction, LfsActions, LfsBatchObject, LfsBatchRequest, LfsBatchResponse, LfsLock,
    LfsObject, LfsOperation,
};
use crate::utils::{is_safe_relative_path, normalize_project_path, repository_path, sha256_hex};
use chrono::Utc;
use uuid::Uuid;

/// Process-wide lock guarding read-modify-write of the lock files.
fn lock_guard() -> &'static Mutex<HashMap<PathBuf, ()>> {
    static LOCKS: OnceLock<Mutex<HashMap<PathBuf, ()>>> = OnceLock::new();
    LOCKS.get_or_init(|| Mutex::new(HashMap::new()))
}

/// LFS operations for a project.
#[derive(Debug, Clone)]
pub struct LfsService {
    config: GitConfig,
}

impl LfsService {
    pub fn new(config: GitConfig) -> Self {
        LfsService { config }
    }

    /// Directory for LFS metadata/objects of a project.
    pub fn lfs_dir(&self, project_path: &str) -> GitResult<PathBuf> {
        Ok(match &self.config.lfs_root {
            Some(root) => root.join(normalize_project_path(project_path)?),
            None => repository_path(&self.config.root, project_path)?.join("lfs"),
        })
    }

    /// Path of a single LFS object.
    pub fn object_path(&self, project_path: &str, oid: &str) -> GitResult<PathBuf> {
        if oid.len() < 4 || !oid.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(GitError::Lfs(format!("invalid oid: {oid}")));
        }
        Ok(self
            .lfs_dir(project_path)?
            .join("objects")
            .join(&oid[0..2])
            .join(&oid[2..4])
            .join(oid))
    }

    pub fn has_object(&self, project_path: &str, oid: &str) -> bool {
        self.object_path(project_path, oid)
            .map(|path| path.is_file())
            .unwrap_or(false)
    }

    /// Store a verified LFS object. Returns an error for a hash/size mismatch.
    pub fn store(&self, project_path: &str, oid: &str, content: &[u8]) -> GitResult<()> {
        if content.len() as u64 > self.config.max_lfs_object_size {
            return Err(GitError::Lfs(format!(
                "object exceeds limit of {} bytes",
                self.config.max_lfs_object_size
            )));
        }
        let actual = sha256_hex(content);
        if actual != oid {
            return Err(GitError::Lfs(format!(
                "oid mismatch: expected {oid}, computed {actual}"
            )));
        }

        let path = self.object_path(project_path, oid)?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let temp = path.with_extension(format!("tmp-{}", Uuid::new_v4()));
        fs::write(&temp, content)?;
        fs::rename(&temp, &path)?;
        Ok(())
    }

    /// Read an LFS object from storage.
    pub fn load(&self, project_path: &str, oid: &str) -> GitResult<Vec<u8>> {
        if !self.has_object(project_path, oid) {
            return Err(GitError::NotFound(format!("LFS object {oid}")));
        }
        Ok(fs::read(self.object_path(project_path, oid)?)?)
    }

    /// Build the batch response. `url_for` receives the project path, oid and
    /// operation so the HTTP layer can construct absolute URLs.
    pub fn batch<F>(
        &self,
        project_path: &str,
        request: &LfsBatchRequest,
        url_for: F,
    ) -> GitResult<LfsBatchResponse>
    where
        F: Fn(&str, &str, LfsOperation) -> String,
    {
        if !request
            .transfers
            .as_ref()
            .is_none_or(|t| t.iter().any(|t| t == "basic"))
        {
            return Err(GitError::Lfs("unsupported transfer adapter".to_owned()));
        }

        let mut objects = Vec::with_capacity(request.objects.len());
        for LfsObject { oid, size } in &request.objects {
            if *size > self.config.max_lfs_object_size {
                objects.push(LfsBatchObject {
                    oid: oid.clone(),
                    size: *size,
                    actions: None,
                    error: Some(crate::models::LfsErrorInfo {
                        code: 413,
                        message: "object exceeds storage limit".to_owned(),
                    }),
                });
                continue;
            }

            let exists = self.has_object(project_path, oid);
            let action = |operation| LfsAction {
                href: url_for(project_path, oid, operation),
            };
            let actions = match request.operation {
                LfsOperation::Upload if !exists => Some(LfsActions {
                    upload: Some(action(LfsOperation::Upload)),
                    download: None,
                }),
                LfsOperation::Upload => None,
                LfsOperation::Download if exists => Some(LfsActions {
                    upload: None,
                    download: Some(action(LfsOperation::Download)),
                }),
                LfsOperation::Download => {
                    objects.push(LfsBatchObject {
                        oid: oid.clone(),
                        size: *size,
                        actions: None,
                        error: Some(crate::models::LfsErrorInfo {
                            code: 404,
                            message: "object not found".to_owned(),
                        }),
                    });
                    continue;
                }
            };

            objects.push(LfsBatchObject {
                oid: oid.clone(),
                size: *size,
                actions,
                error: None,
            });
        }

        Ok(LfsBatchResponse {
            transfer: "basic",
            objects,
        })
    }

    fn locks_path(&self, project_path: &str) -> GitResult<PathBuf> {
        Ok(self.lfs_dir(project_path)?.join("locks.json"))
    }

    pub fn locks(&self, project_path: &str, path: Option<&str>) -> GitResult<Vec<LfsLock>> {
        let path = path.unwrap_or("");
        let _guard = lock_guard().lock().unwrap_or_else(|e| e.into_inner());
        let all = self.read_locks(project_path)?;
        Ok(all
            .into_iter()
            .filter(|lock| path.is_empty() || lock.path == path)
            .collect())
    }

    pub fn create_lock(&self, project_path: &str, path: &str, owner: &str) -> GitResult<LfsLock> {
        if !is_safe_relative_path(path) {
            return Err(GitError::Lfs(format!("invalid lock path: {path}")));
        }

        let _guard = lock_guard().lock().unwrap_or_else(|e| e.into_inner());
        let mut locks = self.read_locks(project_path)?;
        if let Some(existing) = locks.iter().find(|lock| lock.path == path) {
            return Err(GitError::Lfs(format!(
                "path is already locked by {} ({})",
                existing.owner, existing.id
            )));
        }
        let lock = LfsLock {
            id: Uuid::new_v4().to_string(),
            path: path.to_owned(),
            owner: owner.to_owned(),
            locked_at: Utc::now(),
        };
        locks.push(lock.clone());
        self.write_locks(project_path, &locks)?;
        Ok(lock)
    }

    pub fn unlock(&self, project_path: &str, id: &str, owner: &str) -> GitResult<()> {
        let _guard = lock_guard().lock().unwrap_or_else(|e| e.into_inner());
        let mut locks = self.read_locks(project_path)?;
        let Some(index) = locks.iter().position(|lock| lock.id == id) else {
            return Err(GitError::NotFound(format!("LFS lock {id}")));
        };
        if locks[index].owner != owner && owner != "admin" {
            return Err(GitError::PermissionDenied(format!(
                "lock {id} belongs to another user"
            )));
        }
        locks.remove(index);
        self.write_locks(project_path, &locks)
    }

    fn read_locks(&self, project_path: &str) -> GitResult<Vec<LfsLock>> {
        let path = self.locks_path(project_path)?;
        if !path.exists() {
            return Ok(Vec::new());
        }
        let content = fs::read_to_string(path)?;
        Ok(serde_json::from_str(&content)?)
    }

    fn write_locks(&self, project_path: &str, locks: &[LfsLock]) -> GitResult<()> {
        let path = self.locks_path(project_path)?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&path, serde_json::to_string_pretty(locks)?)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::LfsPointer;
    use crate::utils::{format_lfs_pointer, parse_lfs_pointer};

    fn service(root: &PathBuf) -> LfsService {
        LfsService::new(GitConfig {
            root: root.clone(),
            ..GitConfig::default()
        })
    }

    #[test]
    fn stores_and_loads_verified_objects() {
        let root = tempfile::tempdir().unwrap();
        let service = service(&root.path().to_path_buf());
        let content = b"large file content";
        let oid = sha256_hex(content);

        assert!(
            service
                .store("acme/app", "b".repeat(64).as_str(), content)
                .is_err()
        );
        service.store("acme/app", &oid, content).unwrap();
        assert_eq!(service.load("acme/app", &oid).unwrap(), content);
        assert!(service.has_object("acme/app", &oid));
    }

    #[test]
    fn batch_reports_upload_and_download_actions() {
        let root = tempfile::tempdir().unwrap();
        let service = service(&root.path().to_path_buf());
        let oid = sha256_hex(b"payload");
        let request = LfsBatchRequest {
            operation: LfsOperation::Upload,
            objects: vec![LfsObject {
                oid: oid.clone(),
                size: 7,
            }],
            transfers: None,
        };
        let response = service
            .batch("acme/app", &request, |_, oid, op| {
                format!("/lfs/{oid}/{op}")
            })
            .unwrap();
        assert!(
            response.objects[0]
                .actions
                .as_ref()
                .unwrap()
                .upload
                .is_some()
        );

        let request = LfsBatchRequest {
            operation: LfsOperation::Download,
            ..request
        };
        let response = service
            .batch("acme/app", &request, |_, oid, op| {
                format!("/lfs/{oid}/{op}")
            })
            .unwrap();
        assert_eq!(response.objects[0].error.as_ref().unwrap().code, 404);

        service.store("acme/app", &oid, b"payload").unwrap();
        let response = service
            .batch("acme/app", &request, |_, oid, op| {
                format!("/lfs/{oid}/{op}")
            })
            .unwrap();
        assert!(
            response.objects[0]
                .actions
                .as_ref()
                .unwrap()
                .download
                .is_some()
        );
    }

    #[test]
    fn lock_lifecycle() {
        let root = tempfile::tempdir().unwrap();
        let service = service(&root.path().to_path_buf());
        let lock = service
            .create_lock("acme/app", "assets/big.bin", "alice")
            .unwrap();
        assert_eq!(service.locks("acme/app", None).unwrap().len(), 1);
        assert!(
            service
                .create_lock("acme/app", "assets/big.bin", "bob")
                .is_err()
        );
        assert!(service.unlock("acme/app", &lock.id, "bob").is_err());
        service.unlock("acme/app", &lock.id, "alice").unwrap();
        assert!(service.locks("acme/app", None).unwrap().is_empty());
    }

    #[test]
    fn pointer_uses_lfs_spec() {
        let pointer: LfsPointer =
            parse_lfs_pointer(format_lfs_pointer("a".repeat(64).as_str(), 1).as_bytes()).unwrap();
        assert_eq!(pointer.oid, "a".repeat(64));
    }
}
