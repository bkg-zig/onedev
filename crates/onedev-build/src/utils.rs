//! Small helpers for the build layer.

use std::path::Path;

/// Wildcard match of a whole string: `*` matches any run of characters
/// (including `/`), `?` exactly one character (vanilla `WildcardUtils.matchString`).
pub fn match_string(pattern: &str, value: &str) -> bool {
    let pattern: Vec<char> = pattern.chars().collect();
    let value: Vec<char> = value.chars().collect();
    segment_match(&pattern, &value)
}

/// Kept for callers that predate [`match_string`].
pub fn glob_matches(pattern: &str, value: &str) -> bool {
    match_string(pattern, value)
}

fn segment_match(pattern: &[char], value: &[char]) -> bool {
    let mut prev = vec![false; value.len() + 1];
    prev[0] = true;
    for &ch in pattern {
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

/// Path wildcard match (vanilla `WildcardUtils.matchPath`, Ant semantics):
/// `/` separates segments, `**` matches any number of whole segments,
/// `*` and `?` never cross a `/`. A pattern ending in `/` means `<pattern>**`.
pub fn match_path(pattern: &str, path: &str) -> bool {
    let mut pattern = pattern.to_owned();
    if pattern.ends_with('/') {
        pattern.push_str("**");
    }
    let pattern_segments: Vec<Vec<char>> = pattern.split('/').map(|s| s.chars().collect()).collect();
    let path_segments: Vec<Vec<char>> = path.split('/').map(|s| s.chars().collect()).collect();
    let is_double_star = |segment: &Vec<char>| segment.len() == 2 && segment[0] == '*' && segment[1] == '*';

    // reachable[j]: the first i pattern segments can consume the first j path segments
    let mut reachable = vec![false; path_segments.len() + 1];
    reachable[0] = true;
    for pattern_segment in &pattern_segments {
        let mut next = vec![false; path_segments.len() + 1];
        if is_double_star(pattern_segment) {
            let mut any = false;
            for j in 0..=path_segments.len() {
                any |= reachable[j];
                next[j] = any;
            }
        } else {
            for j in 1..=path_segments.len() {
                next[j] = reachable[j - 1] && segment_match(pattern_segment, &path_segments[j - 1]);
            }
        }
        reachable = next;
    }
    reachable[path_segments.len()]
}

/// Space separated include/exclude patterns (vanilla `PatternSet`): a leading
/// `-` excludes, `\"..\"` quotes a pattern that contains spaces. No include
/// pattern means "everything that is not excluded".
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PatternSet {
    pub includes: Vec<String>,
    pub excludes: Vec<String>,
}

impl PatternSet {
    pub fn parse(text: &str) -> Self {
        let mut set = PatternSet::default();
        let mut token = String::new();
        let mut quoted = false;
        let mut started = false;
        let mut flush = |token: &mut String, started: &mut bool| {
            if *started {
                let value = std::mem::take(token);
                match value.strip_prefix('-') {
                    Some(excluded) if !excluded.is_empty() => set.excludes.push(excluded.to_owned()),
                    _ if value.is_empty() => {}
                    _ => set.includes.push(value),
                }
                *started = false;
            }
        };
        let mut chars = text.chars();
        while let Some(c) = chars.next() {
            match c {
                '\\' => {
                    started = true;
                    if let Some(escaped) = chars.next() {
                        token.push(escaped);
                    }
                }
                '"' => {
                    quoted = !quoted;
                    started = true;
                }
                c if c.is_whitespace() && !quoted => flush(&mut token, &mut started),
                c => {
                    started = true;
                    token.push(c);
                }
            }
        }
        flush(&mut token, &mut started);
        set
    }

    pub fn is_empty(&self) -> bool {
        self.includes.is_empty() && self.excludes.is_empty()
    }

    /// Match with an arbitrary matcher ([`match_path`] or [`match_string`]).
    pub fn matches(&self, matcher: impl Fn(&str, &str) -> bool, value: &str) -> bool {
        if self.excludes.iter().any(|pattern| matcher(pattern, value)) {
            return false;
        }
        self.includes.is_empty() || self.includes.iter().any(|pattern| matcher(pattern, value))
    }

    /// Path semantics (branches, tags, file paths).
    pub fn matches_path(&self, value: &str) -> bool {
        self.matches(match_path, value)
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn string_wildcards_cross_slashes() {
        assert!(match_string("main", "main"));
        assert!(!match_string("main", "main2"));
        assert!(match_string("rel*", "release/1.0"));
        assert!(match_string("a?c", "abc"));
        assert!(!match_string("a?c", "ac"));
        assert!(match_string("*", ""));
        assert!(match_string("*-*", "build-7"));
    }

    #[test]
    fn path_wildcards_follow_ant_semantics() {
        // `*` stays inside one segment, `**` spans segments
        assert!(match_path("release/*", "release/1.0"));
        assert!(!match_path("release/*", "release/1.0/hotfix"));
        assert!(match_path("release/**", "release/1.0/hotfix"));
        assert!(match_path("release/**", "release"));
        assert!(match_path("**", "any/branch/name"));
        assert!(match_path("**", "main"));
        assert!(match_path("**/fix", "a/b/fix"));
        assert!(match_path("**/fix", "fix"));
        assert!(!match_path("**/fix", "a/b/fixes"));
        assert!(match_path("a/**/z", "a/z"));
        assert!(match_path("a/**/z", "a/b/c/z"));
        assert!(!match_path("a/**/z", "a/b/c/y"));
        assert!(match_path("feature/?", "feature/x"));
        assert!(!match_path("feature/?", "feature/xy"));
        assert!(match_path("docs/", "docs/a/b.md"));
        assert!(match_path("main", "main"));
        assert!(!match_path("main", "main/x"));
        assert!(match_path("**/*.rs", "src/a/lib.rs"));
        assert!(!match_path("*.rs", "src/lib.rs"));
    }

    #[test]
    fn pattern_sets_support_excludes_and_quoting() {
        let set = PatternSet::parse("main release/** -release/old/**");
        assert!(set.matches_path("main"));
        assert!(set.matches_path("release/1"));
        assert!(!set.matches_path("release/old/1"));
        assert!(!set.matches_path("dev"));

        let only_excludes = PatternSet::parse("-experimental/**");
        assert!(only_excludes.matches_path("main"));
        assert!(!only_excludes.matches_path("experimental/x"));

        assert!(PatternSet::parse("").matches_path("anything"));
        assert!(PatternSet::parse("   ").is_empty());

        let quoted = PatternSet::parse("\"my branch\" other");
        assert_eq!(quoted.includes, vec!["my branch".to_owned(), "other".to_owned()]);
        assert!(quoted.matches_path("my branch"));
    }
}
