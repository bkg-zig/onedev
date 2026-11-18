//! Merge Request Handlers

use crate::WebState;
use axum::{
    Form,
    extract::{Path, Query, State},
    response::{Html, IntoResponse, Redirect},
};
use serde::{Deserialize, Serialize};
use tera::Context;

/// List merge requests
pub async fn list_mrs(
    State(state): State<WebState>,
    Path(project_id): Path<String>,
    Query(params): Query<MrListParams>,
) -> impl IntoResponse {
    let mut context = Context::new();
    context.insert("title", "Merge Requests");
    context.insert("project_id", &project_id);
    context.insert("project", &get_project(&state, &project_id).await);
    context.insert(
        "merge_requests",
        &get_merge_requests(&state, &project_id, &params).await,
    );
    context.insert("params", &params);
    context.insert("user", &get_current_user(&state).await);
    render_template(&state, "merge_requests/list.html", context).await
}

/// New merge request page
pub async fn new_mr_page(
    State(state): State<WebState>,
    Path(project_id): Path<String>,
) -> impl IntoResponse {
    let mut context = Context::new();
    context.insert("title", "New Merge Request");
    context.insert("project_id", &project_id);
    context.insert("project", &get_project(&state, &project_id).await);
    context.insert("user", &get_current_user(&state).await);
    context.insert("branches", &get_branches(&state, &project_id).await);
    render_template(&state, "merge_requests/new.html", context).await
}

/// Create merge request
pub async fn create_mr(
    State(state): State<WebState>,
    Path(project_id): Path<String>,
    Form(form): Form<CreateMrForm>,
) -> impl IntoResponse {
    // Validate
    if form.title.trim().is_empty() {
        let mut context = Context::new();
        context.insert("title", "New Merge Request");
        context.insert("error", "Title is required");
        context.insert("form", &form);
        return render_template(&state, "merge_requests/new.html", context)
            .await
            .into_response();
    }

    // Create MR (placeholder)
    Redirect::to(&format!("/projects/{}/merge-requests/1", project_id)).into_response()
}

/// Merge request detail
pub async fn mr_detail(
    State(state): State<WebState>,
    Path((project_id, id)): Path<(String, String)>,
) -> impl IntoResponse {
    let mut context = Context::new();
    context.insert("title", &format!("MR #{}", id));
    context.insert("project_id", &project_id);
    context.insert("project", &get_project(&state, &project_id).await);
    context.insert("mr", &get_merge_request(&state, &project_id, &id).await);
    context.insert("commits", &get_mr_commits(&state, &project_id, &id).await);
    context.insert("changes", &get_mr_changes(&state, &project_id, &id).await);
    context.insert("comments", &get_mr_comments(&state, &project_id, &id).await);
    context.insert("user", &get_current_user(&state).await);
    render_template(&state, "merge_requests/detail.html", context).await
}

