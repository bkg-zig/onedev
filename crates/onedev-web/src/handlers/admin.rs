//! Admin Handlers

use crate::WebState;
use axum::{
    extract::{Query, State},
    response::{Html, IntoResponse},
};
use serde::{Deserialize, Serialize};
use tera::Context;

/// Admin dashboard
pub async fn admin_dashboard(State(state): State<WebState>) -> impl IntoResponse {
    let mut context = Context::new();
    context.insert("title", "Admin Dashboard");
    context.insert("stats", &get_admin_stats(&state).await);
    context.insert("system_info", &get_system_info(&state).await);
    context.insert("recent_users", &get_recent_users(&state).await);
    context.insert("recent_projects", &get_recent_projects(&state).await);
    render_template(&state, "admin/dashboard.html", context).await
}

/// List all users
pub async fn list_users(
    State(state): State<WebState>,
    Query(params): Query<UserListParams>,
) -> impl IntoResponse {
    let mut context = Context::new();
    context.insert("title", "User Management");
    context.insert("users", &get_users(&state, &params).await);
    context.insert("params", &params);
    render_template(&state, "admin/users.html", context).await
}

/// List all projects
pub async fn list_all_projects(
    State(state): State<WebState>,
    Query(params): Query<ProjectListParams>,
) -> impl IntoResponse {
    let mut context = Context::new();
    context.insert("title", "Project Management");
    context.insert("projects", &get_all_projects(&state, &params).await);
    context.insert("params", &params);
    render_template(&state, "admin/projects.html", context).await
}

/// System info
pub async fn system_info(State(state): State<WebState>) -> impl IntoResponse {
    let mut context = Context::new();
    context.insert("title", "System Information");
    context.insert("system", &get_system_info(&state).await);
    context.insert("database", &get_database_info(&state).await);
    context.insert("storage", &get_storage_info(&state).await);
    context.insert("build_queue", &get_build_queue_info(&state).await);
    render_template(&state, "admin/system.html", context).await
}

