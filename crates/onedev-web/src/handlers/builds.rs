//! Build Handlers

use axum::{
    extract::{Path, Query, State},
    response::{Html, IntoResponse},
    Form,
};
use serde::{Deserialize, Serialize};
use tera::Context;
use crate::WebState;

/// List builds
pub async fn list_builds(
    State(state): State<WebState>,
    Path(project_id): Path<String>,
    Query(params): Query<BuildListParams>,
) -> impl IntoResponse {
    let mut context = Context::new();
    context.insert("title", "Builds");
    context.insert("project_id", &project_id);
    context.insert("project", &get_project(&state, &project_id).await);
    context.insert("builds", &get_builds(&state, &project_id, &params).await);
    context.insert("params", &params);
    context.insert("user", &get_current_user(&state).await);
    context.insert("pipelines", &get_pipelines(&state, &project_id).await);
    render_template(&state, "builds/list.html", context).await
}

/// Build detail
pub async fn build_detail(
    State(state): State<WebState>,
    Path((project_id, id)): Path<(String, String)>,
) -> impl IntoResponse {
    let mut context = Context::new();
    context.insert("title", &format!("Build #{}", id));
    context.insert("project_id", &project_id);
    context.insert("project", &get_project(&state, &project_id).await);
    context.insert("build", &get_build(&state, &project_id, &id).await);
    context.insert("stages", &get_build_stages(&state, &project_id, &id).await);
    context.insert("jobs", &get_build_jobs(&state, &project_id, &id).await);
    context.insert("artifacts", &get_build_artifacts(&state, &project_id, &id).await);
    context.insert("user", &get_current_user(&state).await);
    render_template(&state, "builds/detail.html", context).await
}

