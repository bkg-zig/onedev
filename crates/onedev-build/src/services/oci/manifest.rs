//! Image manifests, indexes and image configuration documents.
//!
//! Only what is needed to run a step in an image is modelled: layers (with
//! their compression), the platform selection of multi-architecture images and
//! the runtime defaults of the image configuration (environment, working
//! directory, user).

use serde::{Deserialize, Serialize};

use super::{OciError, OciResult, digest_hex};

pub const MEDIA_OCI_MANIFEST: &str = "application/vnd.oci.image.manifest.v1+json";
pub const MEDIA_OCI_INDEX: &str = "application/vnd.oci.image.index.v1+json";
pub const MEDIA_DOCKER_MANIFEST: &str = "application/vnd.docker.distribution.manifest.v2+json";
pub const MEDIA_DOCKER_LIST: &str = "application/vnd.docker.distribution.manifest.list.v2+json";
pub const MEDIA_DOCKER_SCHEMA1: &str = "application/vnd.docker.distribution.manifest.v1+json";
pub const MEDIA_DOCKER_SCHEMA1_SIGNED: &str =
    "application/vnd.docker.distribution.manifest.v1+prettyjws";

/// `Accept` header of manifest requests.
pub const MANIFEST_ACCEPT: &str = "application/vnd.oci.image.index.v1+json, \
application/vnd.oci.image.manifest.v1+json, \
application/vnd.docker.distribution.manifest.list.v2+json, \
application/vnd.docker.distribution.manifest.v2+json";

/// Manifests and indexes are small documents; anything bigger is hostile.
pub const MAX_MANIFEST_BYTES: usize = 4 * 1024 * 1024;
/// Image configurations carry the build history and can get sizeable.
pub const MAX_CONFIG_BYTES: usize = 8 * 1024 * 1024;
/// Docker itself caps images at 127 layers; leave generous room.
pub const MAX_LAYERS: usize = 255;

/// Operating system and CPU architecture an image is built for.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Platform {
    pub os: String,
    pub architecture: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub variant: Option<String>,
}

impl Platform {
    pub fn new(os: &str, architecture: &str, variant: Option<&str>) -> Self {
        Self {
            os: os.to_owned(),
            architecture: architecture.to_owned(),
            variant: variant.map(str::to_owned),
        }
    }

    /// The Linux platform matching this machine (names as in the OCI spec).
    pub fn linux_host() -> Self {
        let (architecture, variant) = match std::env::consts::ARCH {
            "x86_64" => ("amd64", None),
            "aarch64" => ("arm64", Some("v8")),
            "arm" => ("arm", Some("v7")),
            "x86" => ("386", None),
            "riscv64" => ("riscv64", None),
            "s390x" => ("s390x", None),
            "powerpc64" if cfg!(target_endian = "little") => ("ppc64le", None),
            "powerpc64" => ("ppc64", None),
            other => (other, None),
        };
        Self::new("linux", architecture, variant)
    }
}

impl std::fmt::Display for Platform {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}/{}", self.os, self.architecture)?;
        if let Some(variant) = &self.variant {
            write!(f, "/{variant}")?;
        }
        Ok(())
    }
}

/// A content descriptor: a blob (or nested manifest) addressed by digest.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Descriptor {
    #[serde(rename = "mediaType", default)]
    pub media_type: String,
    pub digest: String,
    pub size: u64,
    #[serde(default)]
    pub platform: Option<Platform>,
}

/// How a layer blob is compressed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LayerCompression {
    None,
    Gzip,
    Zstd,
}

