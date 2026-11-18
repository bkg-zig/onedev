//! Dashboard Handlers

use crate::WebState;
use axum::{
    extract::State,
    response::{Html, IntoResponse},
};
use tera::Context;

/// Main dashboard page
pub async fn dashboard_handler(State(state): State<WebState>) -> impl IntoResponse {
    let mut context = Context::new();
    context.insert("title", "Dashboard");
    context.insert("user", &get_current_user(&state).await);
    context.insert("stats", &get_dashboard_stats(&state).await);
    context.insert("recent_activity", &get_recent_activity(&state).await);
    context.insert("projects", &get_user_projects(&state).await);

    render_template(&state, "dashboard.html", context).await
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

/// Get dashboard statistics
async fn get_dashboard_stats(_state: &WebState) -> serde_json::Value {
    serde_json::json!({
        "projects_count": 5,
        "issues_assigned": 3,
        "issues_created": 10,
        "merge_requests_open": 2,
        "builds_running": 1,
        "builds_pending": 0
    })
}

/// Get recent activity
async fn get_recent_activity(_state: &WebState) -> Vec<serde_json::Value> {
    vec![
        serde_json::json!({
            "id": 1,
            "type": "issue_created",
            "title": "Fix login bug",
            "project": "onedev/core",
            "author": "john",
            "created_at": "2024-01-15T10:30:00Z"
        }),
        serde_json::json!({
            "id": 2,
            "type": "mr_merged",
            "title": "Add new feature",
            "project": "onedev/web",
            "author": "jane",
            "created_at": "2024-01-15T09:15:00Z"
        }),
        serde_json::json!({
            "id": 3,
            "type": "build_completed",
            "title": "Build #42 succeeded",
            "project": "onedev/api",
            "author": "system",
            "created_at": "2024-01-15T08:00:00Z"
        }),
    ]
}

/// Get user projects
async fn get_user_projects(_state: &WebState) -> Vec<serde_json::Value> {
    vec![
        serde_json::json!({
            "id": 1,
            "name": "onedev/core",
            "description": "Core OneDev functionality",
            "visibility": "private",
            "stars": 10,
            "forks": 2,
            "last_activity": "2024-01-15T10:30:00Z"
        }),
        serde_json::json!({
            "id": 2,
            "name": "onedev/web",
            "description": "Web UI for OneDev",
            "visibility": "public",
            "stars": 25,
            "forks": 5,
            "last_activity": "2024-01-15T09:15:00Z"
        }),
    ]
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
