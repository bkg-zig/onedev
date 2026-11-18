//! API Router

use crate::handlers::*;
use crate::openapi::create_openapi;
use axum::response::Json;
use axum::{
    Router,
    routing::{delete, get, patch, post, put},
};
use std::sync::Arc;

/// Application state
#[derive(Clone)]
pub struct AppState {
    pub auth_service: Arc<crate::services::auth::AuthService>,
    pub authorization_service: Arc<crate::services::authz::AuthorizationService>,
    pub user_service: Arc<crate::services::UserService>,
    pub project_service: Arc<crate::services::ProjectService>,
    pub group_service: Arc<crate::services::GroupService>,
    pub issue_service: Arc<crate::services::IssueService>,
    pub mr_service: Arc<crate::services::MergeRequestService>,
    pub build_service: Arc<crate::services::BuildService>,
    pub git_service: Arc<crate::services::GitService>,
    pub lfs_service: Arc<crate::services::LfsService>,
    pub system_service: Arc<crate::services::SystemService>,
}

/// Create the main API router
pub fn create_router(state: AppState) -> Router {
    Router::new()
        // OpenAPI document (Swagger UI crate is incompatible with utoipa 5)
        .route(
            "/api-docs/openapi.json",
            get(|| async { Json(create_openapi()) }),
        )
        // Health & readiness
        .route("/health", get(system_handlers::health))
        .route("/ready", get(system_handlers::ready))
        .route("/version", get(system_handlers::version))
        // Auth routes (public)
        .route("/auth/login", post(auth_handlers::login))
        .route("/auth/verify-2fa", post(auth_handlers::verify_two_factor))
        .route("/auth/refresh", post(auth_handlers::refresh_token))
        // Protected routes (require authentication)
        .route("/auth/logout", post(auth_handlers::logout))
        .route("/auth/me", get(auth_handlers::current_user))
        // User routes
        .route("/users", get(user_handlers::list_users))
        .route("/users", post(user_handlers::create_user))
        .route("/users/{id}", get(user_handlers::get_user))
        .route("/users/{id}", patch(user_handlers::update_user))
        .route("/users/{id}", delete(user_handlers::delete_user))
        .route("/users/{id}/emails", get(user_handlers::list_emails))
        .route("/users/{id}/ssh-keys", get(user_handlers::list_ssh_keys))
        .route("/users/{id}/gpg-keys", get(user_handlers::list_gpg_keys))
        .route("/users/{id}/tokens", get(user_handlers::list_tokens))
        .route("/users/{id}/tokens", post(user_handlers::create_token))
        .route(
            "/users/{id}/tokens/{token_id}",
            delete(user_handlers::revoke_token),
        )
        // Project routes
        .route("/projects", get(project_handlers::list_projects))
        .route("/projects", post(project_handlers::create_project))
        .route("/projects/{id}", get(project_handlers::get_project))
        .route("/projects/{id}", patch(project_handlers::update_project))
        .route("/projects/{id}", delete(project_handlers::delete_project))
        .route("/projects/{id}/fork", post(project_handlers::fork_project))
        .route(
            "/projects/{id}/transfer",
            post(project_handlers::transfer_project),
        )
        .route("/projects/{id}/star", post(project_handlers::star_project))
        .route(
            "/projects/{id}/unstar",
            post(project_handlers::unstar_project),
        )
        .route(
            "/projects/{id}/members",
            get(project_handlers::list_members),
        )
        .route("/projects/{id}/members", post(project_handlers::add_member))
        .route(
            "/projects/{id}/members/{user_id}",
            get(project_handlers::get_member),
        )
        .route(
            "/projects/{id}/members/{user_id}",
            patch(project_handlers::update_member),
        )
        .route(
            "/projects/{id}/members/{user_id}",
            delete(project_handlers::remove_member),
        )
        // Group routes
        .route("/groups", get(group_handlers::list_groups))
        .route("/groups", post(group_handlers::create_group))
        .route("/groups/{id}", get(group_handlers::get_group))
        .route("/groups/{id}", patch(group_handlers::update_group))
        .route("/groups/{id}", delete(group_handlers::delete_group))
        .route(
            "/groups/{id}/members",
            get(group_handlers::list_group_members),
        )
        .route(
            "/groups/{id}/members",
            post(group_handlers::add_group_member),
        )
        .route(
            "/groups/{id}/members/{user_id}",
            get(group_handlers::get_group_member),
        )
        .route(
            "/groups/{id}/members/{user_id}",
            patch(group_handlers::update_group_member),
        )
        .route(
            "/groups/{id}/members/{user_id}",
            delete(group_handlers::remove_group_member),
        )
        // Issue routes
        .route(
            "/projects/{project_id}/issues",
            get(issue_handlers::list_issues),
        )
        .route(
            "/projects/{project_id}/issues",
            post(issue_handlers::create_issue),
        )
        .route(
            "/projects/{project_id}/issues/{id}",
            get(issue_handlers::get_issue),
        )
        .route(
            "/projects/{project_id}/issues/{id}",
            patch(issue_handlers::update_issue),
        )
        .route(
            "/projects/{project_id}/issues/{id}",
            delete(issue_handlers::delete_issue),
        )
        // Merge request routes
        .route(
            "/projects/{project_id}/merge-requests",
            get(mr_handlers::list_merge_requests),
        )
        .route(
            "/projects/{project_id}/merge-requests",
            post(mr_handlers::create_merge_request),
        )
        .route(
            "/projects/{project_id}/merge-requests/{id}",
            get(mr_handlers::get_merge_request),
        )
        .route(
            "/projects/{project_id}/merge-requests/{id}",
            patch(mr_handlers::update_merge_request),
        )
        .route(
            "/projects/{project_id}/merge-requests/{id}",
            delete(mr_handlers::delete_merge_request),
        )
        // Build routes
        .route(
            "/projects/{project_id}/builds",
            get(build_handlers::list_builds),
        )
        .route(
            "/projects/{project_id}/builds",
            post(build_handlers::trigger_build),
        )
        .route(
            "/projects/{project_id}/builds/{id}",
            get(build_handlers::get_build),
        )
        // Git Smart HTTP routes
        .route(
            "/git/{project_path}/info/refs",
            get(git_handlers::info_refs),
        )
        .route(
            "/git/{project_path}/git-upload-pack",
            post(git_handlers::git_upload_pack),
        )
        .route(
            "/git/{project_path}/git-receive-pack",
            post(git_handlers::git_receive_pack),
        )
        // LFS routes
        .route(
            "/git/{project_path}/info/lfs/objects/batch",
            post(lfs_handlers::lfs_batch),
        )
        .route(
            "/git/{project_path}/info/lfs/objects/{oid}",
            get(lfs_handlers::lfs_download),
        )
        .route(
            "/git/{project_path}/info/lfs/objects/{oid}",
            put(lfs_handlers::lfs_upload),
        )
        .route(
            "/git/{project_path}/info/lfs/locks",
            get(lfs_handlers::lfs_locks),
        )
        .route(
            "/git/{project_path}/info/lfs/locks",
            post(lfs_handlers::lfs_create_lock),
        )
        .route(
            "/git/{project_path}/info/lfs/locks/{id}",
            delete(lfs_handlers::lfs_unlock),
        )
        .with_state(state)
}