/// Compression of a layer, from its media type.
pub fn layer_compression(media_type: &str) -> OciResult<LayerCompression> {
    match media_type {
        "application/vnd.oci.image.layer.v1.tar"
        | "application/vnd.docker.image.rootfs.diff.tar" => Ok(LayerCompression::None),
        "application/vnd.oci.image.layer.v1.tar+gzip"
        | "application/vnd.docker.image.rootfs.diff.tar.gzip" => Ok(LayerCompression::Gzip),
        "application/vnd.oci.image.layer.v1.tar+zstd" => Ok(LayerCompression::Zstd),
        "application/vnd.docker.image.rootfs.foreign.diff.tar.gzip"
        | "application/vnd.oci.image.layer.nondistributable.v1.tar"
        | "application/vnd.oci.image.layer.nondistributable.v1.tar+gzip"
        | "application/vnd.oci.image.layer.nondistributable.v1.tar+zstd" => {
            Err(OciError::Unsupported(format!(
                "layer type `{media_type}` (foreign layers are fetched from other URLs and are not supported)"
            )))
        }
        other => Err(OciError::Unsupported(format!(
            "layer type `{other}` (this is not a container image, or it uses an unknown layer format)"
        ))),
    }
}

/// A single-platform image manifest.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImageManifest {
    pub config: Descriptor,
    pub layers: Vec<Descriptor>,
}

/// The two shapes a manifest request can answer with.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ManifestDocument {
    Image(ImageManifest),
    /// Multi-platform index (OCI image index or Docker manifest list).
    Index(Vec<Descriptor>),
}

#[derive(Deserialize)]
struct RawManifest {
    #[serde(rename = "schemaVersion", default)]
    schema_version: u32,
    #[serde(rename = "mediaType", default)]
    media_type: Option<String>,
    #[serde(default)]
    config: Option<Descriptor>,
    #[serde(default)]
    layers: Option<Vec<Descriptor>>,
    #[serde(default)]
    manifests: Option<Vec<Descriptor>>,
    #[serde(default, rename = "fsLayers")]
    fs_layers: Option<serde_json::Value>,
}

/// Parse a manifest request answer. The body decides the shape; the content
/// type is only used to produce a better message for formats we do not speak.
pub fn parse_manifest(content_type: Option<&str>, body: &[u8]) -> OciResult<ManifestDocument> {
    if body.len() > MAX_MANIFEST_BYTES {
        return Err(OciError::Limit(format!(
            "the manifest is larger than {MAX_MANIFEST_BYTES} bytes"
        )));
    }
    let content_type = content_type
        .map(|value| value.split(';').next().unwrap_or(value).trim())
        .unwrap_or("");
    if matches!(
        content_type,
        MEDIA_DOCKER_SCHEMA1 | MEDIA_DOCKER_SCHEMA1_SIGNED
    ) {
        return Err(schema1());
    }
    if !is_json_object(body) {
        return Err(OciError::Registry(
            "the registry sent an invalid manifest: not a JSON object".to_owned(),
        ));
    }
    let raw: RawManifest = serde_json::from_slice(body)
        .map_err(|e| OciError::Registry(format!("the registry sent an invalid manifest: {e}")))?;
    if raw.schema_version == 1 || raw.fs_layers.is_some() {
        return Err(schema1());
    }
    if let Some(manifests) = raw.manifests {
        validate_descriptors(&manifests)?;
        return Ok(ManifestDocument::Index(manifests));
    }
    match (raw.config, raw.layers) {
        (Some(config), Some(layers)) => {
            if layers.len() > MAX_LAYERS {
                return Err(OciError::Limit(format!(
                    "the image has {} layers (at most {MAX_LAYERS} are supported)",
                    layers.len()
                )));
            }
            validate_descriptors(&layers)?;
            validate_descriptors(std::slice::from_ref(&config))?;
            match config.media_type.as_str() {
                "application/vnd.oci.image.config.v1+json"
                | "application/vnd.docker.container.image.v1+json" => {}
                other => {
                    return Err(OciError::Unsupported(format!(
                        "artifact type `{other}` is not a container image{}",
                        raw.media_type
                            .map(|m| format!(" (manifest type {m})"))
                            .unwrap_or_default()
                    )));
                }
            }
            Ok(ManifestDocument::Image(ImageManifest { config, layers }))
        }
        _ => Err(OciError::Registry(
            "the registry sent a manifest that is neither an image nor an image index".to_owned(),
        )),
    }
}

