//! Authentication Handlers

use axum::{
    extract::State,
    http::{HeaderValue, StatusCode},
    response::{Html, IntoResponse, Redirect, Response},
    Form,
};
use serde::{Deserialize, Serialize};
use tera::Context;
use crate::WebState;

/// Login page
pub async fn login_page(State(state): State<WebState>) -> impl IntoResponse {
    let mut context = Context::new();
    context.insert("title", "Sign In");
    render_template(&state, "auth/login.html", context).await
}

/// Login handler
pub async fn login_handler(
    State(state): State<WebState>,
    Form(form): Form<LoginForm>,
) -> impl IntoResponse {
    // Validate credentials (placeholder)
    if form.username == "admin" && form.password == "admin" {
        // Set session cookie
        let mut response = Response::new(axum::body::Body::empty());
        *response.status_mut() = StatusCode::FOUND;
        response.headers_mut().insert(
            axum::http::header::LOCATION,
            HeaderValue::from_static("/")
        );
        response.headers_mut().insert(
            axum::http::header::SET_COOKIE,
            HeaderValue::from_str(
                "session=abc123; HttpOnly; Secure; SameSite=Lax; Path=/; Max-Age=86400"
            ).unwrap()
        );
        return response.into_response();
    }

    // Invalid credentials
    let mut context = Context::new();
    context.insert("title", "Sign In");
    context.insert("error", "Invalid username or password");
    context.insert("username", &form.username);
    render_template(&state, "auth/login.html", context).await.into_response()
}

/// Logout handler
pub async fn logout_handler() -> impl IntoResponse {
    let mut response = Response::new(axum::body::Body::empty());
    *response.status_mut() = StatusCode::FOUND;
    response.headers_mut().insert(
        axum::http::header::LOCATION,
        HeaderValue::from_static("/login")
    );
    response.headers_mut().insert(
        axum::http::header::SET_COOKIE,
        HeaderValue::from_str(
            "session=; HttpOnly; Secure; SameSite=Lax; Path=/; Max-Age=0"
        ).unwrap()
    );
    response.into_response()
}

/// Register page
pub async fn register_page(State(state): State<WebState>) -> impl IntoResponse {
    let mut context = Context::new();
    context.insert("title", "Sign Up");
    render_template(&state, "auth/register.html", context).await
}

/// Register handler
pub async fn register_handler(
    State(state): State<WebState>,
    Form(form): Form<RegisterForm>,
) -> impl IntoResponse {
    // Validate form
    let mut errors = Vec::new();

    if form.username.len() < 3 {
        errors.push("Username must be at least 3 characters");
    }
    if form.email.is_empty() || !form.email.contains('@') {
        errors.push("Valid email is required");
    }
    if form.password.len() < 8 {
        errors.push("Password must be at least 8 characters");
    }
    if form.password != form.confirm_password {
        errors.push("Passwords do not match");
    }

    if !errors.is_empty() {
        let mut context = Context::new();
        context.insert("title", "Sign Up");
        context.insert("errors", &errors);
        context.insert("form", &form);
        return render_template(&state, "auth/register.html", context).await.into_response();
    }

    // Create user (placeholder)
    // Redirect to login
    Redirect::to("/login?registered=true").into_response()
}

#[derive(Debug, Deserialize, Serialize)]
pub struct LoginForm {
    pub username: String,
    pub password: String,
    pub remember: Option<bool>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct RegisterForm {
    pub username: String,
    pub email: String,
    pub password: String,
    pub confirm_password: String,
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