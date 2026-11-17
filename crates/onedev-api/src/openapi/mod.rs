//! OpenAPI Documentation Setup

use utoipa::openapi::security::{ApiKey, ApiKeyValue, SecurityScheme};
use utoipa::Modify;
use utoipa::openapi::OpenApi;

/// Add Bearer token security scheme to OpenAPI
pub struct SecurityAddon;

impl Modify for SecurityAddon {
    fn modify(&self, openapi: &mut OpenApi) {
        if let Some(components) = openapi.components.as_mut() {
            components.add_security_scheme(
                "bearer_auth",
                SecurityScheme::ApiKey(ApiKey::Header(ApiKeyValue::new("Authorization"))),
            );
            
            components.add_security_scheme(
                "cookie_auth",
                SecurityScheme::ApiKey(ApiKey::Cookie(ApiKeyValue::new("access_token"))),
            );
        }
    }
}

/// Create OpenAPI documentation
pub fn create_openapi() -> utoipa::openapi::OpenApi {
    use crate::handlers::*;
    
    utoipa::openapi::OpenApiBuilder::new()
        .info(
            utoipa::openapi::InfoBuilder::new()
                .title("OneDev API")
                .version(env!("CARGO_PKG_VERSION"))
                .description(Some("OneDev Rust Rewrite - Complete Git Server with CI/CD, Issue Tracking, Code Review"))
                .contact(Some(utoipa::openapi::ContactBuilder::new()
                    .name(Some("BKG Team"))
                    .url(Some("https://github.com/bkg-zig/onedev"))
                    .build()))
                .license(Some(utoipa::openapi::LicenseBuilder::new()
                    .name("Apache-2.0")
                    .url(Some("https://www.apache.org/licenses/LICENSE-2.0"))
                    .build()))
                .build(),
        )
        .servers(vec![
            utoipa::openapi::ServerBuilder::new()
                .url("/api")
                .description("API Server")
                .build(),
        ])
        .paths(
            utoipa::openapi::path::PathBuilder::new()
                // Auth endpoints
                .path("/auth/login", post(auth_handlers::login))
                .path("/auth/verify-2fa", post(auth_handlers::verify_two_factor))
                .path("/auth/logout", post(auth_handlers::logout))
                .path("/auth/me", get(auth_handlers::current_user))
                .path("/auth/refresh", post(auth_handlers::refresh_token))
                // User endpoints
                .path("/users", get(user_handlers::list_users))
                .path("/users/{id}", get(user_handlers::get_user))
                .path("/users", post(user_handlers::create_user))
                .path("/users/{id}", patch(user_handlers::update_user))
                .path("/users/{id}", delete(user_handlers::delete_user))
                .path("/users/{id}/emails", get(user_handlers::list_emails))
                .path("/users/{id}/ssh-keys", get(user_handlers::list_ssh_keys))
                .path("/users/{id}/gpg-keys", get(user_handlers::list_gpg_keys))
                .path("/users/{id}/tokens", get(user_handlers::list_tokens))
                .path("/users/{id}/tokens", post(user_handlers::create_token))
                // Project endpoints
                .path("/projects", get(project_handlers::list_projects))
                .path("/projects/{id}", get(project_handlers::get_project))
                .path("/projects", post(project_handlers::create_project))
                .path("/projects/{id}", patch(project_handlers::update_project))
                .path("/projects/{id}", delete(project_handlers::delete_project))
                .path("/projects/{id}/fork", post(project_handlers::fork_project))
                .path("/projects/{id}/transfer", post(project_handlers::transfer_project))
                .path("/projects/{id}/star", post(project_handlers::star_project))
                .path("/projects/{id}/unstar", post(project_handlers::unstar_project))
                .path("/projects/{id}/members", get(project_handlers::list_members))
                .path("/projects/{id}/members", post(project_handlers::add_member))
                .path("/projects/{id}/members/{user_id}", get(project_handlers::get_member))
                .path("/projects/{id}/members/{user_id}", patch(project_handlers::update_member))
                .path("/projects/{id}/members/{user_id}", delete(project_handlers::remove_member))
                // Group endpoints
                .path("/groups", get(group_handlers::list_groups))
                .path("/groups/{id}", get(group_handlers::get_group))
                .path("/groups", post(group_handlers::create_group))
                .path("/groups/{id}", patch(group_handlers::update_group))
                .path("/groups/{id}", delete(group_handlers::delete_group))
                .path("/groups/{id}/members", get(group_handlers::list_group_members))
                .path("/groups/{id}/members", post(group_handlers::add_group_member))
                .path("/groups/{id}/members/{user_id}", get(group_handlers::get_group_member))
                .path("/groups/{id}/members/{user_id}", patch(group_handlers::update_group_member))
                .path("/groups/{id}/members/{user_id}", delete(group_handlers::remove_group_member))
                // Issue endpoints
                .path("/projects/{project_id}/issues", get(issue_handlers::list_issues))
                .path("/projects/{project_id}/issues/{id}", get(issue_handlers::get_issue))
                .path("/projects/{project_id}/issues", post(issue_handlers::create_issue))
                .path("/projects/{project_id}/issues/{id}", patch(issue_handlers::update_issue))
                .path("/projects/{project_id}/issues/{id}", delete(issue_handlers::delete_issue))
                // Merge request endpoints
                .path("/projects/{project_id}/merge-requests", get(mr_handlers::list_merge_requests))
                .path("/projects/{project_id}/merge-requests/{id}", get(mr_handlers::get_merge_request))
                .path("/projects/{project_id}/merge-requests", post(mr_handlers::create_merge_request))
                .path("/projects/{project_id}/merge-requests/{id}", patch(mr_handlers::update_merge_request))
                .path("/projects/{project_id}/merge-requests/{id}", delete(mr_handlers::delete_merge_request))
                // Build endpoints
                .path("/projects/{project_id}/builds", get(build_handlers::list_builds))
                .path("/projects/{project_id}/builds/{id}", get(build_handlers::get_build))
                .path("/projects/{project_id}/builds", post(build_handlers::trigger_build))
                // Git Smart HTTP endpoints
                .path("/git/{project_path}/info/refs", get(git_handlers::info_refs))
                .path("/git/{project_path}/git-upload-pack", post(git_handlers::git_upload_pack))
                .path("/git/{project_path}/git-receive-pack", post(git_handlers::git_receive_pack))
                // LFS endpoints
                .path("/git/{project_path}/info/lfs/objects/batch", post(lfs_handlers::lfs_batch))
                .path("/git/{project_path}/info/lfs/objects/{oid}", get(lfs_handlers::lfs_download))
                .path("/git/{project_path}/info/lfs/objects/{oid}", put(lfs_handlers::lfs_upload))
                .path("/git/{project_path}/info/lfs/locks", get(lfs_handlers::lfs_locks))
                .path("/git/{project_path}/info/lfs/locks", post(lfs_handlers::lfs_create_lock))
                .path("/git/{project_path}/info/lfs/locks/{id}", delete(lfs_handlers::lfs_unlock))
                // System endpoints
                .path("/health", get(system_handlers::health))
                .path("/ready", get(system_handlers::ready))
                .path("/version", get(system_handlers::version))
                .build(),
        )
        .components(Some(
            utoipa::openapi::ComponentsBuilder::new()
                .schemas_from_iter([
                    // Auth
                    crate::models::users::UserRef::schema(),
                    crate::models::users::UserResponse::schema(),
                    crate::models::users::UserListItem::schema(),
                    crate::models::users::CreateUserRequest::schema(),
                    crate::models::users::UpdateUserRequest::schema(),
                    crate::models::users::ChangePasswordRequest::schema(),
                    crate::models::users::UserQuery::schema(),
                    // Projects
                    crate::models::projects::ProjectRef::schema(),
                    crate::models::projects::ProjectResponse::schema(),
                    crate::models::projects::ProjectListItem::schema(),
                    crate::models::projects::CreateProjectRequest::schema(),
                    crate::models::projects::UpdateProjectRequest::schema(),
                    crate::models::projects::ProjectQuery::schema(),
                    crate::models::projects::ForkProjectRequest::schema(),
                    crate::models::projects::TransferProjectRequest::schema(),
                    // Groups
                    crate::models::groups::GroupRef::schema(),
                    crate::models::groups::GroupResponse::schema(),
                    crate::models::groups::GroupListItem::schema(),
                    crate::models::groups::CreateGroupRequest::schema(),
                    crate::models::groups::UpdateGroupRequest::schema(),
                    crate::models::groups::GroupQuery::schema(),
                    // Access tokens
                    crate::models::access_tokens::AccessTokenWithRawResponse::schema(),
                    crate::models::access_tokens::CreateAccessTokenRequest::schema(),
                    crate::models::access_tokens::UpdateAccessTokenRequest::schema(),
                    crate::models::access_tokens::AccessTokenQuery::schema(),
                    // Pagination
                    crate::models::pagination::CursorPagination::schema(),
                    crate::models::pagination::OffsetPagination::schema(),
                    crate::models::pagination::PaginationMeta::schema(),
                    crate::models::pagination::PaginatedResponse::<crate::models::users::UserRef>::schema(),
                    // Problem details
                    crate::errors::ProblemDetails::schema(),
                ])
                .build()
        ))
        .security(vec![
            utoipa::openapi::security::SecurityScheme::ApiKey(
                utoipa::openapi::security::ApiKey::Header(utoipa::openapi::security::ApiKeyValue::new("Authorization"))
            )
        ])
        .modifier(&SecurityAddon)
        .build()
}