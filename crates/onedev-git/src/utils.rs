//! Small, dependency-free helpers used by the Git services.

use std::io::Write;
use std::path::{Component, Path, PathBuf};
use std::process::{Command, Stdio};

use crate::errors::{GitError, GitResult};
use crate::models::{LfsObject, LfsPointer};

const LFS_POINTER_VERSION: &str = "version https://git-lfs.github.com/spec/v1";

/// Validate and normalize a project path used to locate a bare repository.
///
/// Only relative, slash-separated path segments consisting of letters,
/// digits, `.`, `_` and `-` are accepted. `.` and `..` are rejected so that a
/// project can never escape the configured repository root.
pub fn normalize_project_path(project_path: &str) -> GitResult<String> {
    let trimmed = project_path.trim().trim_end_matches('/').trim_end_matches(".git");
    if trimmed.is_empty() {
        return Err(GitError::InvalidProjectPath("path is empty".to_owned()));
    }

    let mut segments = Vec::new();
    for segment in trimmed.split('/') {
        if segment.is_empty()
            || segment == "."
            || segment == ".."
            || segment.len() > 128
            || !segment
                .chars()
                .enumerate()
                .all(|(i, c)| c.is_ascii_alphanumeric() || c == '-' || c == '_' || (c == '.' && i > 0))
        {
            return Err(GitError::InvalidProjectPath(trimmed.to_owned()));
        }
        segments.push(segment.to_owned());
    }

    Ok(segments.join("/"))
}

/// Absolute path of the bare repository for a project path.
pub fn repository_path(root: &Path, project_path: &str) -> GitResult<PathBuf> {
    let normalized = normalize_project_path(project_path)?;
    Ok(root.join(format!("{normalized}.git")))
}

/// Reject path traversal in paths taken from a revision or an LFS request.
pub fn is_safe_relative_path(path: &str) -> bool {
    let path = Path::new(path);
    !path.is_absolute()
        && path
            .components()
            .all(|c| matches!(c, Component::Normal(_)))
}

/// Validate a full ref name (`refs/heads/...`, `refs/tags/...`).
pub fn validate_ref_name(name: &str) -> GitResult<()> {
    let invalid = name.is_empty()
        || name.starts_with('-')
        || name.starts_with('.')
        || name.ends_with('.')
        || name.ends_with('/')
        || name.ends_with(".lock")
        || name.contains("..")
        || name.contains("//")
        || name.contains("@{")
        || name.contains('\\')
        || name.chars().any(|c| c.is_ascii_control() || "~^:?*[".contains(c));

    if invalid {
        return Err(GitError::InvalidRef(name.to_owned()));
    }
    if name != "HEAD" && !name.starts_with("refs/") {
        return Err(GitError::InvalidRef(format!("not a full ref: {name}")));
    }
    Ok(())
}

/// Validate a full hex object id (SHA-1 or SHA-256).
pub fn validate_object_id(oid: &str) -> GitResult<()> {
    if (oid.len() == 40 || oid.len() == 64) && oid.bytes().all(|b| b.is_ascii_hexdigit()) {
        Ok(())
    } else {
        Err(GitError::Object(format!("invalid object id: {oid}")))
    }
}

/// Simple glob matching for branch/tag protection (`*` and `?`).
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

/// Parse an LFS pointer file. Returns `None` for regular file content.
pub fn parse_lfs_pointer(content: &[u8]) -> Option<LfsPointer> {
    let text = std::str::from_utf8(content).ok()?;
    let mut lines = text.lines();
    if lines.next()? != LFS_POINTER_VERSION {
        return None;
    }

    let mut oid = None;
    let mut size = None;
    for line in lines {
        if let Some(value) = line.strip_prefix("oid sha256:") {
            if !value.is_empty() && value.len() == 64 && value.bytes().all(|b| b.is_ascii_hexdigit()) {
                oid = Some(value.to_owned());
            } else {
                return None;
            }
        } else if let Some(value) = line.strip_prefix("size ") {
            size = value.parse::<u64>().ok();
        }
    }

    match (oid, size) {
        (Some(oid), Some(size)) => Some(LfsPointer { oid, size }),
        _ => None,
    }
}

/// Render an LFS pointer file (content as stored in the Git blob).
pub fn format_lfs_pointer(oid: &str, size: u64) -> String {
    format!("{LFS_POINTER_VERSION}\noid sha256:{oid}\nsize {size}\n")
}

/// SHA-256 of the object, hex encoded; used to verify uploaded LFS objects.
pub fn sha256_hex(content: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(content);
    hex_encode(&hasher.finalize())
}

pub fn hex_encode(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(HEX[(byte >> 4) as usize] as char);
        out.push(HEX[(byte & 0x0f) as usize] as char);
    }
    out
}

/// One pkt-line as defined by the Git pack protocol.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PktLine {
    Data(Vec<u8>),
    Flush,
    Delimiter,
    ResponseEnd,
}

