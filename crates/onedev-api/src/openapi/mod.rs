//! OpenAPI Documentation Setup

use std::collections::BTreeMap;
use utoipa::openapi::OpenApi;
use utoipa::openapi::path::{HttpMethod, Operation, PathItemBuilder, PathsBuilder};
use utoipa::openapi::security::{ApiKey, ApiKeyValue, SecurityScheme};
use utoipa::{Modify, PartialSchema};

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

fn build_paths() -> utoipa::openapi::Paths {
    // (path, method) pairs mirroring `routes::create_router`
    let routes: &[(&str, HttpMethod)] = &[
        ("/auth/login", HttpMethod::Post),
        ("/auth/verify-2fa", HttpMethod::Post),
        ("/auth/logout", HttpMethod::Post),
        ("/auth/me", HttpMethod::Get),
        ("/auth/refresh", HttpMethod::Post),
        ("/users", HttpMethod::Get),
        ("/users", HttpMethod::Post),
        ("/users/{id}", HttpMethod::Get),
        ("/users/{id}", HttpMethod::Patch),
        ("/users/{id}", HttpMethod::Delete),
        ("/users/{id}/emails", HttpMethod::Get),
        ("/users/{id}/ssh-keys", HttpMethod::Get),
        ("/users/{id}/gpg-keys", HttpMethod::Get),
        ("/users/{id}/tokens", HttpMethod::Get),
        ("/users/{id}/tokens", HttpMethod::Post),
        ("/projects", HttpMethod::Get),
        ("/projects", HttpMethod::Post),
        ("/projects/{id}", HttpMethod::Get),
        ("/projects/{id}", HttpMethod::Patch),
        ("/projects/{id}", HttpMethod::Delete),
        ("/projects/{id}/fork", HttpMethod::Post),
        ("/projects/{id}/transfer", HttpMethod::Post),
        ("/projects/{id}/star", HttpMethod::Post),
        ("/projects/{id}/unstar", HttpMethod::Post),
        ("/projects/{id}/members", HttpMethod::Get),
        ("/projects/{id}/members", HttpMethod::Post),
        ("/projects/{id}/members/{user_id}", HttpMethod::Get),
        ("/projects/{id}/members/{user_id}", HttpMethod::Patch),
        ("/projects/{id}/members/{user_id}", HttpMethod::Delete),
        ("/groups", HttpMethod::Get),
        ("/groups", HttpMethod::Post),
        ("/groups/{id}", HttpMethod::Get),
        ("/groups/{id}", HttpMethod::Patch),
        ("/groups/{id}", HttpMethod::Delete),
        ("/groups/{id}/members", HttpMethod::Get),
        ("/groups/{id}/members", HttpMethod::Post),
        ("/groups/{id}/members/{user_id}", HttpMethod::Get),
        ("/groups/{id}/members/{user_id}", HttpMethod::Patch),
        ("/groups/{id}/members/{user_id}", HttpMethod::Delete),
        ("/projects/{project_id}/issues", HttpMethod::Get),
        ("/projects/{project_id}/issues", HttpMethod::Post),
        ("/projects/{project_id}/issues/{id}", HttpMethod::Get),
        ("/projects/{project_id}/issues/{id}", HttpMethod::Patch),
        ("/projects/{project_id}/issues/{id}", HttpMethod::Delete),
        ("/projects/{project_id}/merge-requests", HttpMethod::Get),
        ("/projects/{project_id}/merge-requests", HttpMethod::Post),
        (
            "/projects/{project_id}/merge-requests/{id}",
            HttpMethod::Get,
        ),
        (
            "/projects/{project_id}/merge-requests/{id}",
            HttpMethod::Patch,
        ),
        (
            "/projects/{project_id}/merge-requests/{id}",
            HttpMethod::Delete,
        ),
        ("/projects/{project_id}/builds", HttpMethod::Get),
        ("/projects/{project_id}/builds", HttpMethod::Post),
        ("/projects/{project_id}/builds/{id}", HttpMethod::Get),
        ("/git/{project_path}/info/refs", HttpMethod::Get),
        ("/git/{project_path}/git-upload-pack", HttpMethod::Post),
        ("/git/{project_path}/git-receive-pack", HttpMethod::Post),
        (
            "/git/{project_path}/info/lfs/objects/batch",
            HttpMethod::Post,
        ),
        (
            "/git/{project_path}/info/lfs/objects/{oid}",
            HttpMethod::Get,
        ),
        (
            "/git/{project_path}/info/lfs/objects/{oid}",
            HttpMethod::Put,
        ),
        ("/git/{project_path}/info/lfs/locks", HttpMethod::Get),
        ("/git/{project_path}/info/lfs/locks", HttpMethod::Post),
        (
            "/git/{project_path}/info/lfs/locks/{id}",
            HttpMethod::Delete,
        ),
        ("/health", HttpMethod::Get),
        ("/ready", HttpMethod::Get),
        ("/version", HttpMethod::Get),
    ];

    let mut by_path: BTreeMap<&str, PathItemBuilder> = BTreeMap::new();
    for (path, method) in routes {
        let builder = by_path.entry(path).or_default();
        *builder = std::mem::take(builder).operation(method.clone(), Operation::default());
    }

    let mut paths = PathsBuilder::new();
    for (path, item) in by_path {
        paths = paths.path(path, item.build());
    }
    paths.build()
}