#[derive(Debug, Deserialize, Serialize)]
pub struct UserListParams {
    pub page: Option<u32>,
    pub per_page: Option<u32>,
    pub search: Option<String>,
    pub state: Option<String>,
    pub role: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ProjectListParams {
    pub page: Option<u32>,
    pub per_page: Option<u32>,
    pub search: Option<String>,
    pub visibility: Option<String>,
}

/// Get admin stats
async fn get_admin_stats(_state: &WebState) -> serde_json::Value {
    serde_json::json!({
        "total_users": 150,
        "active_users": 120,
        "total_projects": 50,
        "public_projects": 20,
        "total_builds_today": 45,
        "running_builds": 2,
        "pending_builds": 3,
        "webhook_deliveries_24h": 250,
        "failed_webhooks_24h": 5
    })
}

/// Get system info
async fn get_system_info(_state: &WebState) -> serde_json::Value {
    serde_json::json!({
        "version": "0.1.0",
        "rust_version": "1.89.0",
        "uptime": "7d 12h 34m",
        "cpu_usage": 25.5,
        "memory_usage": 65.2,
        "disk_usage": 45.8,
        "network_in": 1024.5,
        "network_out": 2048.3
    })
}

/// Get recent users
async fn get_recent_users(_state: &WebState) -> Vec<serde_json::Value> {
    vec![
        serde_json::json!({
            "id": 151,
            "username": "newuser1",
            "email": "user1@example.com",
            "full_name": "New User 1",
            "created_at": "2024-01-15T08:00:00Z",
            "last_sign_in": "2024-01-15T10:00:00Z",
            "state": "active",
            "admin": false
        }),
        serde_json::json!({
            "id": 152,
            "username": "newuser2",
            "email": "user2@example.com",
            "full_name": "New User 2",
            "created_at": "2024-01-15T09:00:00Z",
            "last_sign_in": null,
            "state": "pending",
            "admin": false
        }),
    ]
}

/// Get recent projects
async fn get_recent_projects(_state: &WebState) -> Vec<serde_json::Value> {
    vec![
        serde_json::json!({
            "id": 51,
            "name": "new-project",
            "path": "user/new-project",
            "owner": "newuser1",
            "visibility": "private",
            "created_at": "2024-01-15T08:30:00Z"
        }),
        serde_json::json!({
            "id": 52,
            "name": "awesome-lib",
            "path": "org/awesome-lib",
            "owner": "newuser2",
            "visibility": "public",
            "created_at": "2024-01-15T09:30:00Z"
        }),
    ]
}

/// Get users
async fn get_users(_state: &WebState, _params: &UserListParams) -> Vec<serde_json::Value> {
    vec![
        serde_json::json!({
            "id": 1,
            "username": "admin",
            "email": "admin@example.com",
            "full_name": "Admin User",
            "avatar_url": null,
            "state": "active",
            "admin": true,
            "created_at": "2024-01-01T00:00:00Z",
            "last_sign_in": "2024-01-15T10:30:00Z",
            "projects_count": 10,
            "ssh_keys_count": 2,
            "gpg_keys_count": 1
        }),
        serde_json::json!({
            "id": 2,
            "username": "john",
            "email": "john@example.com",
            "full_name": "John Doe",
            "avatar_url": null,
            "state": "active",
            "admin": false,
            "created_at": "2024-01-02T00:00:00Z",
            "last_sign_in": "2024-01-15T09:00:00Z",
            "projects_count": 5,
            "ssh_keys_count": 1,
            "gpg_keys_count": 0
        }),
        serde_json::json!({
            "id": 3,
            "username": "jane",
            "email": "jane@example.com",
            "full_name": "Jane Smith",
            "avatar_url": null,
            "state": "active",
            "admin": false,
            "created_at": "2024-01-03T00:00:00Z",
            "last_sign_in": "2024-01-14T18:00:00Z",
            "projects_count": 8,
            "ssh_keys_count": 1,
            "gpg_keys_count": 1
        }),
    ]
}

/// Get all projects
async fn get_all_projects(
    _state: &WebState,
    _params: &ProjectListParams,
) -> Vec<serde_json::Value> {
    vec![
        serde_json::json!({
            "id": 1,
            "name": "onedev/core",
            "path": "onedev/core",
            "description": "Core OneDev functionality",
            "visibility": "private",
            "owner": "admin",
            "stars": 10,
            "forks": 2,
            "open_issues": 5,
            "open_mrs": 2,
            "last_activity": "2024-01-15T10:30:00Z",
            "created_at": "2024-01-01T00:00:00Z",
            "archived": false
        }),
        serde_json::json!({
            "id": 2,
            "name": "onedev/web",
            "path": "onedev/web",
            "description": "Web UI for OneDev",
            "visibility": "public",
            "owner": "admin",
            "stars": 25,
            "forks": 5,
            "open_issues": 3,
            "open_mrs": 1,
            "last_activity": "2024-01-15T09:15:00Z",
            "created_at": "2024-01-02T00:00:00Z",
            "archived": false
        }),
    ]
}

/// Get database info
async fn get_database_info(_state: &WebState) -> serde_json::Value {
    serde_json::json!({
        "size": "2.5 GB",
        "tables": 45,
        "connections_active": 12,
        "connections_idle": 8,
        "max_connections": 20,
        "cache_hit_ratio": 99.5
    })
}

/// Get storage info
async fn get_storage_info(_state: &WebState) -> serde_json::Value {
    serde_json::json!({
        "type": "local",
        "path": "/var/lib/onedev/storage",
        "total_space": "500 GB",
        "used_space": "45 GB",
        "available_space": "455 GB",
        "repositories_size": "30 GB",
        "artifacts_size": "10 GB",
        "lfs_size": "5 GB"
    })
}

/// Get build queue info
async fn get_build_queue_info(_state: &WebState) -> serde_json::Value {
    serde_json::json!({
        "running": 2,
        "pending": 3,
        "max_concurrent": 4,
        "avg_wait_time": "2m 30s",
        "completed_today": 45,
        "failed_today": 3
    })
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
