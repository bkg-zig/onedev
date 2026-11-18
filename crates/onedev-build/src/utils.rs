//! Small helpers for the build layer.

use std::path::Path;

/// Wildcard match with `*` and `?` for trigger patterns.
pub fn glob_matches(pattern: &str, value: &str) -> bool {
    let pattern: Vec<char> = pattern.chars().collect();
    let value: Vec<char> = value.chars().collect();
    let mut prev = vec![false; value.len() + 1];
    prev[0] = true;
    for ch in pattern {
        let mut current = vec![false; value.len() + 1];
        if ch == '*' {
            current[0] = prev[0];
            for i in 1..=value.len() {
                current[i] = prev[i] || current[i - 1];
            }
        } else {
            for i in 1..=value.len() {
                current[i] = prev[i - 1] && (ch == '?' || ch == value[i - 1]);
            }
        }
        prev = current;
    }
    prev[value.len()]
}

/// First existing build-spec file in a checkout: `.onedev-buildspec.yml`
/// (vanilla name) with `.yaml` accepted as well.
pub fn find_buildspec(work_dir: &Path) -> Option<std::path::PathBuf> {
    for name in [".onedev-buildspec.yml", ".onedev-buildspec.yaml"] {
        let candidate = work_dir.join(name);
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    None
}
