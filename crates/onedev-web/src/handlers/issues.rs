//! Issue Handlers

use crate::WebState;
use axum::{
    Form,
    extract::{Path, Query, State},
    response::{Html, IntoResponse, Redirect},
};
use serde::{Deserialize, Serialize};
use tera::Context;

/// List issues
pub async fn list_issues(
    State(state): State<WebState>,
    Path(project_id): Path<String>,
    Query(params): Query<IssueListParams>,
) -> impl IntoResponse {
    let mut context = Context::new();
    context.insert("title", "Issues");
    context.insert("project_id", &project_id);
    context.insert("project", &get_project(&state, &project_id).await);
    context.insert("issues", &get_issues(&state, &project_id, &params).await);
    context.insert("params", &params);
    context.insert("user", &get_current_user(&state).await);
    context.insert("labels", &get_labels(&state, &project_id).await);
    context.insert("milestones", &get_milestones(&state, &project_id).await);
    render_template(&state, "issues/list.html", context).await
}

/// New issue page
pub async fn new_issue_page(
    State(state): State<WebState>,
    Path(project_id): Path<String>,
) -> impl IntoResponse {
    let mut context = Context::new();
    context.insert("title", "New Issue");
    context.insert("project_id", &project_id);
    context.insert("project", &get_project(&state, &project_id).await);
    context.insert("user", &get_current_user(&state).await);
    context.insert("labels", &get_labels(&state, &project_id).await);
    context.insert("milestones", &get_milestones(&state, &project_id).await);
    context.insert("assignees", &get_assignees(&state, &project_id).await);
    render_template(&state, "issues/new.html", context).await
}

/// Create issue
pub async fn create_issue(
    State(state): State<WebState>,
    Path(project_id): Path<String>,
    Form(form): Form<CreateIssueForm>,
) -> impl IntoResponse {
    // Validate
    if form.title.trim().is_empty() {
        let mut context = Context::new();
        context.insert("title", "New Issue");
        context.insert("error", "Issue title is required");
        context.insert("form", &form);
        return render_template(&state, "issues/new.html", context)
            .await
            .into_response();
    }

    // Create issue (placeholder)
    Redirect::to(&format!("/projects/{}/issues/1", project_id)).into_response()
}

/// Issue detail
pub async fn issue_detail(
    State(state): State<WebState>,
    Path((project_id, id)): Path<(String, String)>,
) -> impl IntoResponse {
    let mut context = Context::new();
    context.insert("title", &format!("Issue #{}", id));
    context.insert("project_id", &project_id);
    context.insert("project", &get_project(&state, &project_id).await);
    context.insert("issue", &get_issue(&state, &project_id, &id).await);
    context.insert("comments", &get_comments(&state, &project_id, &id).await);
    context.insert("user", &get_current_user(&state).await);
    render_template(&state, "issues/detail.html", context).await
}

/// Edit issue page
pub async fn edit_issue_page(
    State(state): State<WebState>,
    Path((project_id, id)): Path<(String, String)>,
) -> impl IntoResponse {
    let mut context = Context::new();
    context.insert("title", &format!("Edit Issue #{}", id));
    context.insert("project_id", &project_id);
    context.insert("project", &get_project(&state, &project_id).await);
    context.insert("issue", &get_issue(&state, &project_id, &id).await);
    context.insert("user", &get_current_user(&state).await);
    context.insert("labels", &get_labels(&state, &project_id).await);
    context.insert("milestones", &get_milestones(&state, &project_id).await);
    context.insert("assignees", &get_assignees(&state, &project_id).await);
    render_template(&state, "issues/edit.html", context).await
}

/// Update issue
pub async fn update_issue(
    _state: State<WebState>,
    Path((project_id, id)): Path<(String, String)>,
    _form: Form<UpdateIssueForm>,
) -> impl IntoResponse {
    // Update issue (placeholder)
    Redirect::to(&format!(
        "/projects/{}/issues/{}?saved=true",
        project_id, id
    ))
    .into_response()
}

