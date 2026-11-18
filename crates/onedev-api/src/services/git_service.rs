//! Git Service — Smart HTTP backed by `onedev-git`.

use std::path::PathBuf;

use onedev_db::DbPool;
use onedev_git::models::{GitActor, GitConfig, GitService as GitProtocol};
use onedev_git::services::hooks::HookEngine;
use onedev_git::{GitHttpResponse, GitResult, SmartHttpService};

pub struct GitService {
    http: SmartHttpService,
    hooks: HookEngine,
}

impl GitService {
    pub fn new(_db_pool: DbPool) -> Self {
        Self::with_root(git_root())
    }

    pub fn with_root(root: PathBuf) -> Self {
        let config = GitConfig {
            root: root.clone(),
            ..GitConfig::default()
        };
        Self {
            http: SmartHttpService::new(config),
            hooks: HookEngine::new(root, PathBuf::from("git")),
        }
    }

    pub fn info_refs(&self, project_path: &str, service: &str) -> GitResult<GitHttpResponse> {
        let service = GitProtocol::parse(service)?;
        self.http.info_refs(project_path, service)
    }

    pub fn upload_pack(&self, project_path: &str, body: &[u8]) -> GitResult<GitHttpResponse> {
        self.http
            .rpc(project_path, GitProtocol::UploadPack, body, None, None)
    }

    pub fn receive_pack(
        &self,
        project_path: &str,
        body: &[u8],
        actor: GitActor,
    ) -> GitResult<GitHttpResponse> {
        self.http.rpc(
            project_path,
            GitProtocol::ReceivePack,
            body,
            Some(actor),
            Some(&self.hooks),
        )
    }
}

fn git_root() -> PathBuf {
    std::env::var("ONEDEV_GIT_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("/var/lib/onedev/repos"))
}
