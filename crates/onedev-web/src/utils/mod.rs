//! Utility Functions for Web UI

/// Flash message for user notifications
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct FlashMessage {
    pub level: FlashLevel,
    pub message: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum FlashLevel {
    Success,
    Error,
    Warning,
    Info,
}

impl FlashMessage {
    pub fn success(message: impl Into<String>) -> Self {
        Self {
            level: FlashLevel::Success,
            message: message.into(),
        }
    }

    pub fn error(message: impl Into<String>) -> Self {
        Self {
            level: FlashLevel::Error,
            message: message.into(),
        }
    }

    pub fn warning(message: impl Into<String>) -> Self {
        Self {
            level: FlashLevel::Warning,
            message: message.into(),
        }
    }

    pub fn info(message: impl Into<String>) -> Self {
        Self {
            level: FlashLevel::Info,
            message: message.into(),
        }
    }
}

/// Flash messages extractor
#[derive(Default)]
pub struct FlashMessages(pub Vec<FlashMessage>);

impl FlashMessages {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push(&mut self, msg: FlashMessage) {
        self.0.push(msg);
    }

    pub fn success(&mut self, message: impl Into<String>) {
        self.push(FlashMessage::success(message));
    }

    pub fn error(&mut self, message: impl Into<String>) {
        self.push(FlashMessage::error(message));
    }

    pub fn warning(&mut self, message: impl Into<String>) {
        self.push(FlashMessage::warning(message));
    }

    pub fn info(&mut self, message: impl Into<String>) {
        self.push(FlashMessage::info(message));
    }
}

/// Pagination helper
#[derive(Debug, Clone, serde::Serialize)]
pub struct Pagination {
    pub page: u32,
    pub per_page: u32,
    pub total: u64,
    pub total_pages: u32,
    pub has_prev: bool,
    pub has_next: bool,
    pub prev_page: Option<u32>,
    pub next_page: Option<u32>,
    pub pages: Vec<PageLink>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct PageLink {
    pub number: u32,
    pub url: String,
    pub current: bool,
}

impl Pagination {
    pub fn new(page: u32, per_page: u32, total: u64, base_url: &str) -> Self {
        let total_pages = ((total as f64) / (per_page as f64)).ceil() as u32;
        let page = page.min(total_pages.max(1));

        let mut pages = Vec::new();
        let start = if page > 3 { page - 2 } else { 1 };
        let end = (start + 4).min(total_pages);

        for n in start..=end {
            pages.push(PageLink {
                number: n,
                url: format!("{}?page={}", base_url, n),
                current: n == page,
            });
        }

        Self {
            page,
            per_page,
            total,
            total_pages,
            has_prev: page > 1,
            has_next: page < total_pages,
            prev_page: if page > 1 { Some(page - 1) } else { None },
            next_page: if page < total_pages {
                Some(page + 1)
            } else {
                None
            },
            pages,
        }
    }
}

/// Form validation errors
#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct ValidationErrors {
    pub errors: std::collections::HashMap<String, Vec<String>>,
}

impl ValidationErrors {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add(&mut self, field: &str, message: &str) {
        self.errors
            .entry(field.to_string())
            .or_default()
            .push(message.to_string());
    }

    pub fn get(&self, field: &str) -> Option<&Vec<String>> {
        self.errors.get(field)
    }

    pub fn has_errors(&self) -> bool {
        !self.errors.is_empty()
    }

    pub fn is_empty(&self) -> bool {
        self.errors.is_empty()
    }
}

/// Breadcrumb helper
#[derive(Debug, Clone, serde::Serialize)]
pub struct Breadcrumb {
    pub label: String,
    pub url: Option<String>,
    pub active: bool,
}

pub fn breadcrumbs(items: Vec<(&str, Option<&str>)>) -> Vec<Breadcrumb> {
    let len = items.len();
    items
        .into_iter()
        .enumerate()
        .map(|(i, (label, url))| Breadcrumb {
            label: label.to_string(),
            url: url.map(|s| s.to_string()),
            active: i == len - 1,
        })
        .collect()
}

/// Format bytes as human readable
pub fn format_bytes(bytes: u64) -> String {
    const UNITS: &[&str] = &["B", "KB", "MB", "GB", "TB"];
    let mut size = bytes as f64;
    let mut unit = 0;

    while size >= 1024.0 && unit < UNITS.len() - 1 {
        size /= 1024.0;
        unit += 1;
    }

    if unit == 0 {
        format!("{} {}", bytes, UNITS[unit])
    } else {
        format!("{:.1} {}", size, UNITS[unit])
    }
}

/// Format duration as human readable
pub fn format_duration(seconds: u64) -> String {
    if seconds < 60 {
        format!("{}s", seconds)
    } else if seconds < 3600 {
        format!("{}m {}s", seconds / 60, seconds % 60)
    } else if seconds < 86400 {
        format!("{}h {}m", seconds / 3600, (seconds % 3600) / 60)
    } else {
        format!("{}d {}h", seconds / 86400, (seconds % 86400) / 3600)
    }
}

/// Truncate string with ellipsis
pub fn truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        let truncated = &s[..max_len];
        let last_space = truncated.rfind(' ').unwrap_or(max_len);
        format!("{}...", &truncated[..last_space])
    }
}

