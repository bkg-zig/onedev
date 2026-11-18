//! A native OCI image toolkit: everything needed to turn an image reference
//! into a root filesystem on disk, without Docker.
//!
//! * [`reference`] parses `registry/repository:tag@digest` references,
//! * [`registry`] speaks the OCI distribution / Docker registry v2 protocol
//!   (token and basic authentication, manifest lists, digest verification),
//! * [`unpack`] applies image layers to a directory (whiteouts, hard links,
//!   path traversal and symlink escape protection, size limits),
//! * [`store`] keeps pulled images in a shared on-disk cache with
//!   cross-process locking, tag freshness and pruning.

pub mod manifest;
pub mod reference;
pub mod registry;
pub mod store;
pub mod unpack;

#[cfg(test)]
pub(crate) mod testing;

pub use manifest::{ImageConfig, Platform};
pub use reference::ImageReference;
pub use registry::{CredentialStore, Credentials, RegistryClient, RegistryOptions};
pub use store::{
    ImageLease, ImageStore, PruneOptions, PruneReport, PullEvent, PulledImage, StoreOptions,
};

/// Why an image operation failed.
#[derive(Debug, thiserror::Error)]
pub enum OciError {
    #[error("invalid image reference `{reference}`: {reason}")]
    Reference { reference: String, reason: String },
    /// The registry answered, but not with what we asked for.
    #[error("{0}")]
    Registry(String),
    /// The registry could not be reached or is temporarily failing (network
    /// error, timeout, 5xx, rate limit). Retrying later may help.
    #[error("registry unavailable: {0}")]
    Unavailable(String),
    #[error("authentication failed: {0}")]
    Auth(String),
    #[error("digest mismatch for {what}: expected {expected}, got {actual}")]
    Digest {
        what: String,
        expected: String,
        actual: String,
    },
    #[error("unsupported image: {0}")]
    Unsupported(String),
    #[error("invalid image layer: {0}")]
    Layer(String),
    #[error("limit exceeded: {0}")]
    Limit(String),
    #[error("interrupted")]
    Interrupted,
    #[error("{context}: {source}")]
    Io {
        context: String,
        #[source]
        source: std::io::Error,
    },
}

impl OciError {
    pub(crate) fn io(context: impl Into<String>, source: std::io::Error) -> Self {
        OciError::Io {
            context: context.into(),
            source,
        }
    }
}

pub type OciResult<T> = Result<T, OciError>;

/// `sha256:<hex>` of `bytes`.
pub fn sha256_digest(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    format!("sha256:{}", hex::encode(Sha256::digest(bytes)))
}

/// The hex part of a `sha256:<hex>` digest, validated so it is safe to use
/// as a file name.
pub fn digest_hex(digest: &str) -> OciResult<&str> {
    let hex = digest.strip_prefix("sha256:").ok_or_else(|| {
        OciError::Unsupported(format!("digest `{digest}` (only sha256 is supported)"))
    })?;
    if hex.len() == 64 && hex.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f')) {
        Ok(hex)
    } else {
        Err(OciError::Registry(format!("malformed digest `{digest}`")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn digests_are_computed_and_validated() {
        let digest = sha256_digest(b"hello");
        assert_eq!(
            digest,
            "sha256:2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824"
        );
        assert_eq!(digest_hex(&digest).unwrap().len(), 64);
        assert!(digest_hex("sha512:abcd").is_err());
        assert!(digest_hex("sha256:../../etc/passwd").is_err());
        assert!(
            digest_hex("sha256:ABCDEF0123456789ABCDEF0123456789ABCDEF0123456789ABCDEF0123456789")
                .is_err()
        );
    }
}