/// Create OpenAPI documentation
pub fn create_openapi() -> utoipa::openapi::OpenApi {
    let mut api = utoipa::openapi::OpenApiBuilder::new()
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
        .servers(Some(vec![
            utoipa::openapi::ServerBuilder::new()
                .url("/api")
                .description(Some("API Server"))
                .build(),
        ]))
        .paths(build_paths())
        .components(Some(
            utoipa::openapi::ComponentsBuilder::new()
                .schemas_from_iter([
                    ("UserRef", crate::models::users::UserRef::schema()),
                    ("UserResponse", crate::models::users::UserResponse::schema()),
                    ("UserListItem", crate::models::users::UserListItem::schema()),
                    ("CreateUserRequest", crate::models::users::CreateUserRequest::schema()),
                    ("UpdateUserRequest", crate::models::users::UpdateUserRequest::schema()),
                    ("ChangePasswordRequest", crate::models::users::ChangePasswordRequest::schema()),
                    ("UserQuery", crate::models::users::UserQuery::schema()),
                    ("ProjectRef", crate::models::projects::ProjectRef::schema()),
                    ("ProjectResponse", crate::models::projects::ProjectResponse::schema()),
                    ("ProjectListItem", crate::models::projects::ProjectListItem::schema()),
                    ("CreateProjectRequest", crate::models::projects::CreateProjectRequest::schema()),
                    ("UpdateProjectRequest", crate::models::projects::UpdateProjectRequest::schema()),
                    ("ProjectQuery", crate::models::projects::ProjectQuery::schema()),
                    ("ForkProjectRequest", crate::models::projects::ForkProjectRequest::schema()),
                    ("TransferProjectRequest", crate::models::projects::TransferProjectRequest::schema()),
                    ("GroupRef", crate::models::groups::GroupRef::schema()),
                    ("GroupResponse", crate::models::groups::GroupResponse::schema()),
                    ("GroupListItem", crate::models::groups::GroupListItem::schema()),
                    ("CreateGroupRequest", crate::models::groups::CreateGroupRequest::schema()),
                    ("UpdateGroupRequest", crate::models::groups::UpdateGroupRequest::schema()),
                    ("GroupQuery", crate::models::groups::GroupQuery::schema()),
                    ("AccessTokenWithRawResponse", crate::models::access_tokens::AccessTokenWithRawResponse::schema()),
                    ("CreateAccessTokenRequest", crate::models::access_tokens::CreateAccessTokenRequest::schema()),
                    ("UpdateAccessTokenRequest", crate::models::access_tokens::UpdateAccessTokenRequest::schema()),
                    ("AccessTokenQuery", crate::models::access_tokens::AccessTokenQuery::schema()),
                    ("CursorPagination", crate::models::pagination::CursorPagination::schema()),
                    ("OffsetPagination", crate::models::pagination::OffsetPagination::schema()),
                    ("PaginationMeta", crate::models::pagination::PaginationMeta::schema()),
                    ("PaginatedResponse", crate::models::pagination::PaginatedResponse::<crate::models::users::UserRef>::schema()),
                    ("ProblemDetails", crate::errors::ProblemDetails::schema()),
                ])
                .build()
        ))
        .security(Some(vec![
            utoipa::openapi::security::SecurityRequirement::new(
                "bearer_auth",
                Vec::<String>::new(),
            )
        ]))
        .build();

    SecurityAddon.modify(&mut api);
    api
}