/// serde happily reads a struct from a JSON array (`[]` -> all defaults), which
/// would turn garbage into an "empty" document; insist on an object.
fn is_json_object(body: &[u8]) -> bool {
    body.iter().find(|b| !b.is_ascii_whitespace()) == Some(&b'{')
}

fn schema1() -> OciError {
    OciError::Unsupported(
        "Docker image manifest schema 1 is obsolete; push the image again with a current Docker or buildah"
            .to_owned(),
    )
}

fn validate_descriptors(descriptors: &[Descriptor]) -> OciResult<()> {
    for descriptor in descriptors {
        digest_hex(&descriptor.digest)?;
    }
    Ok(())
}

/// Pick the entry of an image index that runs on `wanted`.
///
/// Attestation and signature manifests (`unknown/unknown`) never match. When
/// several variants exist the exact variant wins, then entries without a
/// variant, then any other.
pub fn select_platform<'a>(
    entries: &'a [Descriptor],
    wanted: &Platform,
) -> OciResult<&'a Descriptor> {
    let candidates = entries.iter().filter(|entry| {
        entry
            .platform
            .as_ref()
            .is_some_and(|p| p.os == wanted.os && p.architecture == wanted.architecture)
    });
    let mut best: Option<(&Descriptor, u8)> = None;
    for entry in candidates {
        let variant = entry.platform.as_ref().and_then(|p| p.variant.as_deref());
        let rank = match (variant, wanted.variant.as_deref()) {
            (Some(have), Some(want)) if have == want => 3,
            (None, _) => 2,
            (_, None) => 1,
            _ => 0,
        };
        if best.is_none_or(|(_, current)| rank > current) {
            best = Some((entry, rank));
        }
    }
    best.map(|(entry, _)| entry).ok_or_else(|| {
        let available: Vec<String> = entries
            .iter()
            .filter_map(|entry| entry.platform.as_ref())
            .filter(|p| p.architecture != "unknown")
            .map(ToString::to_string)
            .collect();
        OciError::Unsupported(format!(
            "the image has no variant for {wanted} (available: {})",
            if available.is_empty() {
                "none".to_owned()
            } else {
                available.join(", ")
            }
        ))
    })
}

/// Runtime defaults of an image that matter for running a step in it.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImageConfig {
    pub os: String,
    pub architecture: String,
    /// `NAME=value` entries.
    pub env: Vec<String>,
    pub working_dir: String,
    pub user: String,
    /// Number of layers the configuration declares.
    pub diff_ids: usize,
}

#[derive(Deserialize)]
struct RawImageConfig {
    #[serde(default)]
    architecture: String,
    #[serde(default)]
    os: String,
    #[serde(default)]
    config: Option<RawRuntimeConfig>,
    #[serde(default)]
    rootfs: Option<RawRootfs>,
}

#[derive(Deserialize, Default)]
struct RawRuntimeConfig {
    #[serde(rename = "Env", default)]
    env: Option<Vec<String>>,
    #[serde(rename = "WorkingDir", default)]
    working_dir: Option<String>,
    #[serde(rename = "User", default)]
    user: Option<String>,
}

#[derive(Deserialize)]
struct RawRootfs {
    #[serde(default)]
    diff_ids: Vec<String>,
}

