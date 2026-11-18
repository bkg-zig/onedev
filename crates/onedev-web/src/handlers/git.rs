//! Git Repository Handlers

use crate::WebState;
use axum::{
    extract::{Path, Query, State},
    response::{Html, IntoResponse},
};
use serde::{Deserialize, Serialize};
use tera::Context;

/// Git overview
pub async fn git_overview(
    State(state): State<WebState>,
    Path(project_id): Path<String>,
) -> impl IntoResponse {
    let mut context = Context::new();
    context.insert("title", "Repository");
    context.insert("project_id", &project_id);
    context.insert("project", &get_project(&state, &project_id).await);
    context.insert("branches", &get_branches(&state, &project_id).await);
    context.insert("tags", &get_tags(&state, &project_id).await);
    context.insert(
        "commits",
        &get_recent_commits(&state, &project_id, "main", 10).await,
    );
    context.insert("user", &get_current_user(&state).await);
    render_template(&state, "git/overview.html", context).await
}

/// Git tree (file browser)
pub async fn git_tree(
    State(state): State<WebState>,
    Path((project_id, ref_name)): Path<(String, String)>,
    Query(params): Query<GitTreeParams>,
) -> impl IntoResponse {
    let path = params.path.unwrap_or_default();
    let mut context = Context::new();
    context.insert("title", &format!("Repository: {}", ref_name));
    context.insert("project_id", &project_id);
    context.insert("project", &get_project(&state, &project_id).await);
    context.insert("ref_name", &ref_name);
    context.insert("path", &path);
    context.insert(
        "entries",
        &get_tree_entries(&state, &project_id, &ref_name, &path).await,
    );
    context.insert("branches", &get_branches(&state, &project_id).await);
    context.insert("tags", &get_tags(&state, &project_id).await);
    context.insert("user", &get_current_user(&state).await);
    render_template(&state, "git/tree.html", context).await
}

/// Git blob (file view)
pub async fn git_blob(
    State(state): State<WebState>,
    Path((project_id, ref_name, path)): Path<(String, String, String)>,
) -> impl IntoResponse {
    let mut context = Context::new();
    context.insert("title", &format!("{} @ {}", path, ref_name));
    context.insert("project_id", &project_id);
    context.insert("project", &get_project(&state, &project_id).await);
    context.insert("ref_name", &ref_name);
    context.insert("path", &path);
    context.insert(
        "blob",
        &get_blob(&state, &project_id, &ref_name, &path).await,
    );
    context.insert("branches", &get_branches(&state, &project_id).await);
    context.insert("tags", &get_tags(&state, &project_id).await);
    context.insert("user", &get_current_user(&state).await);
    render_template(&state, "git/blob.html", context).await
}

/// Git commits list
pub async fn git_commits(
    State(state): State<WebState>,
    Path(project_id): Path<String>,
    Query(params): Query<GitCommitsParams>,
) -> impl IntoResponse {
    let ref_name = params.ref_name.unwrap_or_else(|| "main".to_string());
    let path = params.path;
    let mut context = Context::new();
    context.insert("title", "Commits");
    context.insert("project_id", &project_id);
    context.insert("project", &get_project(&state, &project_id).await);
    context.insert("ref_name", &ref_name);
    context.insert("path", &path);
    context.insert(
        "commits",
        &get_recent_commits(&state, &project_id, &ref_name, 50).await,
    );
    context.insert("branches", &get_branches(&state, &project_id).await);
    context.insert("tags", &get_tags(&state, &project_id).await);
    context.insert("user", &get_current_user(&state).await);
    render_template(&state, "git/commits.html", context).await
}

/// Git commit detail
pub async fn git_commit(
    State(state): State<WebState>,
    Path((project_id, sha)): Path<(String, String)>,
) -> impl IntoResponse {
    let mut context = Context::new();
    context.insert("title", &format!("Commit {}", sha.get(..8).unwrap_or(&sha)));
    context.insert("project_id", &project_id);
    context.insert("project", &get_project(&state, &project_id).await);
    context.insert("commit", &get_commit(&state, &project_id, &sha).await);
    context.insert("user", &get_current_user(&state).await);
    render_template(&state, "git/commit.html", context).await
}

