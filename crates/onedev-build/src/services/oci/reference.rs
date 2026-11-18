//! Image references: `[registry[:port]/]repository[:tag][@sha256:digest]`.
//!
//! The rules follow the Docker reference grammar: the first path component
//! is a registry host when it contains a `.` or `:` (or is `localhost`),
//! otherwise the image lives on Docker Hub, where a single-component
//! repository is an official image below `library/`.

use std::fmt;

use super::{OciError, OciResult};

/// Docker Hub, as written in references and credential files.
pub const DOCKER_HUB: &str = "docker.io";
/// Host that actually serves Docker Hub's registry API.
const DOCKER_HUB_API_HOST: &str = "registry-1.docker.io";
const DEFAULT_TAG: &str = "latest";
const MAX_REFERENCE_LEN: usize = 255 + 128 + 80;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ImageReference {
    /// Canonical registry, e.g. `docker.io`, `ghcr.io`, `127.0.0.1:5000`.
    pub registry: String,
    /// Repository path, e.g. `library/alpine` or `group/project/image`.
    pub repository: String,
    pub tag: Option<String>,
    /// `sha256:<hex>`; wins over the tag when both are given.
    pub digest: Option<String>,
}

impl ImageReference {
    pub fn parse(input: &str) -> OciResult<Self> {
        let fail = |reason: &str| OciError::Reference {
            reference: input.chars().take(120).collect(),
            reason: reason.to_owned(),
        };
        let input_trimmed = input.trim();
        if input_trimmed.is_empty() {
            return Err(fail("the reference is empty"));
        }
        if input_trimmed.len() > MAX_REFERENCE_LEN {
            return Err(fail("the reference is too long"));
        }
        if input_trimmed.starts_with('-')
            || input_trimmed
                .chars()
                .any(|c| c.is_whitespace() || c.is_control())
        {
            return Err(fail(
                "the reference contains whitespace or control characters",
            ));
        }
        if input_trimmed.contains("://") {
            return Err(fail("the reference must not contain a URL scheme"));
        }

        let (name_and_tag, digest) = match input_trimmed.split_once('@') {
            Some((name, digest)) => (name, Some(digest)),
            None => (input_trimmed, None),
        };
        if let Some(digest) = digest {
            let hex = digest
                .strip_prefix("sha256:")
                .ok_or_else(|| fail("only sha256 digests are supported"))?;
            if hex.len() != 64 || !hex.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f')) {
                return Err(fail(
                    "the digest must be `sha256:` followed by 64 lowercase hex digits",
                ));
            }
        }

        // the tag is whatever follows the last `:` of the last path component
        let (path, tag) = match name_and_tag.rsplit_once('/') {
            Some((head, last)) => match last.split_once(':') {
                Some((name, tag)) => (format!("{head}/{name}"), Some(tag)),
                None => (name_and_tag.to_owned(), None),
            },
            None => match name_and_tag.split_once(':') {
                Some((name, tag)) => (name.to_owned(), Some(tag)),
                None => (name_and_tag.to_owned(), None),
            },
        };
        if let Some(tag) = tag {
            validate_tag(tag).map_err(|reason| fail(&reason))?;
        }

        let (registry, repository) = match path.split_once('/') {
            Some((first, rest)) if looks_like_registry(first) => {
                (first.to_owned(), rest.to_owned())
            }
            _ => (DOCKER_HUB.to_owned(), path.clone()),
        };
        let registry = canonical_registry(&registry);
        let repository = if registry == DOCKER_HUB && !repository.contains('/') {
            format!("library/{repository}")
        } else {
            repository
        };
        validate_registry(&registry).map_err(|reason| fail(&reason))?;
        validate_repository(&repository).map_err(|reason| fail(&reason))?;

        let tag = match (tag, digest) {
            (Some(tag), _) => Some(tag.to_owned()),
            (None, Some(_)) => None,
            (None, None) => Some(DEFAULT_TAG.to_owned()),
        };
        Ok(Self {
            registry,
            repository,
            tag,
            digest: digest.map(str::to_owned),
        })
    }

    /// Host (and port) of the registry API.
    pub fn api_host(&self) -> &str {
        if self.registry == DOCKER_HUB {
            DOCKER_HUB_API_HOST
        } else {
            &self.registry
        }
    }

    /// What to ask the registry for: the digest when pinned, else the tag.
    pub fn manifest_reference(&self) -> &str {
        self.digest
            .as_deref()
            .or(self.tag.as_deref())
            .unwrap_or(DEFAULT_TAG)
    }

    /// Hostname without the port.
    pub fn host_only(&self) -> &str {
        host_only(self.api_host())
    }
}

