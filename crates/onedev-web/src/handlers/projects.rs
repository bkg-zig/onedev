//! Project Handlers

use crate::WebState;
use axum::{
    Form,
    extract::{Path, Query, State},
    response::{Html, IntoResponse, Redirect},
};
use serde::{Deserialize, Serialize};
use tera::Context;

/// List projects
pub async fn list_projects(
    State(state): State<WebState>,
    Query(params): Query<ProjectListParams>,
) -> impl IntoResponse {
    let mut context = Context::new();
    context.insert("title", "Projects");
    context.insert("projects", &get_projects(&state, &params).await);
    context.insert("params", &params);
    context.insert("user", &get_current_user(&state).await);
    render_template(&state, "projects/list.html", context).await
}

/// New project page
pub async fn new_project_page(State(state): State<WebState>) -> impl IntoResponse {
    let mut context = Context::new();
    context.insert("title", "New Project");
    context.insert("user", &get_current_user(&state).await);
    render_template(&state, "projects/new.html", context).await
}

/// Create project
pub async fn create_project(
    State(state): State<WebState>,
    Form(form): Form<CreateProjectForm>,
) -> impl IntoResponse {
    // Validate
    if form.name.trim().is_empty() {
        let mut context = Context::new();
        context.insert("title", "New Project");
        context.insert("error", "Project name is required");
        context.insert("form", &form);
        return render_template(&state, "projects/new.html", context)
            .await
            .into_response();
    }

    // Create project (placeholder)
    // Redirect to project
    Redirect::to(&format!("/projects/{}", slugify(&form.name))).into_response()
}

/// Project detail
pub async fn project_detail(
    State(state): State<WebState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let mut context = Context::new();
    context.insert("title", &format!("Project: {}", id));
    context.insert("project", &get_project(&state, &id).await);
    context.insert("user", &get_current_user(&state).await);
    render_template(&state, "projects/detail.html", context).await
}

/// Project settings
pub async fn project_settings(
    State(state): State<WebState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let mut context = Context::new();
    context.insert("title", &format!("Settings: {}", id));
    context.insert("project", &get_project(&state, &id).await);
    context.insert("user", &get_current_user(&state).await);
    render_template(&state, "projects/settings.html", context).await
}

/// Update project
pub async fn update_project(
    _state: State<WebState>,
    Path(id): Path<String>,
    _form: Form<UpdateProjectForm>,
) -> impl IntoResponse {
    // Update project (placeholder)
    Redirect::to(&format!("/projects/{}/settings?saved=true", id)).into_response()
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ProjectListParams {
    pub page: Option<u32>,
    pub per_page: Option<u32>,
    pub search: Option<String>,
    pub visibility: Option<String>,
    pub sort: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct CreateProjectForm {
    pub name: String,
    pub description: Option<String>,
    pub visibility: String,
    pub initialize_with_readme: bool,
    pub template: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct UpdateProjectForm {
    pub name: String,
    pub description: Option<String>,
    pub visibility: String,
    pub default_branch: String,
    pub merge_requests_enabled: bool,
    pub issues_enabled: bool,
    pub wiki_enabled: bool,
    pub builds_enabled: bool,
}

/// Get projects list
/// Live project list from the database when configured, otherwise the
/// dev placeholder rows. `search` is a plain substring filter for now;
/// the full vanilla query language is a later slice (see report 04).
async fn get_projects(state: &WebState, params: &ProjectListParams) -> Vec<serde_json::Value> {
    if let Some(pool) = &state.db_pool {
        let service = onedev_api::services::ProjectService::new(pool.clone());
        let per_page = params.per_page.unwrap_or(25).min(100).max(1) as i64;
        let page = params.page.unwrap_or(0) as i64;
        if let Ok(rows) = service.list(per_page, page * per_page).await {
            let needle = params.search.as_deref().unwrap_or("").to_lowercase();
            return rows
                .into_iter()
                .filter(|item| {
                    needle.is_empty()
                        || item.project.name.to_lowercase().contains(&needle)
                        || item.project.path.to_lowercase().contains(&needle)
                })
                .map(|item| {
                    serde_json::json!({
                        "id": item.project.id,
                        "name": item.project.name,
                        "path": item.project.path,
                        "description": item.project.description,
                        "visibility": format!("{:?}", item.project.visibility).to_lowercase(),
                        "stars": item.project.star_count,
                        "forks": item.project.fork_count,
                        // Counters come from ProjectResponse, not the list
                        // item; wired once the stats query lands.
                        "open_issues": 0,
                        "open_mrs": 0,
                        "last_activity": item.last_activity_at,
                        "owner": { "username": item.owner.name, "avatar_url": item.owner.avatar_url }
                    })
                })
                .collect();
        }
    }
    vec![
        serde_json::json!({
            "id": 1,
            "name": "onedev/core",
            "path": "onedev/core",
            "description": "Core OneDev functionality",
            "visibility": "private",
            "stars": 10,
            "forks": 2,
            "open_issues": 5,
            "open_mrs": 2,
            "last_activity": "2024-01-15T10:30:00Z",
            "owner": { "username": "admin", "avatar_url": null }
        }),
        serde_json::json!({
            "id": 2,
            "name": "onedev/web",
            "path": "onedev/web",
            "description": "Web UI for OneDev",
            "visibility": "public",
            "stars": 25,
            "forks": 5,
            "open_issues": 3,
            "open_mrs": 1,
            "last_activity": "2024-01-15T09:15:00Z",
            "owner": { "username": "admin", "avatar_url": null }
        }),
    ]
}

/// Get single project
async fn get_project(_state: &WebState, id: &str) -> serde_json::Value {
    serde_json::json!({
        "id": id,
        "name": "onedev/core",
        "path": "onedev/core",
        "description": "Core OneDev functionality",
        "visibility": "private",
        "stars": 10,
        "forks": 2,
        "open_issues": 5,
        "open_mrs": 2,
        "default_branch": "main",
        "merge_requests_enabled": true,
        "issues_enabled": true,
        "wiki_enabled": true,
        "builds_enabled": true,
        "created_at": "2024-01-01T00:00:00Z",
        "updated_at": "2024-01-15T10:30:00Z",
        "owner": { "id": 1, "username": "admin", "full_name": "Admin User", "avatar_url": null },
        "members": [
            { "user": { "username": "admin", "full_name": "Admin User" }, "role": "owner" },
            { "user": { "username": "john", "full_name": "John Doe" }, "role": "developer" }
        ]
    })
}

/// Get current user (placeholder)
async fn get_current_user(_state: &WebState) -> serde_json::Value {
    serde_json::json!({
        "id": "1",
        "username": "admin",
        "full_name": "Admin User",
        "avatar_url": null,
        "email": "admin@example.com"
    })
}

/// Slugify string for URLs
fn slugify(s: &str) -> String {
    s.to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '-' })
        .collect::<String>()
        .split('-')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}

/// Render template helper
async fn render_template(state: &WebState, template: &str, context: Context) -> Html<String> {
    match state.tera.render(template, &context) {
        Ok(html) => Html(html),
        Err(e) => {
            tracing::error!("Template rendering failed: {}", e);
            Html(format!("<h1>Template Error</h1><pre>{}</pre>", e))
        }
    }
}
