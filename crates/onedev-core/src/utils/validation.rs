//! Validation patterns used by the `validator` derives of the domain models.

use regex::Regex;
use std::sync::LazyLock;

/// One path segment: starts and ends with an alphanumeric character,
/// `_`, `.` and `-` are allowed in between.
const SEGMENT: &str = r"[A-Za-z0-9](?:[A-Za-z0-9_.-]*[A-Za-z0-9])?";

fn compile(pattern: String) -> Regex {
    Regex::new(&pattern).expect("built-in validation pattern must be a valid regex")
}

/// User name (single segment).
pub static USERNAME_REGEX: LazyLock<Regex> = LazyLock::new(|| compile(format!("^{SEGMENT}$")));

/// Workspace name (single segment).
pub static WORKSPACE_NAME_REGEX: LazyLock<Regex> = LazyLock::new(|| compile(format!("^{SEGMENT}$")));

/// Project path: one or more segments separated by `/`.
pub static PROJECT_PATH_REGEX: LazyLock<Regex> =
    LazyLock::new(|| compile(format!("^{SEGMENT}(?:/{SEGMENT})*$")));

/// Group path: one or more segments separated by `/`.
pub static GROUP_PATH_REGEX: LazyLock<Regex> =
    LazyLock::new(|| compile(format!("^{SEGMENT}(?:/{SEGMENT})*$")));

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn username_accepts_plain_names_and_rejects_edge_characters() {
        assert!(USERNAME_REGEX.is_match("alice"));
        assert!(USERNAME_REGEX.is_match("a.b-c_d"));
        assert!(!USERNAME_REGEX.is_match("-alice"));
        assert!(!USERNAME_REGEX.is_match("alice."));
        assert!(!USERNAME_REGEX.is_match("a/b"));
    }

    #[test]
    fn project_path_allows_nested_segments() {
        assert!(PROJECT_PATH_REGEX.is_match("group/sub/project"));
        assert!(!PROJECT_PATH_REGEX.is_match("group//project"));
        assert!(!PROJECT_PATH_REGEX.is_match("/project"));
    }
}