/// Decode a pkt-line stream. Malformed trailing bytes are reported as an error.
pub fn decode_pkt_lines(input: &[u8]) -> GitResult<Vec<PktLine>> {
    let mut result = Vec::new();
    let mut offset = 0;
    while offset < input.len() {
        if input.len() - offset < 4 {
            return Err(GitError::Protocol("truncated pkt-line header".to_owned()));
        }
        let header = &input[offset..offset + 4];
        let len = std::str::from_utf8(header)
            .ok()
            .and_then(|s| usize::from_str_radix(s, 16).ok())
            .ok_or_else(|| GitError::Protocol("invalid pkt-line length".to_owned()))?;
        offset += 4;

        match len {
            0 => result.push(PktLine::Flush),
            1 => result.push(PktLine::Delimiter),
            2 => result.push(PktLine::ResponseEnd),
            3..=4 => return Err(GitError::Protocol("invalid pkt-line length".to_owned())),
            n => {
                if input.len() - offset < n - 4 {
                    return Err(GitError::Protocol("truncated pkt-line payload".to_owned()));
                }
                result.push(PktLine::Data(input[offset..offset + n - 4].to_vec()));
                offset += n - 4;
            }
        }
    }
    Ok(result)
}

/// Encode a single pkt-line payload.
pub fn encode_pkt_line(data: &[u8]) -> Vec<u8> {
    let len = data.len() + 4;
    let mut out = format!("{len:04x}").into_bytes();
    out.extend_from_slice(data);
    out
}

/// Serialize an LFS object for the batch API.
pub fn lfs_object(oid: &str, size: u64) -> LfsObject {
    LfsObject { oid: oid.to_owned(), size }
}

/// Run a Git command against a repository, returning stdout.
///
/// Repository hooks are explicitly disabled so that only the Rust hook layer
/// can accept or reject a push.
pub(crate) fn run_git(
    git_binary: &Path,
    repo: &Path,
    args: &[&str],
    stdin: Option<&[u8]>,
) -> GitResult<Vec<u8>> {
    let mut command = Command::new(git_binary);
    command
        .arg("-c")
        .arg("core.hooksPath=/dev/null")
        .arg("--git-dir")
        .arg(repo)
        .args(args)
        .stdin(if stdin.is_some() { Stdio::piped() } else { Stdio::null() })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let mut child = command.spawn().map_err(|err| {
        GitError::msg(format!("failed to spawn {}: {err}", git_binary.display()))
    })?;
    // Write stdin on a separate thread so a large pack cannot deadlock
    // against the stdout pipe.
    let writer = stdin.map(|input| {
        let mut pipe = child.stdin.take();
        let owned = input.to_vec();
        std::thread::spawn(move || {
            if let Some(mut pipe) = pipe.take() {
                let _ = pipe.write_all(&owned);
            }
        })
    });
    let output = child.wait_with_output()?;
    if let Some(writer) = writer {
        let _ = writer.join();
    }
    if !output.status.success() {
        return Err(GitError::CommandFailed {
            command: format!("git {}", args.join(" ")),
            status: output.status.to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).trim().to_owned(),
        });
    }
    Ok(output.stdout)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn project_paths_are_confined_to_the_root() {
        assert_eq!(normalize_project_path("foo/bar").unwrap(), "foo/bar");
        assert_eq!(normalize_project_path("foo/bar.git/").unwrap(), "foo/bar");
        assert!(normalize_project_path("../foo").is_err());
        assert!(normalize_project_path("foo/../bar").is_err());
        assert!(normalize_project_path("/etc/passwd").is_err());
        assert!(normalize_project_path("foo//bar").is_err());
    }

    #[test]
    fn ref_names_follow_git_rules() {
        assert!(validate_ref_name("refs/heads/main").is_ok());
        assert!(validate_ref_name("refs/heads/feature/x").is_ok());
        assert!(validate_ref_name("refs/heads/a..b").is_err());
        assert!(validate_ref_name("refs/heads/main.lock").is_err());
        assert!(validate_ref_name("main").is_err());
    }

    #[test]
    fn lfs_pointer_round_trip() {
        let oid = "a".repeat(64);
        let pointer = format_lfs_pointer(&oid, 42);
        let parsed = parse_lfs_pointer(pointer.as_bytes()).unwrap();
        assert_eq!(parsed.oid, oid);
        assert_eq!(parsed.size, 42);
        assert!(parse_lfs_pointer(b"not a pointer\n").is_none());
    }

    #[test]
    fn pkt_lines_decode() {
        let data = b"0000";
        let encoded = [encode_pkt_line(b"hello"), data.to_vec()].concat();
        assert_eq!(
            decode_pkt_lines(&encoded).unwrap(),
            vec![PktLine::Data(b"hello".to_vec()), PktLine::Flush]
        );
    }

    #[test]
    fn glob_patterns_match() {
        assert!(glob_matches("main", "main"));
        assert!(glob_matches("release/*", "release/1.0"));
        assert!(!glob_matches("release/*", "feature/1.0"));
    }
}
