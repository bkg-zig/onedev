//! Utility functions for authentication

use base64::{Engine as _, engine::general_purpose};
use rand::RngCore;
use secrecy::{SecretString, ExposeSecret};
use chrono::{DateTime, Utc};
use uuid::Uuid;

/// Generate a secure random string
pub fn generate_random_string(length: usize) -> String {
    const CHARSET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";
    let mut result = String::with_capacity(length);
    let mut bytes = vec![0u8; length];
    rand::rng().fill_bytes(&mut bytes);
    
    for &b in &bytes {
        result.push(CHARSET[(b as usize) % CHARSET.len()] as char);
    }
    result
}

/// Generate a secure random token (URL-safe base64)
pub fn generate_token(length: usize) -> String {
    let mut bytes = vec![0u8; length];
    rand::rng().fill_bytes(&mut bytes);
    general_purpose::URL_SAFE_NO_PAD.encode(bytes)
}

/// Generate a UUID v7 (time-ordered)
pub fn generate_uuid_v7() -> Uuid {
    Uuid::now_v7()
}

/// Parse a principal string (u:<id> or a:<id>)
pub fn parse_principal(principal: &str) -> Option<(PrincipalType, i64)> {
    if let Some(id_str) = principal.strip_prefix("u:") {
        id_str.parse::<i64>().ok().map(|id| (PrincipalType::User, id))
    } else if let Some(id_str) = principal.strip_prefix("a:") {
        id_str.parse::<i64>().ok().map(|id| (PrincipalType::AccessToken, id))
    } else {
        None
    }
}

/// Principal type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrincipalType {
    User,
    AccessToken,
}

/// Format a principal string
pub fn format_principal(principal_type: PrincipalType, id: i64) -> String {
    match principal_type {
        PrincipalType::User => format!("u:{}", id),
        PrincipalType::AccessToken => format!("a:{}", id),
    }
}

/// Constant-time string comparison (prevents timing attacks)
pub fn constant_time_eq(a: &str, b: &str) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut result = 0u8;
    for (ca, cb) in a.bytes().zip(b.bytes()) {
        result |= ca ^ cb;
    }
    result == 0
}

/// Securely compare two secret strings
pub fn secret_eq(a: &SecretString, b: &SecretString) -> bool {
    let a_bytes = a.expose_secret().as_bytes();
    let b_bytes = b.expose_secret().as_bytes();
    
    if a_bytes.len() != b_bytes.len() {
        return false;
    }
    
    let mut result = 0u8;
    for (ca, cb) in a_bytes.iter().zip(b_bytes.iter()) {
        result |= ca ^ cb;
    }
    result == 0
}

/// Check if a timestamp is expired
pub fn is_expired(expires_at: DateTime<Utc>) -> bool {
    Utc::now() > expires_at
}

/// Calculate time until expiration
pub fn time_until_expiration(expires_at: DateTime<Utc>) -> Option<chrono::Duration> {
    let now = Utc::now();
    if now >= expires_at {
        None
    } else {
        Some(expires_at - now)
    }
}

/// Sanitize a string for logging (remove secrets)
pub fn sanitize_for_log(input: &str) -> String {
    // Replace potential secrets with ***
    let patterns = [
        ("password", "***"),
        ("token", "***"),
        ("secret", "***"),
        ("key", "***"),
        ("authorization", "***"),
    ];
    
    let mut result = input.to_string();
    for (pattern, replacement) in patterns {
        let regex = regex::Regex::new(&format!(r#"(?i){}=["']?[^"'\s]+"#", pattern)).ok();
        if let Some(re) = regex {
            result = re.replace_all(&result, format!("{}=\"{}\"", pattern, replacement)).to_string();
        }
    }
    result
}

/// Extract client IP from headers
pub fn extract_client_ip(headers: &axum::http::HeaderMap) -> Option<String> {
    // Check X-Forwarded-For
    if let Some(forwarded) = headers.get("X-Forwarded-For") {
        if let Ok(ip) = forwarded.to_str() {
            return Some(ip.split(',').next().unwrap_or(ip).trim().to_string());
        }
    }
    
    // Check X-Real-IP
    if let Some(real_ip) = headers.get("X-Real-IP") {
        if let Ok(ip) = real_ip.to_str() {
            return Some(ip.to_string());
        }
    }
    
    // Check CF-Connecting-IP (Cloudflare)
    if let Some(cf_ip) = headers.get("CF-Connecting-IP") {
        if let Ok(ip) = cf_ip.to_str() {
            return Some(ip.to_string());
        }
    }
    
    None
}

/// Parse User-Agent string for basic info
pub fn parse_user_agent(user_agent: &str) -> UserAgentInfo {
    // Simple parsing - in production you'd use a proper UA parser
    let mut info = UserAgentInfo::default();
    
    let ua_lower = user_agent.to_lowercase();
    
    // Browser detection
    if ua_lower.contains("firefox") {
        info.browser = Some("Firefox".to_string());
    } else if ua_lower.contains("chrome") && !ua_lower.contains("edg") {
        info.browser = Some("Chrome".to_string());
    } else if ua_lower.contains("safari") && !ua_lower.contains("chrome") {
        info.browser = Some("Safari".to_string());
    } else if ua_lower.contains("edg") {
        info.browser = Some("Edge".to_string());
    }
    
    // OS detection
    if ua_lower.contains("windows") {
        info.os = Some("Windows".to_string());
    } else if ua_lower.contains("mac") {
        info.os = Some("macOS".to_string());
    } else if ua_lower.contains("linux") {
        info.os = Some("Linux".to_string());
    } else if ua_lower.contains("android") {
        info.os = Some("Android".to_string());
    } else if ua_lower.contains("ios") || ua_lower.contains("iphone") {
        info.os = Some("iOS".to_string());
    }
    
    // Device type
    if ua_lower.contains("mobile") {
        info.device_type = Some("Mobile".to_string());
    } else if ua_lower.contains("tablet") {
        info.device_type = Some("Tablet".to_string());
    } else {
        info.device_type = Some("Desktop".to_string());
    }
    
    info.raw = user_agent.to_string();
    info
}

/// Parsed User-Agent information
#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct UserAgentInfo {
    pub browser: Option<String>,
    pub os: Option<String>,
    pub device_type: Option<String>,
    pub raw: String,
}