#[derive(Debug, Deserialize, Serialize)]
pub struct GitTreeParams {
    pub path: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct GitCommitsParams {
    pub ref_name: Option<String>,
    pub path: Option<String>,
    pub page: Option<u32>,
    pub per_page: Option<u32>,
}

/// Git repository root (pared with the API's GitService).
fn git_root() -> std::path::PathBuf {
    std::env::var("ONEDEV_GIT_ROOT")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|_| std::path::PathBuf::from("/var/lib/onedev/repos"))
}

fn git_service() -> onedev_git::services::repository::RepositoryService {
    onedev_git::services::repository::RepositoryService::new(onedev_git::models::GitConfig {
        root: git_root(),
        ..Default::default()
    })
}

/// Resolve the repository path for a project URL id.
/// Numeric ids are looked up in the DB; anything else is used as-is.
async fn project_path(state: &WebState, id: &str) -> String {
    if let (Some(pool), Ok(pid)) = (&state.db_pool, id.parse::<i64>()) {
        if let Ok(project) = onedev_api::services::ProjectService::new(pool.clone())
            .get(pid)
            .await
        {
            return project.full_path;
        }
    }
    id.to_owned()
}

/// Get project
async fn get_project(state: &WebState, id: &str) -> serde_json::Value {
    if let (Some(pool), Ok(pid)) = (&state.db_pool, id.parse::<i64>()) {
        if let Ok(project) = onedev_api::services::ProjectService::new(pool.clone())
            .get(pid)
            .await
        {
            return serde_json::json!({
                "id": project.id,
                "name": project.name,
                "path": project.full_path,
                "description": project.description,
                "visibility": format!("{:?}", project.visibility).to_lowercase(),
                "default_branch": project.default_branch
            });
        }
    }
    serde_json::json!({
        "id": id,
        "name": "onedev/core",
        "path": "onedev/core",
        "visibility": "private",
        "default_branch": "main"
    })
}

/// Get branches (live via gix when the repo exists, else placeholder).
async fn get_branches(state: &WebState, project_id: &str) -> Vec<serde_json::Value> {
    let path = project_path(state, project_id).await;
    if let Ok(refs) = git_service().list_branches(&path) {
        let mut out = refs
            .into_iter()
            .map(|r| {
                serde_json::json!({ "name": r.name, "protected": r.name == "main", "commit": { "sha": r.object_id } })
            })
            .collect::<Vec<_>>();
        if !out.is_empty() {
            return out;
        }
    }
    vec![
        serde_json::json!({ "name": "main", "protected": true, "commit": { "sha": "abc123def456", "message": "Release v1.0.0" } }),
        serde_json::json!({ "name": "develop", "protected": false, "commit": { "sha": "def456ghi789", "message": "Merge feature/dark-mode" } }),
        serde_json::json!({ "name": "feature/dark-mode", "protected": false, "commit": { "sha": "ghi789jkl012", "message": "Add dark mode styles" } }),
        serde_json::json!({ "name": "hotfix/login-bug", "protected": false, "commit": { "sha": "jkl012mno345", "message": "Fix login special chars" } }),
    ]
}

/// Get tags (live via gix when the repo exists, else placeholder).
async fn get_tags(state: &WebState, project_id: &str) -> Vec<serde_json::Value> {
    let path = project_path(state, project_id).await;
    if let Ok(refs) = git_service().list_tags(&path) {
        let out = refs
            .into_iter()
            .map(|r| serde_json::json!({ "name": r.name, "commit": { "sha": r.object_id } }))
            .collect::<Vec<_>>();
        if !out.is_empty() {
            return out;
        }
    }
    vec![
        serde_json::json!({ "name": "v1.0.0", "commit": { "sha": "abc123def456", "message": "Release v1.0.0" }, "tagger": { "name": "Admin User", "date": "2024-01-15T10:00:00Z" } }),
        serde_json::json!({ "name": "v0.9.0", "commit": { "sha": "mno345pqr678", "message": "Beta release" }, "tagger": { "name": "Admin User", "date": "2024-01-01T10:00:00Z" } }),
    ]
}