#[derive(Debug, Deserialize, Serialize)]
pub struct IssueListParams {
    pub page: Option<u32>,
    pub per_page: Option<u32>,
    pub state: Option<String>,
    pub assignee: Option<String>,
    pub label: Option<String>,
    pub milestone: Option<String>,
    pub search: Option<String>,
    pub sort: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct CreateIssueForm {
    pub title: String,
    pub description: Option<String>,
    pub assignee_id: Option<String>,
    pub label_ids: Vec<String>,
    pub milestone_id: Option<String>,
    pub confidential: bool,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct UpdateIssueForm {
    pub title: String,
    pub description: Option<String>,
    pub state: String,
    pub assignee_id: Option<String>,
    pub label_ids: Vec<String>,
    pub milestone_id: Option<String>,
    pub confidential: bool,
}

/// Live issue list from the database when configured, otherwise the dev
/// placeholder rows. `search` is a plain substring filter for now; the
/// full vanilla query language is a later slice (see report 04).
async fn get_issues(
    state: &WebState,
    project_id: &str,
    params: &IssueListParams,
) -> Vec<serde_json::Value> {
    let needle = params.search.as_deref().unwrap_or("").to_lowercase();
    let matches = |title: &str, description: Option<&str>| {
        needle.is_empty()
            || title.to_lowercase().contains(&needle)
            || description.unwrap_or("").to_lowercase().contains(&needle)
    };
    if let (Some(pool), Ok(pid)) = (&state.db_pool, project_id.parse::<i64>()) {
        let issues = onedev_api::services::IssueService::new(pool.clone());
        let users = onedev_api::services::UserService::new(pool.clone());
        let per_page = params.per_page.unwrap_or(25).min(100).max(1) as i64;
        let page = params.page.unwrap_or(0) as i64;
        if let Ok(rows) = issues.list(pid, per_page, page * per_page).await {
            let mut out = Vec::new();
            for row in rows {
                if !matches(&row.title, row.description.as_deref()) {
                    continue;
                }
                let author_name = users
                    .get(row.submitter_id)
                    .await
                    .map(|user| user.name)
                    .unwrap_or_default();
                let assignee_name = match row.assignee_id {
                    Some(id) => users.get(id).await.map(|user| user.name).unwrap_or_default(),
                    None => String::new(),
                };
                out.push(serde_json::json!({
                    "id": row.id,
                    "number": row.number,
                    "title": row.title,
                    "description": row.description,
                    "state": format!("{:?}", row.state).to_lowercase(),
                    "author": { "username": author_name, "full_name": null, "avatar_url": null },
                    "assignee": if assignee_name.is_empty() { serde_json::Value::Null } else { serde_json::json!({ "username": assignee_name, "full_name": null, "avatar_url": null }) },
                    "labels": [],
                    "milestone": null,
                    "created_at": row.created_at,
                    "updated_at": row.updated_at,
                    "comments_count": 0,
                    "confidential": false
                }));
            }
            return out;
        }
    }
    [
        serde_json::json!({
            "id": 1,
            "number": 42,
            "title": "Fix login authentication bug",
            "description": "Users cannot login with special characters in password",
            "state": "open",
            "author": { "username": "john", "full_name": "John Doe", "avatar_url": null },
            "assignee": { "username": "jane", "full_name": "Jane Smith", "avatar_url": null },
            "labels": [
                { "id": 1, "name": "bug", "color": "#ff0000" },
                { "id": 2, "name": "high-priority", "color": "#ff9900" }
            ],
            "milestone": { "id": 1, "title": "v1.0.0", "due_date": "2024-02-01" },
            "created_at": "2024-01-10T10:00:00Z",
            "updated_at": "2024-01-15T10:30:00Z",
            "comments_count": 5,
            "confidential": false
        }),
        serde_json::json!({
            "id": 2,
            "number": 41,
            "title": "Add dark mode support",
            "description": "Implement dark/light theme toggle",
            "state": "open",
            "author": { "username": "jane", "full_name": "Jane Smith", "avatar_url": null },
            "assignee": null,
            "labels": [
                { "id": 3, "name": "feature", "color": "#00ff00" },
                { "id": 4, "name": "ui", "color": "#0099ff" }
            ],
            "milestone": { "id": 1, "title": "v1.0.0", "due_date": "2024-02-01" },
            "created_at": "2024-01-12T14:00:00Z",
            "updated_at": "2024-01-14T16:00:00Z",
            "comments_count": 3,
            "confidential": false
        }),
    ]
    .into_iter()
    .filter(|item| {
        matches(
            item.get("title").and_then(|v| v.as_str()).unwrap_or(""),
            item.get("description").and_then(|v| v.as_str()),
        )
    })
    .collect()
}

/// Get single issue
async fn get_issue(_state: &WebState, _project_id: &str, id: &str) -> serde_json::Value {
    serde_json::json!({
        "id": id,
        "number": 42,
        "title": "Fix login authentication bug",
        "description": "Users cannot login with special characters in password",
        "state": "open",
        "author": { "username": "john", "full_name": "John Doe", "avatar_url": null },
        "assignee": { "username": "jane", "full_name": "Jane Smith", "avatar_url": null },
        "labels": [
            { "id": 1, "name": "bug", "color": "#ff0000" },
            { "id": 2, "name": "high-priority", "color": "#ff9900" }
        ],
        "milestone": { "id": 1, "title": "v1.0.0", "due_date": "2024-02-01" },
        "created_at": "2024-01-10T10:00:00Z",
        "updated_at": "2024-01-15T10:30:00Z",
        "confidential": false
    })
}

/// Get issue comments
async fn get_comments(_state: &WebState, _project_id: &str, _id: &str) -> Vec<serde_json::Value> {
    vec![
        serde_json::json!({
            "id": 1,
            "author": { "username": "jane", "full_name": "Jane Smith", "avatar_url": null },
            "content": "I can reproduce this issue. Working on a fix.",
            "created_at": "2024-01-11T09:00:00Z",
            "updated_at": null,
            "system": false
        }),
        serde_json::json!({
            "id": 2,
            "author": { "username": "john", "full_name": "John Doe", "avatar_url": null },
            "content": "Thanks for looking into this!",
            "created_at": "2024-01-11T10:00:00Z",
            "updated_at": null,
            "system": false
        }),
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

/// Get labels
async fn get_labels(_state: &WebState, _project_id: &str) -> Vec<serde_json::Value> {
    vec![
        serde_json::json!({ "id": 1, "name": "bug", "color": "#ff0000", "description": "Something isn't working" }),
        serde_json::json!({ "id": 2, "name": "feature", "color": "#00ff00", "description": "New feature or request" }),
        serde_json::json!({ "id": 3, "name": "documentation", "color": "#0099ff", "description": "Documentation improvements" }),
        serde_json::json!({ "id": 4, "name": "high-priority", "color": "#ff9900", "description": "High priority" }),
    ]
}

/// Get milestones
async fn get_milestones(_state: &WebState, _project_id: &str) -> Vec<serde_json::Value> {
    vec![
        serde_json::json!({ "id": 1, "title": "v1.0.0", "description": "First stable release", "due_date": "2024-02-01", "state": "active" }),
        serde_json::json!({ "id": 2, "title": "v1.1.0", "description": "Minor improvements", "due_date": "2024-03-01", "state": "active" }),
    ]
}

/// Get assignees
async fn get_assignees(_state: &WebState, _project_id: &str) -> Vec<serde_json::Value> {
    vec![
        serde_json::json!({ "id": 1, "username": "admin", "full_name": "Admin User", "avatar_url": null }),
        serde_json::json!({ "id": 2, "username": "john", "full_name": "John Doe", "avatar_url": null }),
        serde_json::json!({ "id": 3, "username": "jane", "full_name": "Jane Smith", "avatar_url": null }),
    ]
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
