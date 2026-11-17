//! Framework-independent smart-HTTP handlers.
//!
//! The HTTP layer (axum in `onedev-api`) maps requests onto these functions
//! and writes the returned `GitHttpResponse` verbatim.

use crate::errors::GitResult;
use crate::models::{GitActor, GitHttpResponse, GitService};
use crate::services::hooks::HookEngine;
use crate::services::smart_http::SmartHttpService;

/// `GET /{project}/info/refs?service=git-upload-pack|git-receive-pack`
pub fn info_refs(
    service: &SmartHttpService,
    project_path: &str,
    service_name: &str,
) -> GitResult<GitHttpResponse> {
    service.info_refs(project_path, GitService::parse(service_name)?)
}

/// `POST /{project}/git-upload-pack`
pub fn git_upload_pack(
    service: &SmartHttpService,
    project_path: &str,
    body: &[u8],
) -> GitResult<GitHttpResponse> {
    service.rpc(project_path, GitService::UploadPack, body, None, None)
}

/// `POST /{project}/git-receive-pack`
pub fn git_receive_pack(
    service: &SmartHttpService,
    project_path: &str,
    body: &[u8],
    actor: Option<GitActor>,
    hooks: Option<&HookEngine>,
) -> GitResult<GitHttpResponse> {
    service.rpc(project_path, GitService::ReceivePack, body, actor, hooks)
}