/// Get recent commits (live via gix history when the repo exists).
async fn get_recent_commits(
    state: &WebState,
    project_id: &str,
    ref_name: &str,
    limit: usize,
) -> Vec<serde_json::Value> {
    let path = project_path(state, project_id).await;
    if let Ok(commits) = git_service().history(&path, ref_name, limit) {
        let out = commits
            .into_iter()
            .map(|c| {
                let short = c.id.chars().take(7).collect::<String>();
                serde_json::json!({
                    "sha": c.id,
                    "short_sha": short,
                    "message": c.message,
                    "author": { "username": c.author.name, "full_name": c.committer.name, "avatar_url": null, "email": c.author.email },
                    "authored_at": chrono::DateTime::from_timestamp(c.author.timestamp, 0),
                    "committed_at": chrono::DateTime::from_timestamp(c.committer.timestamp, 0),
                    "verified": false,
                    "parents": c.parents
                })
            })
            .collect::<Vec<_>>();
        if !out.is_empty() {
            return out;
        }
    }
    vec![
        serde_json::json!({
            "sha": "abc123def456789",
            "short_sha": "abc123d",
            "message": "Release v1.0.0",
            "author": { "username": "admin", "full_name": "Admin User", "avatar_url": null, "email": "admin@example.com" },
            "authored_at": "2024-01-15T10:00:00Z",
            "committed_at": "2024-01-15T10:00:00Z",
            "verified": true,
            "parents": []
        }),
        serde_json::json!({
            "sha": "def456ghi789012",
            "short_sha": "def456g",
            "message": "Merge pull request #5 from feature/dark-mode",
            "author": { "username": "admin", "full_name": "Admin User", "avatar_url": null, "email": "admin@example.com" },
            "authored_at": "2024-01-15T09:30:00Z",
            "committed_at": "2024-01-15T09:30:00Z",
            "verified": true,
            "parents": ["abc123def456789", "ghi789jkl012345"]
        }),
        serde_json::json!({
            "sha": "ghi789jkl012345",
            "short_sha": "ghi789j",
            "message": "Add dark mode styles",
            "author": { "username": "jane", "full_name": "Jane Smith", "avatar_url": null, "email": "jane@example.com" },
            "authored_at": "2024-01-14T16:00:00Z",
            "committed_at": "2024-01-14T16:00:00Z",
            "verified": true,
            "parents": ["def456ghi789012"]
        }),
    ]
}

/// Get tree entries (live via gix when the repo exists).
async fn get_tree_entries(
    state: &WebState,
    project_id: &str,
    ref_name: &str,
    path: &str,
) -> Vec<serde_json::Value> {
    let repo_path = project_path(state, project_id).await;
    if let Ok(entries) = git_service().list_tree(&repo_path, ref_name, path) {
        let out = entries
            .into_iter()
            .map(|e| {
                serde_json::json!({
                    "name": e.path.rsplit('/').next().unwrap_or(&e.path),
                    "type": format!("{:?}", e.kind).to_lowercase(),
                    "path": e.path,
                    "size": e.size,
                    "oid": e.object_id
                })
            })
            .collect::<Vec<_>>();
        if !out.is_empty() {
            return out;
        }
    }
    if path.is_empty() {
        vec![
            serde_json::json!({ "name": "src", "type": "tree", "mode": "040000", "path": "src" }),
            serde_json::json!({ "name": "tests", "type": "tree", "mode": "040000", "path": "tests" }),
            serde_json::json!({ "name": "docs", "type": "tree", "mode": "040000", "path": "docs" }),
            serde_json::json!({ "name": "Cargo.toml", "type": "blob", "mode": "100644", "size": 2048, "path": "Cargo.toml" }),
            serde_json::json!({ "name": "README.md", "type": "blob", "mode": "100644", "size": 5120, "path": "README.md" }),
            serde_json::json!({ "name": "LICENSE", "type": "blob", "mode": "100644", "size": 1024, "path": "LICENSE" }),
        ]
    } else if path == "src" {
        vec![
            serde_json::json!({ "name": "main.rs", "type": "blob", "mode": "100644", "size": 1024, "path": "src/main.rs" }),
            serde_json::json!({ "name": "lib.rs", "type": "blob", "mode": "100644", "size": 2048, "path": "src/lib.rs" }),
            serde_json::json!({ "name": "components", "type": "tree", "mode": "040000", "path": "src/components" }),
            serde_json::json!({ "name": "utils", "type": "tree", "mode": "040000", "path": "src/utils" }),
        ]
    } else {
        vec![]
    }
}

