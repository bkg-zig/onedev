//! Template Helpers and Functions

use tera::{Function, Value, Result as TeraResult};
use std::collections::HashMap;

/// Register all custom template functions
pub fn register_functions(tera: &mut tera::Tera) {
    tera.register_function("url_for", UrlForFunction);
    tera.register_function("asset_url", AssetUrlFunction);
    tera.register_function("gravatar", GravatarFunction);
    tera.register_function("timeago", TimeAgoFunction);
    tera.register_function("pluralize", PluralizeFunction);
    tera.register_function("truncate_html", TruncateHtmlFunction);
    tera.register_function("highlight", HighlightFunction);
}

/// URL for function - generates URLs for routes
pub struct UrlForFunction;

impl Function for UrlForFunction {
    fn call(&self, args: &HashMap<String, Value>) -> TeraResult<Value> {
        let route = args.get("route")
            .and_then(|v| v.as_str())
            .ok_or_else(|| tera::Error::msg("route argument required"))?;

        let mut url = format!("/{}", route.trim_start_matches('/'));

        // Add query parameters
        if let Some(params) = args.get("params") {
            if let Some(obj) = params.as_object() {
                let query: Vec<String> = obj.iter()
                    .filter_map(|(k, v)| v.as_str().map(|s| format!("{}={}", k, s)))
                    .collect();
                if !query.is_empty() {
                    url.push('?');
                    url.push_str(&query.join("&"));
                }
            }
        }

        Ok(Value::String(url))
    }
}

/// Asset URL function - generates URLs for static assets
pub struct AssetUrlFunction;

impl Function for AssetUrlFunction {
    fn call(&self, args: &HashMap<String, Value>) -> TeraResult<Value> {
        let path = args.get("path")
            .and_then(|v| v.as_str())
            .ok_or_else(|| tera::Error::msg("path argument required"))?;

        let mut url = format!("/static/{}", path.trim_start_matches('/'));

        // Add cache busting in production
        if let Some(version) = args.get("version") {
            if let Some(v) = version.as_str() {
                url.push('?');
                url.push_str("v=");
                url.push_str(v);
            }
        }

        Ok(Value::String(url))
    }
}

/// Gravatar function - generates Gravatar URLs
pub struct GravatarFunction;

impl Function for GravatarFunction {
    fn call(&self, args: &HashMap<String, Value>) -> TeraResult<Value> {
        let email = args.get("email")
            .and_then(|v| v.as_str())
            .ok_or_else(|| tera::Error::msg("email argument required"))?;

        let size = args.get("size")
            .and_then(|v| v.as_u64())
            .unwrap_or(80) as u32;

        let default = args.get("default")
            .and_then(|v| v.as_str())
            .unwrap_or("identicon");

        let hash = format!("{:x}", md5::compute(email.trim().to_lowercase().as_bytes()));

        let url = format!(
            "https://www.gravatar.com/avatar/{}?s={}&d={}",
            hash, size, default
        );

        Ok(Value::String(url))
    }
}

/// Time ago function - human readable time difference
pub struct TimeAgoFunction;

impl Function for TimeAgoFunction {
    fn call(&self, args: &HashMap<String, Value>) -> TeraResult<Value> {
        let timestamp = args.get("timestamp")
            .and_then(|v| v.as_str())
            .ok_or_else(|| tera::Error::msg("timestamp argument required"))?;

        let dt = chrono::DateTime::parse_from_rfc3339(timestamp)
            .or_else(|_| timestamp.parse::<i64>().map(|ts| chrono::DateTime::from_timestamp(ts, 0).unwrap().fixed_offset()))
            .map_err(|_| tera::Error::msg("invalid timestamp"))?;

        let now = chrono::Utc::now();
        let duration = now.signed_duration_since(dt.with_timezone(&chrono::Utc));

        let result = if duration.num_seconds() < 60 {
            "just now".to_string()
        } else if duration.num_minutes() < 60 {
            format!("{}m ago", duration.num_minutes())
        } else if duration.num_hours() < 24 {
            format!("{}h ago", duration.num_hours())
        } else if duration.num_days() < 30 {
            format!("{}d ago", duration.num_days())
        } else if duration.num_days() < 365 {
            format!("{}mo ago", duration.num_days() / 30)
        } else {
            format!("{}y ago", duration.num_days() / 365)
        };

        Ok(Value::String(result))
    }
}

/// Pluralize function
pub struct PluralizeFunction;

impl Function for PluralizeFunction {
    fn call(&self, args: &HashMap<String, Value>) -> TeraResult<Value> {
        let count = args.get("count")
            .and_then(|v| v.as_u64())
            .unwrap_or(0);

        let singular = args.get("singular")
            .and_then(|v| v.as_str())
            .unwrap_or("item");

        let plural_owned = format!("{}s", singular);
        let plural = args.get("plural")
            .and_then(|v| v.as_str())
            .unwrap_or(&plural_owned);

        let result = if count == 1 {
            singular.to_string()
        } else {
            plural.to_string()
        };

        Ok(Value::String(format!("{} {}", count, result)))
    }
}

/// Truncate HTML function
pub struct TruncateHtmlFunction;

impl Function for TruncateHtmlFunction {
    fn call(&self, args: &HashMap<String, Value>) -> TeraResult<Value> {
        let html = args.get("html")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        let length = args.get("length")
            .and_then(|v| v.as_u64())
            .unwrap_or(100) as usize;

        // Simple implementation - strip tags then truncate
        let text = html_escape::encode_safe(html).to_string();
        let text = text.replace('<', " <").replace('>', "> ");

        if text.len() > length {
            let truncated = &text[..length];
            let last_space = truncated.rfind(' ').unwrap_or(length);
            Ok(Value::String(format!("{}...", &truncated[..last_space])))
        } else {
            Ok(Value::String(text))
        }
    }
}

/// Syntax highlight function
pub struct HighlightFunction;

impl Function for HighlightFunction {
    fn call(&self, args: &HashMap<String, Value>) -> TeraResult<Value> {
        let code = args.get("code")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        let language = args.get("language")
            .and_then(|v| v.as_str())
            .unwrap_or("text");

        // In real implementation, use syntect for highlighting
        // For now, just wrap in pre/code tags
        let escaped = html_escape::encode_safe(code).to_string();
        let highlighted = format!(
            "<pre><code class=\"language-{}\">{}</code></pre>",
            language, escaped
        );

        Ok(Value::String(highlighted))
    }
}