#[derive(Debug, Deserialize, Serialize)]
pub struct MrListParams {
    pub page: Option<u32>,
    pub per_page: Option<u32>,
    pub state: Option<String>,
    pub author: Option<String>,
    pub assignee: Option<String>,
    pub target_branch: Option<String>,
    pub search: Option<String>,
    pub sort: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct CreateMrForm {
    pub title: String,
    pub description: Option<String>,
    pub source_branch: String,
    pub target_branch: String,
    pub assignee_id: Option<String>,
    pub reviewer_ids: Vec<String>,
    pub draft: bool,
}

/// Get merge requests list
async fn get_merge_requests(
    _state: &WebState,
    _project_id: &str,
    _params: &MrListParams,
) -> Vec<serde_json::Value> {
    vec![serde_json::json!({
        "id": 1,
        "number": 5,
        "title": "Add dark mode support",
        "description": "Implement dark/light theme toggle for the UI",
        "state": "open",
        "author": { "username": "jane", "full_name": "Jane Smith", "avatar_url": null },
        "assignee": { "username": "john", "full_name": "John Doe", "avatar_url": null },
        "reviewers": [
            { "username": "admin", "full_name": "Admin User", "avatar_url": null }
        ],
        "source_branch": "feature/dark-mode",
        "target_branch": "main",
        "draft": false,
        "merge_status": "can_be_merged",
        "created_at": "2024-01-12T14:00:00Z",
        "updated_at": "2024-01-15T10:30:00Z",
        "commits_count": 3,
        "changes_count": 12
    })]
}

/// Get single merge request
async fn get_merge_request(_state: &WebState, _project_id: &str, id: &str) -> serde_json::Value {
    serde_json::json!({
        "id": id,
        "number": 5,
        "title": "Add dark mode support",
        "description": "Implement dark/light theme toggle for the UI",
        "state": "open",
        "author": { "username": "jane", "full_name": "Jane Smith", "avatar_url": null },
        "assignee": { "username": "john", "full_name": "John Doe", "avatar_url": null },
        "reviewers": [
            { "username": "admin", "full_name": "Admin User", "avatar_url": null }
        ],
        "source_branch": "feature/dark-mode",
        "target_branch": "main",
        "draft": false,
        "merge_status": "can_be_merged",
        "created_at": "2024-01-12T14:00:00Z",
        "updated_at": "2024-01-15T10:30:00Z",
        "commits_count": 3,
        "changes_count": 12,
        "head_commit": { "sha": "abc123", "message": "Add dark mode toggle" }
    })
}

/// Get MR commits
async fn get_mr_commits(_state: &WebState, _project_id: &str, _id: &str) -> Vec<serde_json::Value> {
    vec![
        serde_json::json!({
            "sha": "abc123def456",
            "short_sha": "abc123d",
            "message": "Add dark mode toggle component",
            "author": { "username": "jane", "full_name": "Jane Smith", "avatar_url": null },
            "authored_at": "2024-01-12T14:00:00Z",
            "verified": true
        }),
        serde_json::json!({
            "sha": "def456ghi789",
            "short_sha": "def456g",
            "message": "Update theme context provider",
            "author": { "username": "jane", "full_name": "Jane Smith", "avatar_url": null },
            "authored_at": "2024-01-13T10:00:00Z",
            "verified": true
        }),
        serde_json::json!({
            "sha": "ghi789jkl012",
            "short_sha": "ghi789j",
            "message": "Add dark mode styles",
            "author": { "username": "jane", "full_name": "Jane Smith", "avatar_url": null },
            "authored_at": "2024-01-14T16:00:00Z",
            "verified": true
        }),
    ]
}

/// Get MR changes
async fn get_mr_changes(_state: &WebState, _project_id: &str, _id: &str) -> Vec<serde_json::Value> {
    vec![
        serde_json::json!({
            "old_path": "src/components/ThemeToggle.tsx",
            "new_path": "src/components/ThemeToggle.tsx",
            "status": "added",
            "lines_added": 45,
            "lines_removed": 0
        }),
        serde_json::json!({
            "old_path": "src/context/ThemeContext.tsx",
            "new_path": "src/context/ThemeContext.tsx",
            "status": "modified",
            "lines_added": 12,
            "lines_removed": 3
        }),
        serde_json::json!({
            "old_path": "src/styles/theme.css",
            "new_path": "src/styles/theme.css",
            "status": "modified",
            "lines_added": 78,
            "lines_removed": 12
        }),
    ]
}

/// Get MR comments
async fn get_mr_comments(
    _state: &WebState,
    _project_id: &str,
    _id: &str,
) -> Vec<serde_json::Value> {
    vec![
        serde_json::json!({
            "id": 1,
            "author": { "username": "john", "full_name": "John Doe", "avatar_url": null },
            "content": "LGTM! The implementation looks clean.",
            "created_at": "2024-01-14T17:00:00Z",
            "updated_at": null,
            "type": "comment"
        }),
        serde_json::json!({
            "id": 2,
            "author": { "username": "admin", "full_name": "Admin User", "avatar_url": null },
            "content": "Please add tests for the new component.",
            "created_at": "2024-01-15T09:00:00Z",
            "updated_at": null,
            "type": "review"
        }),
    ]
}

/// Get branches
async fn get_branches(_state: &WebState, _project_id: &str) -> Vec<serde_json::Value> {
    vec![
        serde_json::json!({ "name": "main", "protected": true, "commit": { "sha": "abc123", "message": "Release v1.0.0" } }),
        serde_json::json!({ "name": "develop", "protected": false, "commit": { "sha": "def456", "message": "Merge feature/dark-mode" } }),
        serde_json::json!({ "name": "feature/dark-mode", "protected": false, "commit": { "sha": "ghi789", "message": "Add dark mode styles" } }),
        serde_json::json!({ "name": "hotfix/login-bug", "protected": false, "commit": { "sha": "jkl012", "message": "Fix login special chars" } }),
    ]
}

/// Get project
async fn get_project(_state: &WebState, id: &str) -> serde_json::Value {
    serde_json::json!({
        "id": id,
        "name": "onedev/core",
        "path": "onedev/core",
        "visibility": "private"
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
