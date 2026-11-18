//! OneDev SSR Web UI (Tera + dx)

pub mod config;
pub mod handlers;
pub mod models;
pub mod routes;
pub mod services;
pub mod templates;
pub mod utils;

// Explicit re-exports to avoid ambiguous glob re-exports
pub use config::WebConfig;
pub use handlers::admin;
pub use handlers::auth;
pub use handlers::builds;
pub use handlers::dashboard;
pub use handlers::git;
pub use handlers::issues;
pub use handlers::merge_requests;
pub use handlers::profile;
pub use handlers::projects;
pub use models::admin::*;
pub use models::auth::User;
pub use models::builds::{Build, BuildStatus, Pipeline};
pub use models::dashboard::{DashboardStats, ActivityItem, UserProject};
pub use models::git::{Branch, CommitRef, TreeEntry, Tag, Commit};
pub use models::issues::{Issue, IssueComment, Label, Milestone};
pub use models::merge_requests::MergeRequest;
pub use models::profile::{UserProfile, SshKey, GpgKey, AccessToken, Session, UpdateSettingsForm};
pub use models::projects::{Project, ProjectMember};
pub use services::*;
pub use templates::*;
pub use utils::*;

pub use anyhow::Result;
pub use chrono::{DateTime, Utc};
pub use serde::{Deserialize, Serialize};
pub use thiserror::Error;
/// Re-export commonly used types
pub use uuid::Uuid;

use axum::Router;
use onedev_api::AppState as ApiState;
use std::sync::Arc;

/// Web application state
#[derive(Clone)]
pub struct WebState {
    pub api_state: ApiState,
    pub config: Arc<WebConfig>,
    pub tera: Arc<tera::Tera>,
}

impl WebState {
    pub async fn new(config: WebConfig) -> anyhow::Result<Self> {
        // Initialize Tera template engine
        let template_dir = config.templates.template_dir.to_string_lossy();
        let mut tera = tera::Tera::new(&format!("{}/**/*", template_dir))?;
        tera.autoescape_on(vec![".html", ".htm"]);

        // Add custom filters
        tera.register_filter("markdown", markdown_filter);
        tera.register_filter("datefmt", date_filter);
        tera.register_filter("truncate", truncate_filter);

        // Create API state (would connect to actual API)
        // For now, create a minimal state
        let api_state = create_minimal_api_state().await?;

        Ok(Self {
            api_state,
            config: Arc::new(config),
            tera: Arc::new(tera),
        })
    }
}

/// Create minimal API state for web UI
async fn create_minimal_api_state() -> anyhow::Result<ApiState> {
    // This would normally connect to the API services
    // For now, return a placeholder
    Err(anyhow::anyhow!("API state not implemented for web UI"))
}