impl fmt::Display for ImageReference {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}/{}", self.registry, self.repository)?;
        if let Some(tag) = &self.tag {
            write!(f, ":{tag}")?;
        }
        if let Some(digest) = &self.digest {
            write!(f, "@{digest}")?;
        }
        Ok(())
    }
}

pub(crate) fn host_only(host_port: &str) -> &str {
    if let Some(rest) = host_port.strip_prefix('[') {
        // [::1]:5000
        return rest.split(']').next().unwrap_or(rest);
    }
    host_port.split(':').next().unwrap_or(host_port)
}

/// `docker.io`, `index.docker.io` and friends are one registry.
pub(crate) fn canonical_registry(registry: &str) -> String {
    let lower = registry.to_ascii_lowercase();
    match lower.as_str() {
        "docker.io" | "index.docker.io" | "registry-1.docker.io" | "registry.hub.docker.com" => {
            DOCKER_HUB.to_owned()
        }
        _ => lower,
    }
}

fn looks_like_registry(component: &str) -> bool {
    component.contains('.') || component.contains(':') || component == "localhost"
}

fn validate_registry(registry: &str) -> Result<(), String> {
    let invalid = || format!("`{registry}` is not a valid registry host");
    let port: Option<&str> = if let Some(rest) = registry.strip_prefix('[') {
        // bracketed IPv6 literal: `[::1]` or `[::1]:5000`
        let (inside, after) = rest.split_once(']').ok_or_else(invalid)?;
        if inside.is_empty()
            || !inside
                .chars()
                .all(|c| c.is_ascii_hexdigit() || c == ':' || c == '.')
        {
            return Err(invalid());
        }
        match after {
            "" => None,
            _ => Some(after.strip_prefix(':').ok_or_else(invalid)?),
        }
    } else {
        let (host, port) = match registry.split_once(':') {
            Some((host, port)) => (host, Some(port)),
            None => (registry, None),
        };
        // DNS labels: alphanumeric edges, `-` (and `_`) inside, never empty,
        // so `.`, `..` and friends are no hosts.
        let labels_ok = !host.is_empty()
            && host.split('.').all(|label| {
                let bytes = label.as_bytes();
                !bytes.is_empty()
                    && bytes[0].is_ascii_alphanumeric()
                    && bytes[bytes.len() - 1].is_ascii_alphanumeric()
                    && bytes
                        .iter()
                        .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_'))
            });
        if !labels_ok {
            return Err(invalid());
        }
        port
    };
    if let Some(port) = port
        && (port.is_empty()
            || !port.bytes().all(|b| b.is_ascii_digit())
            || port.parse::<u16>().is_err())
    {
        return Err(format!("`{port}` is not a valid registry port"));
    }
    Ok(())
}

fn validate_repository(repository: &str) -> Result<(), String> {
    if repository.is_empty() {
        return Err("the repository is empty".to_owned());
    }
    for component in repository.split('/') {
        if component.is_empty() {
            return Err("the repository has an empty path component".to_owned());
        }
        let valid_chars = component
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, '.' | '_' | '-'));
        let edge_ok = component
            .chars()
            .next()
            .is_some_and(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
            && component
                .chars()
                .last()
                .is_some_and(|c| c.is_ascii_lowercase() || c.is_ascii_digit());
        if !valid_chars || !edge_ok || component.contains("..") {
            return Err(format!(
                "`{component}` is not a valid repository component (lowercase letters, digits, `.`, `_` and `-`)"
            ));
        }
    }
    Ok(())
}