/// Parse an image configuration blob.
pub fn parse_image_config(body: &[u8]) -> OciResult<ImageConfig> {
    if body.len() > MAX_CONFIG_BYTES {
        return Err(OciError::Limit(format!(
            "the image configuration is larger than {MAX_CONFIG_BYTES} bytes"
        )));
    }
    if !is_json_object(body) {
        return Err(OciError::Registry(
            "the image configuration is invalid: not a JSON object".to_owned(),
        ));
    }
    let raw: RawImageConfig = serde_json::from_slice(body)
        .map_err(|e| OciError::Registry(format!("the image configuration is invalid: {e}")))?;
    let runtime = raw.config.unwrap_or_default();
    Ok(ImageConfig {
        os: raw.os,
        architecture: raw.architecture,
        env: runtime
            .env
            .unwrap_or_default()
            .into_iter()
            .filter(|entry| {
                entry
                    .split_once('=')
                    .is_some_and(|(name, _)| !name.is_empty())
            })
            .collect(),
        working_dir: runtime.working_dir.unwrap_or_default(),
        user: runtime.user.unwrap_or_default(),
        diff_ids: raw.rootfs.map(|r| r.diff_ids.len()).unwrap_or(0),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const D1: &str = "sha256:1111111111111111111111111111111111111111111111111111111111111111";
    const D2: &str = "sha256:2222222222222222222222222222222222222222222222222222222222222222";
    const D3: &str = "sha256:3333333333333333333333333333333333333333333333333333333333333333";

    fn entry(os: &str, arch: &str, variant: Option<&str>, digest: &str) -> Descriptor {
        Descriptor {
            media_type: MEDIA_OCI_MANIFEST.to_owned(),
            digest: digest.to_owned(),
            size: 100,
            platform: Some(Platform::new(os, arch, variant)),
        }
    }

    #[test]
    fn layer_media_types_map_to_compressions() {
        assert_eq!(
            layer_compression("application/vnd.oci.image.layer.v1.tar+gzip").unwrap(),
            LayerCompression::Gzip
        );
        assert_eq!(
            layer_compression("application/vnd.docker.image.rootfs.diff.tar.gzip").unwrap(),
            LayerCompression::Gzip
        );
        assert_eq!(
            layer_compression("application/vnd.oci.image.layer.v1.tar+zstd").unwrap(),
            LayerCompression::Zstd
        );
        assert_eq!(
            layer_compression("application/vnd.oci.image.layer.v1.tar").unwrap(),
            LayerCompression::None
        );
        let foreign =
            layer_compression("application/vnd.docker.image.rootfs.foreign.diff.tar.gzip")
                .unwrap_err()
                .to_string();
        assert!(foreign.contains("foreign"), "{foreign}");
        assert!(layer_compression("application/x-helm").is_err());
    }

    #[test]
    fn image_manifests_are_parsed() {
        let body = format!(
            r#"{{"schemaVersion":2,"mediaType":"{MEDIA_DOCKER_MANIFEST}",
            "config":{{"mediaType":"application/vnd.docker.container.image.v1+json","digest":"{D1}","size":12}},
            "layers":[{{"mediaType":"application/vnd.docker.image.rootfs.diff.tar.gzip","digest":"{D2}","size":34}}]}}"#
        );
        let ManifestDocument::Image(manifest) =
            parse_manifest(Some(MEDIA_DOCKER_MANIFEST), body.as_bytes()).unwrap()
        else {
            panic!("expected an image manifest");
        };
        assert_eq!(manifest.config.digest, D1);
        assert_eq!(manifest.layers.len(), 1);
        assert_eq!(manifest.layers[0].size, 34);
    }

    #[test]
    fn indexes_are_parsed_and_the_platform_is_selected() {
        let body = format!(
            r#"{{"schemaVersion":2,"mediaType":"{MEDIA_OCI_INDEX}","manifests":[
            {{"mediaType":"{MEDIA_OCI_MANIFEST}","digest":"{D1}","size":1,"platform":{{"architecture":"amd64","os":"linux"}}}},
            {{"mediaType":"{MEDIA_OCI_MANIFEST}","digest":"{D2}","size":1,"platform":{{"architecture":"arm64","os":"linux","variant":"v8"}}}},
            {{"mediaType":"{MEDIA_OCI_MANIFEST}","digest":"{D3}","size":1,"platform":{{"architecture":"unknown","os":"unknown"}}}}]}}"#
        );
        let ManifestDocument::Index(entries) = parse_manifest(None, body.as_bytes()).unwrap()
        else {
            panic!("expected an index");
        };
        assert_eq!(entries.len(), 3);
        let amd64 = select_platform(&entries, &Platform::new("linux", "amd64", None)).unwrap();
        assert_eq!(amd64.digest, D1);
        let arm64 =
            select_platform(&entries, &Platform::new("linux", "arm64", Some("v8"))).unwrap();
        assert_eq!(arm64.digest, D2);
        let error = select_platform(&entries, &Platform::new("linux", "s390x", None))
            .unwrap_err()
            .to_string();
        assert!(
            error.contains("linux/s390x") && error.contains("linux/amd64"),
            "{error}"
        );
        assert!(
            !error.contains("unknown"),
            "attestation entries are not offered: {error}"
        );
    }

    #[test]
    fn variants_prefer_the_exact_match_then_none_then_anything() {
        let entries = vec![
            entry("linux", "arm", Some("v5"), D1),
            entry("linux", "arm", None, D2),
            entry("linux", "arm", Some("v7"), D3),
        ];
        assert_eq!(
            select_platform(&entries, &Platform::new("linux", "arm", Some("v7")))
                .unwrap()
                .digest,
            D3
        );
        assert_eq!(
            select_platform(&entries, &Platform::new("linux", "arm", Some("v6")))
                .unwrap()
                .digest,
            D2
        );
        let only_variants = vec![entry("linux", "arm", Some("v5"), D1)];
        assert_eq!(
            select_platform(&only_variants, &Platform::new("linux", "arm", Some("v7")))
                .unwrap()
                .digest,
            D1
        );
    }

    #[test]
    fn unsupported_and_malformed_manifests_are_rejected_with_reasons() {
        let schema1 = br#"{"schemaVersion":1,"fsLayers":[],"name":"x"}"#;
        assert!(
            parse_manifest(None, schema1)
                .unwrap_err()
                .to_string()
                .contains("schema 1")
        );
        assert!(
            parse_manifest(Some(MEDIA_DOCKER_SCHEMA1_SIGNED), b"{}")
                .unwrap_err()
                .to_string()
                .contains("schema 1")
        );
        assert!(parse_manifest(None, b"not json").is_err());
        assert!(parse_manifest(None, b"{}").is_err());
        let helm = format!(
            r#"{{"schemaVersion":2,"config":{{"mediaType":"application/vnd.cncf.helm.config.v1+json","digest":"{D1}","size":1}},"layers":[]}}"#
        );
        let error = parse_manifest(None, helm.as_bytes())
            .unwrap_err()
            .to_string();
        assert!(error.contains("not a container image"), "{error}");
        let bad_digest = r#"{"schemaVersion":2,"config":{"mediaType":"application/vnd.oci.image.config.v1+json","digest":"sha256:../x","size":1},"layers":[]}"#;
        assert!(parse_manifest(None, bad_digest.as_bytes()).is_err());
        assert!(parse_manifest(None, &vec![b' '; MAX_MANIFEST_BYTES + 1]).is_err());
    }

    #[test]
    fn image_configs_expose_env_workdir_and_user() {
        let body = br#"{"architecture":"amd64","os":"linux",
            "config":{"Env":["PATH=/usr/bin","BROKEN","=x","JAVA_HOME=/opt/jdk"],"WorkingDir":"/app","User":"1000:1000","Cmd":["sh"]},
            "rootfs":{"type":"layers","diff_ids":["sha256:a","sha256:b"]}}"#;
        let config = parse_image_config(body).unwrap();
        assert_eq!(config.os, "linux");
        assert_eq!(config.architecture, "amd64");
        assert_eq!(config.env, vec!["PATH=/usr/bin", "JAVA_HOME=/opt/jdk"]);
        assert_eq!(config.working_dir, "/app");
        assert_eq!(config.user, "1000:1000");
        assert_eq!(config.diff_ids, 2);

        let minimal =
            parse_image_config(br#"{"architecture":"arm64","os":"linux","config":null}"#).unwrap();
        assert!(
            minimal.env.is_empty() && minimal.working_dir.is_empty() && minimal.user.is_empty()
        );
        assert!(parse_image_config(b"[]").is_err());
    }

    #[test]
    fn the_host_platform_is_linux() {
        let host = Platform::linux_host();
        assert_eq!(host.os, "linux");
        assert!(!host.architecture.is_empty());
        assert!(host.to_string().starts_with("linux/"));
    }
}