/// Get blob content (live via gix when the repo exists).
async fn get_blob(
    state: &WebState,
    project_id: &str,
    ref_name: &str,
    path: &str,
) -> serde_json::Value {
    let repo_path = project_path(state, project_id).await;
    if let Ok(blob) = git_service().read_blob(&repo_path, ref_name, path) {
        let content = blob.text().unwrap_or_default();
        let lines = content.lines().count();
        return serde_json::json!({
            "path": path,
            "size": content.len(),
            "encoding": "utf-8",
            "content": content,
            "language": "rust",
            "lines": lines
        });
    }
    serde_json::json!({
        "path": path,
        "size": 2048,
        "encoding": "utf-8",
        "content": "// Sample Rust code\nfn main() {\n    println!(\"Hello, OneDev!\");\n}",
        "language": "rust",
        "lines": 5
    })
}

/// Get commit detail (live via gix when the repo exists).
async fn get_commit(state: &WebState, project_id: &str, sha: &str) -> serde_json::Value {
    let repo_path = project_path(state, project_id).await;
    if let Ok(commit) = git_service().resolve_commit(&repo_path, sha) {
        let short = commit.id.chars().take(7).collect::<String>();
        return serde_json::json!({
            "sha": commit.id,
            "short_sha": short,
            "message": commit.message,
            "author": { "username": commit.author.name, "full_name": commit.committer.name, "avatar_url": null, "email": commit.author.email },
            "committer": { "username": commit.committer.name, "full_name": commit.committer.name, "avatar_url": null, "email": commit.committer.email },
            "authored_at": chrono::DateTime::from_timestamp(commit.author.timestamp, 0),
            "committed_at": chrono::DateTime::from_timestamp(commit.committer.timestamp, 0),
            "verified": false,
            "parents": commit.parents,
            "stats": { "files_changed": 0, "additions": 0, "deletions": 0 },
            "changes": []
        });
    }
    serde_json::json!({
        "sha": sha,
        "short_sha": &sha[..8.min(sha.len())],
        "message": "Release v1.0.0\n\nThis release includes the initial stable version of OneDev.",
        "author": { "username": "admin", "full_name": "Admin User", "avatar_url": null, "email": "admin@example.com" },
        "committer": { "username": "admin", "full_name": "Admin User", "avatar_url": null, "email": "admin@example.com" },
        "authored_at": "2024-01-15T10:00:00Z",
        "committed_at": "2024-01-15T10:00:00Z",
        "verified": true,
        "parents": [],
        "stats": { "files_changed": 10, "additions": 500, "deletions": 50 },
        "changes": [
            { "old_path": "Cargo.toml", "new_path": "Cargo.toml", "status": "modified", "additions": 5, "deletions": 2 },
            { "old_path": "src/main.rs", "new_path": "src/main.rs", "status": "modified", "additions": 10, "deletions": 3 },
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