/// Create web router
pub fn create_web_router(state: WebState) -> Router<WebState> {
    let router = Router::new()
        .route(
            "/",
            axum::routing::get(handlers::dashboard::dashboard_handler),
        )
        .route("/login", axum::routing::get(handlers::auth::login_page))
        .route("/login", axum::routing::post(handlers::auth::login_handler))
        .route(
            "/logout",
            axum::routing::post(handlers::auth::logout_handler),
        )
        .route(
            "/register",
            axum::routing::get(handlers::auth::register_page),
        )
        .route(
            "/register",
            axum::routing::post(handlers::auth::register_handler),
        )
        // Projects
        .route(
            "/projects",
            axum::routing::get(handlers::projects::list_projects),
        )
        .route(
            "/projects/new",
            axum::routing::get(handlers::projects::new_project_page),
        )
        .route(
            "/projects/new",
            axum::routing::post(handlers::projects::create_project),
        )
        .route(
            "/projects/:id",
            axum::routing::get(handlers::projects::project_detail),
        )
        .route(
            "/projects/:id/settings",
            axum::routing::get(handlers::projects::project_settings),
        )
        .route(
            "/projects/:id/settings",
            axum::routing::post(handlers::projects::update_project),
        )
        // Issues
        .route(
            "/projects/:project_id/issues",
            axum::routing::get(handlers::issues::list_issues),
        )
        .route(
            "/projects/:project_id/issues/new",
            axum::routing::get(handlers::issues::new_issue_page),
        )
        .route(
            "/projects/:project_id/issues/new",
            axum::routing::post(handlers::issues::create_issue),
        )
        .route(
            "/projects/:project_id/issues/:id",
            axum::routing::get(handlers::issues::issue_detail),
        )
        .route(
            "/projects/:project_id/issues/:id/edit",
            axum::routing::get(handlers::issues::edit_issue_page),
        )
        .route(
            "/projects/:project_id/issues/:id/edit",
            axum::routing::post(handlers::issues::update_issue),
        )
        // Merge Requests
        .route(
            "/projects/:project_id/merge-requests",
            axum::routing::get(handlers::merge_requests::list_mrs),
        )
        .route(
            "/projects/:project_id/merge-requests/new",
            axum::routing::get(handlers::merge_requests::new_mr_page),
        )
        .route(
            "/projects/:project_id/merge-requests/new",
            axum::routing::post(handlers::merge_requests::create_mr),
        )
        .route(
            "/projects/:project_id/merge-requests/:id",
            axum::routing::get(handlers::merge_requests::mr_detail),
        )
        // Builds
        .route(
            "/projects/:project_id/builds",
            axum::routing::get(handlers::builds::list_builds),
        )
        .route(
            "/projects/:project_id/builds/:id",
            axum::routing::get(handlers::builds::build_detail),
        )
        // Git
        .route(
            "/projects/:project_id/git",
            axum::routing::get(handlers::git::git_overview),
        )
        .route(
            "/projects/:project_id/git/tree/:ref",
            axum::routing::get(handlers::git::git_tree),
        )
        .route(
            "/projects/:project_id/git/blob/:ref/*path",
            axum::routing::get(handlers::git::git_blob),
        )
        .route(
            "/projects/:project_id/git/commits",
            axum::routing::get(handlers::git::git_commits),
        )
        .route(
            "/projects/:project_id/git/commits/:sha",
            axum::routing::get(handlers::git::git_commit),
        )
        // Profile
        .route(
            "/profile",
            axum::routing::get(handlers::profile::profile_page),
        )
        .route(
            "/profile/settings",
            axum::routing::get(handlers::profile::settings_page),
        )
        .route(
            "/profile/settings",
            axum::routing::post(handlers::profile::update_settings),
        )
        // Admin
        .route(
            "/admin",
            axum::routing::get(handlers::admin::admin_dashboard),
        )
        .route(
            "/admin/users",
            axum::routing::get(handlers::admin::list_users),
        )
        .route(
            "/admin/projects",
            axum::routing::get(handlers::admin::list_all_projects),
        )
        .route(
            "/admin/system",
            axum::routing::get(handlers::admin::system_info),
        )
        // Static files
        .nest_service(
            "/static",
            tower_http::services::ServeDir::new(&state.config.static_assets.static_dir),
        )
        // Health
        .route("/health", axum::routing::get(|| async { "OK" }))
        // Add middleware
        .layer(tower_http::trace::TraceLayer::new_for_http())
        .layer(tower_http::compression::CompressionLayer::new())
        .with_state(state);

    router
}

// Template filters
pub fn markdown_filter(
    value: &tera::Value,
    _: &std::collections::HashMap<String, tera::Value>,
) -> tera::Result<tera::Value> {
    use pulldown_cmark::{Options, Parser, html};
    let mut options = Options::empty();
    options.insert(Options::ENABLE_STRIKETHROUGH);
    options.insert(Options::ENABLE_TABLES);
    options.insert(Options::ENABLE_FOOTNOTES);
    options.insert(Options::ENABLE_TASKLISTS);

    if let Some(text) = value.as_str() {
        let parser = Parser::new_ext(text, options);
        let mut html_output = String::new();
        html::push_html(&mut html_output, parser);
        Ok(tera::Value::String(html_output))
    } else {
        Ok(tera::Value::String("".to_string()))
    }
}

pub fn date_filter(
    value: &tera::Value,
    args: &std::collections::HashMap<String, tera::Value>,
) -> tera::Result<tera::Value> {
    use chrono::{DateTime, TimeZone, Utc};

    let format = args
        .get("format")
        .and_then(|v| v.as_str())
        .unwrap_or("%Y-%m-%d %H:%M:%S");

    if let Some(timestamp) = value.as_str() {
        if let Ok(dt) = DateTime::parse_from_rfc3339(timestamp) {
            return Ok(tera::Value::String(
                dt.with_timezone(&Utc).format(format).to_string(),
            ));
        }
        if let Ok(dt) = timestamp.parse::<i64>() {
            if let Some(dt) = Utc.timestamp_opt(dt, 0).single() {
                return Ok(tera::Value::String(dt.format(format).to_string()));
            }
        }
    } else if let Some(timestamp) = value.as_i64() {
        if let Some(dt) = Utc.timestamp_opt(timestamp, 0).single() {
            return Ok(tera::Value::String(dt.format(format).to_string()));
        }
    }

    Ok(tera::Value::String("".to_string()))
}

pub fn truncate_filter(
    value: &tera::Value,
    args: &std::collections::HashMap<String, tera::Value>,
) -> tera::Result<tera::Value> {
    let length = args.get("length").and_then(|v| v.as_u64()).unwrap_or(100) as usize;

    if let Some(text) = value.as_str() {
        if text.len() > length {
            let truncated = &text[..length];
            let last_space = truncated.rfind(' ').unwrap_or(length);
            return Ok(tera::Value::String(format!(
                "{}...",
                &truncated[..last_space]
            )));
        }
        return Ok(tera::Value::String(text.to_string()));
    }

    Ok(tera::Value::String("".to_string()))
}