fn validate_tag(tag: &str) -> Result<(), String> {
    let mut chars = tag.chars();
    let first_ok = chars
        .next()
        .is_some_and(|c| c.is_ascii_alphanumeric() || c == '_');
    if !first_ok
        || tag.len() > 128
        || !tag
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | '-'))
    {
        return Err(format!("`{tag}` is not a valid tag"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const HEX: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

    fn parse(input: &str) -> ImageReference {
        ImageReference::parse(input).unwrap_or_else(|e| panic!("{input}: {e}"))
    }

    #[test]
    fn docker_hub_defaults() {
        let alpine = parse("alpine");
        assert_eq!(alpine.registry, "docker.io");
        assert_eq!(alpine.repository, "library/alpine");
        assert_eq!(alpine.tag.as_deref(), Some("latest"));
        assert_eq!(alpine.api_host(), "registry-1.docker.io");
        assert_eq!(alpine.to_string(), "docker.io/library/alpine:latest");

        let user = parse("rust:1.89-slim");
        assert_eq!(
            (user.repository.as_str(), user.tag.as_deref()),
            ("library/rust", Some("1.89-slim"))
        );
        let namespaced = parse("moby/buildkit:v0.13");
        assert_eq!(namespaced.repository, "moby/buildkit");
        assert_eq!(
            parse("index.docker.io/library/busybox").registry,
            "docker.io"
        );
        assert_eq!(parse("docker.io/busybox").repository, "library/busybox");
    }

    #[test]
    fn registries_with_hosts_ports_and_nested_repositories() {
        let ghcr = parse("ghcr.io/org/team/app:1.2.3");
        assert_eq!(ghcr.registry, "ghcr.io");
        assert_eq!(ghcr.repository, "org/team/app");
        assert_eq!(ghcr.tag.as_deref(), Some("1.2.3"));

        let local = parse("localhost:5000/test/app");
        assert_eq!(
            (local.registry.as_str(), local.repository.as_str()),
            ("localhost:5000", "test/app")
        );
        assert_eq!(local.tag.as_deref(), Some("latest"));
        assert_eq!(local.host_only(), "localhost");

        let ip = parse("127.0.0.1:5001/x:y");
        assert_eq!(
            (ip.registry.as_str(), ip.tag.as_deref()),
            ("127.0.0.1:5001", Some("y"))
        );

        // a leading component without dot or port is a Docker Hub namespace
        assert_eq!(parse("myorg/app").registry, "docker.io");
        assert_eq!(parse("localhost/app").registry, "localhost");
    }

    #[test]
    fn digests_win_over_tags() {
        let pinned = parse(&format!("alpine@sha256:{HEX}"));
        assert_eq!(pinned.tag, None);
        assert_eq!(pinned.manifest_reference(), format!("sha256:{HEX}"));
        let both = parse(&format!("alpine:3.20@sha256:{HEX}"));
        assert_eq!(both.tag.as_deref(), Some("3.20"));
        assert_eq!(both.manifest_reference(), format!("sha256:{HEX}"));
        assert_eq!(
            both.to_string(),
            format!("docker.io/library/alpine:3.20@sha256:{HEX}")
        );
        assert_eq!(parse("alpine").manifest_reference(), "latest");
    }

    #[test]
    fn malformed_references_are_rejected() {
        for bad in [
            "",
            "   ",
            "-v /:/host alpine",
            "alpine latest",
            "Alpine",
            "alpine:",
            "alpine:-bad",
            "alpine:ta g",
            "https://docker.io/alpine",
            "a//b",
            "alpine@sha512:abcd",
            "alpine@sha256:abcd",
            &format!("alpine@sha256:{}", HEX.to_uppercase()),
            "ghcr.io/",
            "ghcr.io:abc/app",
            "../etc/passwd",
            "./app",
            "a..b/app",
            "-host.io/app",
            "host-.io/app",
            "[::1/app",
            "[zz]:5000/app",
            "app/..",
            "app/.hidden",
            "line\nbreak",
        ] {
            assert!(
                ImageReference::parse(bad).is_err(),
                "`{bad}` must be rejected"
            );
        }
        assert!(ImageReference::parse(&"a".repeat(500)).is_err());
    }

    #[test]
    fn errors_explain_what_is_wrong() {
        let error = ImageReference::parse("Alpine").unwrap_err().to_string();
        assert!(
            error.contains("Alpine") || error.contains("lowercase"),
            "{error}"
        );
        let error = ImageReference::parse("alpine@sha256:zz")
            .unwrap_err()
            .to_string();
        assert!(error.contains("64 lowercase hex digits"), "{error}");
    }
}
