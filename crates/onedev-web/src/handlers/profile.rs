//! Profile Handlers

use crate::WebState;
use axum::{
    Form,
    extract::State,
    response::{Html, IntoResponse, Redirect},
};
use serde::{Deserialize, Serialize};
use tera::Context;

/// Profile page
pub async fn profile_page(State(state): State<WebState>) -> impl IntoResponse {
    let user = get_current_user(&state).await;
    let mut context = Context::new();
    context.insert("title", "Profile");
    context.insert("user", &user);
    context.insert("stats", &get_user_stats(&state).await);
    context.insert("recent_activity", &get_recent_activity(&state).await);
    render_template(&state, "profile/profile.html", context).await
}

/// Settings page
pub async fn settings_page(State(state): State<WebState>) -> impl IntoResponse {
    let user = get_current_user(&state).await;
    let mut context = Context::new();
    context.insert("title", "Settings");
    context.insert("user", &user);
    context.insert("ssh_keys", &get_ssh_keys(&state).await);
    context.insert("gpg_keys", &get_gpg_keys(&state).await);
    context.insert("tokens", &get_access_tokens(&state).await);
    context.insert("sessions", &get_sessions(&state).await);
    context.insert("two_fa_enabled", &false);
    render_template(&state, "profile/settings.html", context).await
}

/// Update settings
pub async fn update_settings(
    _state: State<WebState>,
    _form: Form<UpdateSettingsForm>,
) -> impl IntoResponse {
    // Update settings (placeholder)
    Redirect::to("/profile/settings?saved=true").into_response()
}

#[derive(Debug, Deserialize, Serialize)]
pub struct UpdateSettingsForm {
    pub full_name: String,
    pub email: String,
    pub username: String,
    pub bio: Option<String>,
    pub website: Option<String>,
    pub location: Option<String>,
    pub theme: String,
    pub language: String,
    pub timezone: String,
}

/// Get user stats
async fn get_user_stats(_state: &WebState) -> serde_json::Value {
    serde_json::json!({
        "projects_count": 5,
        "stars_given": 10,
        "followers": 25,
        "following": 10,
        "contributions_this_year": 150,
        "longest_streak": 30
    })
}

/// Get recent activity
async fn get_recent_activity(_state: &WebState) -> Vec<serde_json::Value> {
    vec![
        serde_json::json!({
            "type": "commit",
            "project": "onedev/core",
            "message": "Fix login bug",
            "date": "2024-01-15T10:30:00Z"
        }),
        serde_json::json!({
            "type": "issue",
            "project": "onedev/web",
            "title": "Add dark mode",
            "date": "2024-01-14T16:00:00Z"
        }),
        serde_json::json!({
            "type": "mr",
            "project": "onedev/api",
            "title": "Update API docs",
            "date": "2024-01-13T14:00:00Z"
        }),
    ]
}

/// Get SSH keys
async fn get_ssh_keys(_state: &WebState) -> Vec<serde_json::Value> {
    vec![serde_json::json!({
        "id": 1,
        "title": "MacBook Pro",
        "key_type": "ED25519",
        "fingerprint": "SHA256:abc123...",
        "created_at": "2024-01-01T10:00:00Z",
        "last_used": "2024-01-15T09:00:00Z"
    })]
}

/// Get GPG keys
async fn get_gpg_keys(_state: &WebState) -> Vec<serde_json::Value> {
    vec![serde_json::json!({
        "id": 1,
        "key_id": "ABCDEF1234567890",
        "name": "Admin User",
        "email": "admin@example.com",
        "created_at": "2024-01-01T10:00:00Z",
        "expires_at": "2025-01-01T10:00:00Z"
    })]
}

/// Get access tokens
async fn get_access_tokens(_state: &WebState) -> Vec<serde_json::Value> {
    vec![serde_json::json!({
        "id": 1,
        "name": "CI/CD Token",
        "scopes": ["read_repository", "write_repository", "read_build"],
        "created_at": "2024-01-01T10:00:00Z",
        "expires_at": "2025-01-01T10:00:00Z",
        "last_used": "2024-01-15T10:00:00Z"
    })]
}

/// Get sessions
async fn get_sessions(_state: &WebState) -> Vec<serde_json::Value> {
    vec![
        serde_json::json!({
            "id": "current",
            "device": "MacBook Pro - Chrome",
            "ip": "192.168.1.100",
            "location": "San Francisco, US",
            "created_at": "2024-01-15T08:00:00Z",
            "last_active": "2024-01-15T10:30:00Z",
            "current": true
        }),
        serde_json::json!({
            "id": "sess_2",
            "device": "iPhone - Safari",
            "ip": "192.168.1.101",
            "location": "San Francisco, US",
            "created_at": "2024-01-14T10:00:00Z",
            "last_active": "2024-01-14T18:00:00Z",
            "current": false
        }),
    ]
}

/// Get current user (placeholder)
async fn get_current_user(_state: &WebState) -> serde_json::Value {
    serde_json::json!({
        "id": "1",
        "username": "admin",
        "full_name": "Admin User",
        "avatar_url": null,
        "email": "admin@example.com",
        "bio": "OneDev Core Developer",
        "website": "https://onedev.example.com",
        "location": "San Francisco, CA",
        "theme": "auto",
        "language": "en",
        "timezone": "America/Los_Angeles",
        "created_at": "2024-01-01T00:00:00Z",
        "two_fa_enabled": false
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