#[derive(Debug, Deserialize, Serialize)]
pub struct BuildListParams {
    pub page: Option<u32>,
    pub per_page: Option<u32>,
    pub status: Option<String>,
    pub branch: Option<String>,
    pub pipeline: Option<String>,
    pub search: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct TriggerBuildForm {
    pub pipeline: String,
    pub branch: String,
    pub variables: std::collections::HashMap<String, String>,
}

/// Get builds list
async fn get_builds(
    state: &WebState,
    project_id: &str,
    params: &BuildListParams,
) -> Vec<serde_json::Value> {
    vec![
        serde_json::json!({
            "id": 1,
            "number": 42,
            "pipeline": "ci",
            "branch": "main",
            "commit": { "sha": "abc123def456", "short_sha": "abc123d", "message": "Release v1.0.0" },
            "status": "success",
            "duration": 180,
            "queued_at": "2024-01-15T10:00:00Z",
            "started_at": "2024-01-15T10:01:00Z",
            "finished_at": "2024-01-15T10:04:00Z",
            "triggered_by": { "username": "admin", "full_name": "Admin User", "avatar_url": null }
        }),
        serde_json::json!({
            "id": 2,
            "number": 41,
            "pipeline": "ci",
            "branch": "feature/dark-mode",
            "commit": { "sha": "def456ghi789", "short_sha": "def456g", "message": "Add dark mode styles" },
            "status": "running",
            "duration": null,
            "queued_at": "2024-01-15T10:30:00Z",
            "started_at": "2024-01-15T10:31:00Z",
            "finished_at": null,
            "triggered_by": { "username": "jane", "full_name": "Jane Smith", "avatar_url": null }
        }),
        serde_json::json!({
            "id": 3,
            "number": 40,
            "pipeline": "deploy",
            "branch": "main",
            "commit": { "sha": "abc123def456", "short_sha": "abc123d", "message": "Release v1.0.0" },
            "status": "failed",
            "duration": 45,
            "queued_at": "2024-01-15T10:05:00Z",
            "started_at": "2024-01-15T10:06:00Z",
            "finished_at": "2024-01-15T10:07:00Z",
            "triggered_by": { "username": "admin", "full_name": "Admin User", "avatar_url": null }
        }),
    ]
}

/// Get single build
async fn get_build(state: &WebState, project_id: &str, id: &str) -> serde_json::Value {
    serde_json::json!({
        "id": id,
        "number": 42,
        "pipeline": "ci",
        "branch": "main",
        "commit": { "sha": "abc123def456", "short_sha": "abc123d", "message": "Release v1.0.0", "author": { "username": "admin", "full_name": "Admin User" } },
        "status": "success",
        "duration": 180,
        "queued_at": "2024-01-15T10:00:00Z",
        "started_at": "2024-01-15T10:01:00Z",
        "finished_at": "2024-01-15T10:04:00Z",
        "triggered_by": { "username": "admin", "full_name": "Admin User", "avatar_url": null },
        "variables": {},
        "artifacts_count": 2
    })
}

/// Get build stages
async fn get_build_stages(state: &WebState, project_id: &str, id: &str) -> Vec<serde_json::Value> {
    vec![
        serde_json::json!({
            "name": "test",
            "status": "success",
            "duration": 90,
            "jobs": [
                { "name": "unit-tests", "status": "success", "duration": 45 },
                { "name": "integration-tests", "status": "success", "duration": 45 }
            ]
        }),
        serde_json::json!({
            "name": "build",
            "status": "success",
            "duration": 60,
            "jobs": [
                { "name": "compile", "status": "success", "duration": 30 },
                { "name": "package", "status": "success", "duration": 30 }
            ]
        }),
        serde_json::json!({
            "name": "deploy",
            "status": "skipped",
            "duration": 0,
            "jobs": []
        }),
    ]
}

/// Get build jobs
async fn get_build_jobs(state: &WebState, project_id: &str, id: &str) -> Vec<serde_json::Value> {
    vec![
        serde_json::json!({
            "id": 1,
            "name": "unit-tests",
            "stage": "test",
            "status": "success",
            "duration": 45,
            "started_at": "2024-01-15T10:01:00Z",
            "finished_at": "2024-01-15T10:01:45Z",
            "runner": "runner-1",
            "artifacts": []
        }),
        serde_json::json!({
            "id": 2,
            "name": "integration-tests",
            "stage": "test",
            "status": "success",
            "duration": 45,
            "started_at": "2024-01-15T10:01:45Z",
            "finished_at": "2024-01-15T10:02:30Z",
            "runner": "runner-1",
            "artifacts": []
        }),
        serde_json::json!({
            "id": 3,
            "name": "compile",
            "stage": "build",
            "status": "success",
            "duration": 30,
            "started_at": "2024-01-15T10:02:30Z",
            "finished_at": "2024-01-15T10:03:00Z",
            "runner": "runner-2",
            "artifacts": [
                { "name": "onedev-linux-amd64", "size": 15728640, "path": "artifacts/onedev-linux-amd64" }
            ]
        }),
        serde_json::json!({
            "id": 4,
            "name": "package",
            "stage": "build",
            "status": "success",
            "duration": 30,
            "started_at": "2024-01-15T10:03:00Z",
            "finished_at": "2024-01-15T10:03:30Z",
            "runner": "runner-2",
            "artifacts": [
                { "name": "onedev-linux-amd64.tar.gz", "size": 10485760, "path": "artifacts/onedev-linux-amd64.tar.gz" }
            ]
        }),
    ]
}

/// Get build artifacts
async fn get_build_artifacts(state: &WebState, project_id: &str, id: &str) -> Vec<serde_json::Value> {
    vec![
        serde_json::json!({
            "name": "onedev-linux-amd64",
            "size": 15728640,
            "path": "artifacts/onedev-linux-amd64",
            "download_url": "/projects/1/builds/42/artifacts/onedev-linux-amd64/download"
        }),
        serde_json::json!({
            "name": "onedev-linux-amd64.tar.gz",
            "size": 10485760,
            "path": "artifacts/onedev-linux-amd64.tar.gz",
            "download_url": "/projects/1/builds/42/artifacts/onedev-linux-amd64.tar.gz/download"
        }),
    ]
}

/// Get pipelines
async fn get_pipelines(state: &WebState, project_id: &str) -> Vec<serde_json::Value> {
    vec![
        serde_json::json!({ "name": "ci", "description": "Continuous Integration" }),
        serde_json::json!({ "name": "deploy", "description": "Deploy to production" }),
        serde_json::json!({ "name": "release", "description": "Create release artifacts" }),
    ]
}

/// Get project
async fn get_project(state: &WebState, id: &str) -> serde_json::Value {
    serde_json::json!({
        "id": id,
        "name": "onedev/core",
        "path": "onedev/core",
        "visibility": "private"
    })
}

/// Get current user (placeholder)
async fn get_current_user(state: &WebState) -> serde_json::Value {
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