/// Generate random string
pub fn random_string(len: usize) -> String {
    use rand::Rng;
    const CHARSET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";
    let mut rng = rand::rng();
    (0..len)
        .map(|_| CHARSET[rng.random_range(0..CHARSET.len())] as char)
        .collect()
}

/// Slugify string for URLs
pub fn slugify(s: &str) -> String {
    s.to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '-' })
        .collect::<String>()
        .split('-')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}

/// Check if string is valid identifier
pub fn is_valid_identifier(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 255
        && s.chars()
            .all(|c| c.is_alphanumeric() || c == '-' || c == '_' || c == '.')
        && !s.starts_with('-')
        && !s.ends_with('-')
        && !s.contains("--")
}

/// Sanitize HTML for safe display
pub fn sanitize_html(html: &str) -> String {
    // In real implementation, use a proper HTML sanitizer like ammonia
    // For now, basic escaping
    html_escape::encode_safe(html).to_string()
}

/// Get file extension from path
pub fn get_file_extension(path: &str) -> Option<&str> {
    std::path::Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
}

/// Get language from file extension
pub fn get_language_from_extension(ext: &str) -> &'static str {
    match ext.to_lowercase().as_str() {
        "rs" => "rust",
        "js" | "jsx" => "javascript",
        "ts" | "tsx" => "typescript",
        "py" => "python",
        "go" => "go",
        "java" => "java",
        "cpp" | "cc" | "cxx" => "cpp",
        "c" => "c",
        "cs" => "csharp",
        "rb" => "ruby",
        "php" => "php",
        "swift" => "swift",
        "kt" => "kotlin",
        "scala" => "scala",
        "clj" => "clojure",
        "hs" => "haskell",
        "ml" => "ocaml",
        "fs" => "fsharp",
        "dart" => "dart",
        "lua" => "lua",
        "r" => "r",
        "jl" => "julia",
        "ex" | "exs" => "elixir",
        "erl" => "erlang",
        "pl" => "perl",
        "sh" | "bash" => "bash",
        "ps1" => "powershell",
        "sql" => "sql",
        "html" | "htm" => "html",
        "css" => "css",
        "scss" | "sass" => "scss",
        "less" => "less",
        "xml" => "xml",
        "json" => "json",
        "yaml" | "yml" => "yaml",
        "toml" => "toml",
        "ini" => "ini",
        "cfg" => "ini",
        "conf" => "ini",
        "md" | "markdown" => "markdown",
        "txt" => "text",
        "dockerfile" => "dockerfile",
        "makefile" => "makefile",
        _ => "text",
    }
}
