//! Git smart-HTTP (protocol v0/v2 ref advertisement + stateless RPC).
//!
//! The advertisement is generated from the gix-based repository service.
//! The pack transfer itself is delegated to `git upload-pack` /
//! `git receive-pack --stateless-rpc`, which is also how vanilla OneDev
//! handles parts of the protocol (via JGit plus the git executable).
//! Repositories are always opened with shell hooks disabled; the Rust
//! `HookEngine` is the only pre-receive gate.

use crate::errors::{GitError, GitResult};
use crate::models::{GitActor, GitConfig, GitHttpResponse, GitService, PushContext};
use crate::services::hooks::{push_context, HookEngine};
use crate::services::repository::RepositoryService;
use crate::utils::{encode_pkt_line, run_git};

const UPLOAD_CAPABILITIES: &str = "multi_ack_detailed no-done side-band-64k thin-pack ofs-delta deepen-since deepen-not object-format=sha1 agent=onedev-git";
const RECEIVE_CAPABILITIES: &str = "report-status report-status-v2 delete-refs side-band-64k quiet atomic ofs-delta push-options object-format=sha1 agent=onedev-git";
const ZERO_OID: &str = "0000000000000000000000000000000000000000";

/// Smart-HTTP operations for all projects under one storage root.
#[derive(Debug, Clone)]
pub struct SmartHttpService {
    config: GitConfig,
    repositories: RepositoryService,
}

impl SmartHttpService {
    pub fn new(config: GitConfig) -> Self {
        let repositories = RepositoryService::new(config.clone());
        SmartHttpService { config, repositories }
    }

    pub fn repositories(&self) -> &RepositoryService {
        &self.repositories
    }

    /// `GET info/refs?service=...`: pkt-line ref advertisement.
    pub fn info_refs(&self, project_path: &str, service: GitService) -> GitResult<GitHttpResponse> {
        let refs = self.repositories.list_refs(project_path)?;
        let capabilities = match service {
            GitService::UploadPack => UPLOAD_CAPABILITIES,
            GitService::ReceivePack => RECEIVE_CAPABILITIES,
        };

        let mut body = encode_pkt_line(format!("# service={}\n", service.as_str()).as_bytes());
        body.extend_from_slice(b"0000");

        if refs.is_empty() {
            let line = format!("{ZERO_OID} HEAD\0{capabilities}\n");
            body.extend_from_slice(&encode_pkt_line(line.as_bytes()));
        } else {
            for (index, reference) in refs.iter().enumerate() {
                let line = if index == 0 {
                    format!("{} {}\0{capabilities}\n", reference.object_id, reference.name)
                } else {
                    format!("{} {}\n", reference.object_id, reference.name)
                };
                body.extend_from_slice(&encode_pkt_line(line.as_bytes()));
            }
        }
        body.extend_from_slice(b"0000");

        Ok(GitHttpResponse::ok(service.advertisement_type(), body))
    }

    /// `POST git-upload-pack` / `git-receive-pack` with a request body.
    ///
    /// For receive-pack, `hooks` (when given) validates the ref updates
    /// before the pack is accepted and notifies listeners afterwards.
    pub fn rpc(
        &self,
        project_path: &str,
        service: GitService,
        body: &[u8],
        actor: Option<GitActor>,
        hooks: Option<&HookEngine>,
    ) -> GitResult<GitHttpResponse> {
        let repo = self.repositories.repository_path(project_path)?;
        if !repo.is_dir() {
            return Err(GitError::NotFound(repo.display().to_string()));
        }

        let push = match (service, hooks) {
            (GitService::ReceivePack, Some(engine)) => {
                let actor = actor.unwrap_or(GitActor {
                    id: "anonymous".to_owned(),
                    name: "anonymous".to_owned(),
                });
                let context: PushContext = push_context(project_path, actor, body)?;
                engine.pre_receive(&context)?;
                Some((engine, context))
            }
            _ => None,
        };

        let repo_arg = repo.display().to_string();
        let out = run_git(
            &self.config.git_binary,
            &repo,
            &[service.as_str(), "--stateless-rpc", &repo_arg],
            Some(body),
        )?;

        if let Some((engine, context)) = push {
            engine.post_receive(&context);
        }

        Ok(GitHttpResponse::ok(service.result_type(), out))
    }
}
