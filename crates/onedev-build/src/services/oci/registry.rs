//! Client for the OCI distribution API (Docker registry HTTP API v2).
//!
//! * Bearer (token) and Basic authentication, following the
//!   `WWW-Authenticate` challenge of the registry; tokens are cached per
//!   registry and repository until shortly before they expire,
//! * credentials come from a Docker `config.json` (inline `auth`,
//!   `username`/`password`, `credHelpers` and `credsStore` helper programs),
//! * HTTPS unless the registry is listed as insecure; a redirect to plain HTTP
//!   is never followed otherwise, and credentials are never sent over HTTP to
//!   token endpoints of secure registries,
//! * everything that is downloaded is verified against its digest (and the
//!   declared size for blobs) while it streams to disk,
//! * transient failures (network errors, timeouts, 5xx, 429) are retried with
//!   backoff.

use std::collections::HashMap;
use std::error::Error as _;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use base64::Engine as _;
use reqwest::header::{ACCEPT, ACCEPT_ENCODING, AUTHORIZATION, HeaderValue};
use reqwest::{Client, Method, Response, StatusCode, Url};
use serde::Deserialize;
use sha2::{Digest as _, Sha256};
use tokio::io::AsyncWriteExt as _;

use super::manifest::{
    Descriptor, ImageConfig, ImageManifest, MANIFEST_ACCEPT, MAX_CONFIG_BYTES, MAX_MANIFEST_BYTES,
    ManifestDocument, Platform, parse_image_config, parse_manifest, select_platform,
};
use super::reference::{DOCKER_HUB, ImageReference, canonical_registry, host_only};
use super::{OciError, OciResult, digest_hex, sha256_digest};

const MAX_CREDENTIAL_FILE_BYTES: u64 = 1024 * 1024;
const MAX_TOKEN_RESPONSE_BYTES: usize = 64 * 1024;
const MAX_ERROR_BODY_BYTES: usize = 16 * 1024;
const MAX_RETRY_AFTER: Duration = Duration::from_secs(10);
const CREDENTIAL_HELPER_TIMEOUT: Duration = Duration::from_secs(15);

// ---------------------------------------------------------------------------
// Credentials
// ---------------------------------------------------------------------------

/// User name and password (or access token) for a registry.
#[derive(Clone, PartialEq, Eq)]
pub struct Credentials {
    pub username: String,
    pub password: String,
}

impl Credentials {
    pub fn new(username: impl Into<String>, password: impl Into<String>) -> Self {
        Self {
            username: username.into(),
            password: password.into(),
        }
    }

    fn basic_header(&self) -> String {
        let raw = format!("{}:{}", self.username, self.password);
        format!(
            "Basic {}",
            base64::engine::general_purpose::STANDARD.encode(raw)
        )
    }
}

impl std::fmt::Debug for Credentials {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Credentials")
            .field("username", &self.username)
            .field("password", &"<redacted>")
            .finish()
    }
}

/// Where registry credentials come from: explicitly added entries and
/// (optionally) a Docker `config.json`, which is re-read on every lookup so
/// rotated credentials are picked up without a restart.
#[derive(Debug, Clone, Default)]
pub struct CredentialStore {
    file: Option<PathBuf>,
    explicit: HashMap<String, Credentials>,
}

#[derive(Deserialize, Default)]
struct DockerConfigFile {
    #[serde(default)]
    auths: HashMap<String, DockerAuthEntry>,
    #[serde(default, rename = "credsStore")]
    creds_store: Option<String>,
    #[serde(default, rename = "credHelpers")]
    cred_helpers: HashMap<String, String>,
}

#[derive(Deserialize, Default)]
struct DockerAuthEntry {
    #[serde(default)]
    auth: Option<String>,
    #[serde(default)]
    username: Option<String>,
    #[serde(default)]
    password: Option<String>,
    #[serde(default)]
    identitytoken: Option<String>,
    #[serde(default)]
    registrytoken: Option<String>,
}

#[derive(Deserialize)]
struct HelperOutput {
    #[serde(rename = "Username", default)]
    username: String,
    #[serde(rename = "Secret", default)]
    secret: String,
}

impl CredentialStore {
    pub fn new() -> Self {
        Self::default()
    }

    /// Use the credentials of a Docker `config.json`.
    pub fn with_file(mut self, path: impl Into<PathBuf>) -> Self {
        self.file = Some(path.into());
        self
    }

    /// Add credentials for one registry (`ghcr.io`, `registry.example.com:5000`).
    pub fn with_credentials(mut self, registry: &str, credentials: Credentials) -> Self {
        self.explicit
            .insert(normalize_registry_key(registry), credentials);
        self
    }

    /// Credentials to use for `registry` (canonical name, e.g. `docker.io`).
    pub async fn lookup(&self, registry: &str) -> OciResult<Option<Credentials>> {
        let key = normalize_registry_key(registry);
        if let Some(credentials) = self.explicit.get(&key) {
            return Ok(Some(credentials.clone()));
        }
        let Some(path) = &self.file else {
            return Ok(None);
        };
        let config = match read_docker_config(path)? {
            Some(config) => config,
            None => return Ok(None),
        };
        let normalized_helpers: HashMap<String, &String> = config
            .cred_helpers
            .iter()
            .map(|(host, helper)| (normalize_registry_key(host), helper))
            .collect();
        let helper = normalized_helpers
            .get(&key)
            .map(|helper| helper.as_str())
            .or(config.creds_store.as_deref())
            .filter(|helper| !helper.is_empty());
        if let Some(helper) = helper {
            let server = if key == DOCKER_HUB {
                "https://index.docker.io/v1/".to_owned()
            } else {
                key.clone()
            };
            if let Some(credentials) = run_credential_helper(helper, &server).await? {
                return Ok(Some(credentials));
            }
        }
        let entry = config
            .auths
            .iter()
            .find(|(host, _)| normalize_registry_key(host) == key)
            .map(|(_, entry)| entry);
        let Some(entry) = entry else {
            return Ok(None);
        };
        if let Some(auth) = entry.auth.as_deref().filter(|a| !a.is_empty()) {
            let decoded = base64::engine::general_purpose::STANDARD
                .decode(auth.trim())
                .ok()
                .and_then(|bytes| String::from_utf8(bytes).ok())
                .ok_or_else(|| {
                    OciError::Auth(format!(
                        "the `auth` entry for {key} in {} is not valid base64 of `user:password`",
                        path.display()
                    ))
                })?;
            let (username, password) = decoded.split_once(':').ok_or_else(|| {
                OciError::Auth(format!(
                    "the `auth` entry for {key} in {} must decode to `user:password`",
                    path.display()
                ))
            })?;
            return Ok(Some(Credentials::new(username, password)));
        }
        if let (Some(username), Some(password)) = (&entry.username, &entry.password)
            && !username.is_empty()
        {
            return Ok(Some(Credentials::new(username.clone(), password.clone())));
        }
        if entry.identitytoken.is_some() || entry.registrytoken.is_some() {
            return Err(unsupported_token_credentials(&key));
        }
        Ok(None)
    }
}

fn unsupported_token_credentials(registry: &str) -> OciError {
    OciError::Auth(format!(
        "the credentials for {registry} are an identity token (OAuth refresh token), which is not supported; use a user name and password or an access token"
    ))
}

fn read_docker_config(path: &Path) -> OciResult<Option<DockerConfigFile>> {
    let metadata = match std::fs::metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            return Err(OciError::io(
                format!("cannot read the registry credentials {}", path.display()),
                error,
            ));
        }
    };
    if metadata.len() > MAX_CREDENTIAL_FILE_BYTES {
        return Err(OciError::Auth(format!(
            "{} is unreasonably large for a credentials file",
            path.display()
        )));
    }
    let text = std::fs::read_to_string(path).map_err(|error| {
        OciError::io(
            format!("cannot read the registry credentials {}", path.display()),
            error,
        )
    })?;
    serde_json::from_str(&text).map(Some).map_err(|error| {
        OciError::Auth(format!(
            "{} is not a valid Docker config.json: {error}",
            path.display()
        ))
    })
}

/// `https://Index.Docker.io/v1/` -> `docker.io`, `ghcr.io/` -> `ghcr.io`.
fn normalize_registry_key(key: &str) -> String {
    let without_scheme = key
        .strip_prefix("https://")
        .or_else(|| key.strip_prefix("http://"))
        .unwrap_or(key);
    let host = without_scheme.split('/').next().unwrap_or(without_scheme);
    canonical_registry(host)
}

async fn run_credential_helper(name: &str, server: &str) -> OciResult<Option<Credentials>> {
    if name.is_empty()
        || !name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
    {
        return Err(OciError::Auth(format!(
            "`{name}` is not a valid credential helper name"
        )));
    }
    let program = format!("docker-credential-{name}");
    let mut child = tokio::process::Command::new(&program)
        .arg("get")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .kill_on_drop(true)
        .spawn()
        .map_err(|error| {
            OciError::Auth(format!(
                "cannot run the credential helper {program}: {error}"
            ))
        })?;
    if let Some(mut stdin) = child.stdin.take() {
        let _ = stdin.write_all(server.as_bytes()).await;
    }
    let output = tokio::time::timeout(CREDENTIAL_HELPER_TIMEOUT, child.wait_with_output())
        .await
        .map_err(|_| {
            OciError::Auth(format!(
                "the credential helper {program} did not answer in time"
            ))
        })?
        .map_err(|error| {
            OciError::Auth(format!("the credential helper {program} failed: {error}"))
        })?;
    if !output.status.success() {
        let message = String::from_utf8_lossy(&output.stdout);
        if message.to_ascii_lowercase().contains("not found") {
            return Ok(None);
        }
        return Err(OciError::Auth(format!(
            "the credential helper {program} failed: {}",
            sanitize(&message, 200)
        )));
    }
    let parsed: HelperOutput = serde_json::from_slice(&output.stdout).map_err(|error| {
        OciError::Auth(format!(
            "the credential helper {program} sent unusable output: {error}"
        ))
    })?;
    if parsed.username == "<token>" {
        return Err(unsupported_token_credentials(server));
    }
    if parsed.username.is_empty() {
        return Ok(None);
    }
    Ok(Some(Credentials::new(parsed.username, parsed.secret)))
}

// ---------------------------------------------------------------------------
// WWW-Authenticate
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
enum Challenge {
    Bearer {
        realm: String,
        service: Option<String>,
        scope: Option<String>,
    },
    Basic,
    Other(String),
}

/// Parse the value(s) of `WWW-Authenticate` headers into challenges.
fn parse_challenges<'a>(values: impl IntoIterator<Item = &'a str>) -> Vec<Challenge> {
    let mut parsed: Vec<(String, HashMap<String, String>)> = Vec::new();
    for value in values {
        let bytes = value.as_bytes();
        let mut i = 0;
        while i < bytes.len() {
            while i < bytes.len() && (bytes[i].is_ascii_whitespace() || bytes[i] == b',') {
                i += 1;
            }
            let start = i;
            while i < bytes.len()
                && !bytes[i].is_ascii_whitespace()
                && bytes[i] != b','
                && bytes[i] != b'='
            {
                i += 1;
            }
            if start == i {
                // a stray `=`: skip it to guarantee progress
                i += 1;
                continue;
            }
            let token = &value[start..i];
            let mut j = i;
            while j < bytes.len() && bytes[j].is_ascii_whitespace() {
                j += 1;
            }
            if j < bytes.len() && bytes[j] == b'=' {
                // key=value of the current challenge
                i = j + 1;
                while i < bytes.len() && bytes[i].is_ascii_whitespace() {
                    i += 1;
                }
                let mut text = String::new();
                if i < bytes.len() && bytes[i] == b'"' {
                    i += 1;
                    while i < bytes.len() && bytes[i] != b'"' {
                        if bytes[i] == b'\\' && i + 1 < bytes.len() {
                            i += 1;
                        }
                        // push whole UTF-8 characters
                        let ch_start = i;
                        i += 1;
                        while i < bytes.len() && !value.is_char_boundary(i) {
                            i += 1;
                        }
                        text.push_str(&value[ch_start..i]);
                    }
                    i += 1; // closing quote
                } else {
                    let v_start = i;
                    while i < bytes.len() && bytes[i] != b',' && !bytes[i].is_ascii_whitespace() {
                        i += 1;
                    }
                    text.push_str(&value[v_start..i]);
                }
                if let Some((_, params)) = parsed.last_mut() {
                    params.insert(token.to_ascii_lowercase(), text);
                }
            } else {
                parsed.push((token.to_ascii_lowercase(), HashMap::new()));
                i = j.max(i);
            }
        }
    }
    parsed
        .into_iter()
        .map(|(scheme, mut params)| match scheme.as_str() {
            "bearer" => match params.remove("realm") {
                Some(realm) if !realm.is_empty() => Challenge::Bearer {
                    realm,
                    service: params.remove("service").filter(|s| !s.is_empty()),
                    scope: params.remove("scope").filter(|s| !s.is_empty()),
                },
                _ => Challenge::Other("bearer without realm".to_owned()),
            },
            "basic" => Challenge::Basic,
            other => Challenge::Other(other.to_owned()),
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Client
// ---------------------------------------------------------------------------

/// Settings of a [`RegistryClient`].
#[derive(Debug, Clone)]
pub struct RegistryOptions {
    pub credentials: CredentialStore,
    /// Registries (`host` or `host:port`) that are contacted over plain HTTP.
    pub insecure_registries: Vec<String>,
    /// Extra trusted root certificates (PEM bundles), e.g. an internal CA.
    pub ca_certificates_pem: Vec<Vec<u8>>,
    pub connect_timeout: Duration,
    /// A transfer that yields no data for this long is aborted.
    pub read_timeout: Duration,
    /// Attempts per request for transient failures.
    pub max_attempts: u32,
    /// First backoff delay (doubles per attempt).
    pub retry_base: Duration,
}

impl Default for RegistryOptions {
    fn default() -> Self {
        Self {
            credentials: CredentialStore::default(),
            insecure_registries: Vec::new(),
            ca_certificates_pem: Vec::new(),
            connect_timeout: Duration::from_secs(15),
            read_timeout: Duration::from_secs(60),
            max_attempts: 3,
            retry_base: Duration::from_millis(500),
        }
    }
}

/// A manifest as the registry delivered it.
#[derive(Debug, Clone)]
pub struct FetchedManifest {
    /// Digest of the received bytes.
    pub digest: String,
    pub media_type: Option<String>,
    pub body: Vec<u8>,
}

/// An image resolved for one platform.
#[derive(Debug, Clone)]
pub struct ResolvedImage {
    /// Digest of the multi-platform index the reference pointed at, if any.
    pub index_digest: Option<String>,
    /// Digest of the single-platform manifest (identifies the image content).
    pub manifest_digest: String,
    pub manifest: ImageManifest,
}

struct CachedAuth {
    header: String,
    expires: Instant,
}

struct Request<'a> {
    method: Method,
    path: String,
    accept: Option<&'a str>,
    identity_encoding: bool,
}

pub struct RegistryClient {
    http: Client,
    credentials: CredentialStore,
    insecure: Vec<String>,
    tokens: Mutex<HashMap<(String, String), CachedAuth>>,
    max_attempts: u32,
    retry_base: Duration,
}

impl RegistryClient {
    pub fn new(options: RegistryOptions) -> OciResult<Self> {
        let insecure: Vec<String> = options
            .insecure_registries
            .iter()
            .map(|entry| entry.trim().to_ascii_lowercase())
            .filter(|entry| !entry.is_empty())
            .collect();
        let redirect_insecure = insecure.clone();
        let policy = reqwest::redirect::Policy::custom(move |attempt| {
            if attempt.previous().len() >= 5 {
                return attempt.error("too many redirects");
            }
            let url = attempt.url();
            if url.scheme() == "http" && !host_is_insecure(&redirect_insecure, &url_host_port(url))
            {
                return attempt.error("refusing to follow a redirect to plain HTTP");
            }
            attempt.follow()
        });
        let mut builder = Client::builder()
            .user_agent(concat!("onedev-build/", env!("CARGO_PKG_VERSION")))
            .connect_timeout(options.connect_timeout)
            .read_timeout(options.read_timeout)
            .no_gzip()
            .no_brotli()
            .redirect(policy);
        for pem in &options.ca_certificates_pem {
            let certificates = reqwest::Certificate::from_pem_bundle(pem).map_err(|error| {
                OciError::Auth(format!(
                    "the configured CA certificate bundle is invalid: {error}"
                ))
            })?;
            for certificate in certificates {
                builder = builder.add_root_certificate(certificate);
            }
        }
        let http = builder.build().map_err(|error| {
            OciError::Registry(format!("cannot create the HTTP client: {error}"))
        })?;
        Ok(Self {
            http,
            credentials: options.credentials,
            insecure,
            tokens: Mutex::new(HashMap::new()),
            max_attempts: options.max_attempts.max(1),
            retry_base: options.retry_base,
        })
    }

    // -- public operations ----------------------------------------------------

    /// Digest the registry currently maps `reference` to, without downloading
    /// the manifest (`HEAD`). `None` when the registry does not announce it.
    pub async fn head_digest(&self, reference: &ImageReference) -> OciResult<Option<String>> {
        let request = Request {
            method: Method::HEAD,
            path: format!(
                "/v2/{}/manifests/{}",
                reference.repository,
                reference.manifest_reference()
            ),
            accept: Some(MANIFEST_ACCEPT),
            identity_encoding: false,
        };
        let response = self
            .execute(reference, &request, "the image manifest")
            .await?;
        let digest = response
            .headers()
            .get("docker-content-digest")
            .and_then(|value| value.to_str().ok())
            .map(str::to_owned)
            .filter(|digest| digest_hex(digest).is_ok());
        Ok(digest)
    }

    /// Download a manifest by tag or digest and verify it against its digest.
    pub async fn fetch_manifest(
        &self,
        reference: &ImageReference,
        manifest_ref: &str,
    ) -> OciResult<FetchedManifest> {
        let request = Request {
            method: Method::GET,
            path: format!("/v2/{}/manifests/{manifest_ref}", reference.repository),
            accept: Some(MANIFEST_ACCEPT),
            identity_encoding: false,
        };
        let response = self
            .execute(reference, &request, "the image manifest")
            .await?;
        let media_type = response
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .map(|value| value.split(';').next().unwrap_or(value).trim().to_owned());
        let body = self
            .read_limited(reference, response, MAX_MANIFEST_BYTES)
            .await?;
        let digest = sha256_digest(&body);
        if manifest_ref.starts_with("sha256:") && manifest_ref != digest {
            return Err(OciError::Digest {
                what: format!("manifest {reference}"),
                expected: manifest_ref.to_owned(),
                actual: digest,
            });
        }
        Ok(FetchedManifest {
            digest,
            media_type,
            body,
        })
    }

    /// Resolve `reference` to the single-platform manifest for `platform`.
    pub async fn resolve(
        &self,
        reference: &ImageReference,
        platform: &Platform,
    ) -> OciResult<ResolvedImage> {
        let top = self
            .fetch_manifest(reference, reference.manifest_reference())
            .await?;
        match parse_manifest(top.media_type.as_deref(), &top.body)? {
            ManifestDocument::Image(manifest) => Ok(ResolvedImage {
                index_digest: None,
                manifest_digest: top.digest,
                manifest,
            }),
            ManifestDocument::Index(entries) => {
                let chosen = select_platform(&entries, platform)?;
                let nested = self.fetch_manifest(reference, &chosen.digest).await?;
                match parse_manifest(nested.media_type.as_deref(), &nested.body)? {
                    ManifestDocument::Image(manifest) => Ok(ResolvedImage {
                        index_digest: Some(top.digest),
                        manifest_digest: nested.digest,
                        manifest,
                    }),
                    ManifestDocument::Index(_) => Err(OciError::Unsupported(
                        "nested image indexes are not supported".to_owned(),
                    )),
                }
            }
        }
    }

    /// Download and parse the image configuration of `manifest`.
    pub async fn fetch_config(
        &self,
        reference: &ImageReference,
        manifest: &ImageManifest,
    ) -> OciResult<ImageConfig> {
        let descriptor = &manifest.config;
        if descriptor.size > MAX_CONFIG_BYTES as u64 {
            return Err(OciError::Limit(format!(
                "the image configuration claims {} bytes",
                descriptor.size
            )));
        }
        digest_hex(&descriptor.digest)?;
        let request = Request {
            method: Method::GET,
            path: format!("/v2/{}/blobs/{}", reference.repository, descriptor.digest),
            accept: None,
            identity_encoding: true,
        };
        let response = self
            .execute(reference, &request, "the image configuration")
            .await?;
        let body = self
            .read_limited(reference, response, MAX_CONFIG_BYTES)
            .await?;
        let actual = sha256_digest(&body);
        if actual != descriptor.digest {
            return Err(OciError::Digest {
                what: "the image configuration".to_owned(),
                expected: descriptor.digest.clone(),
                actual,
            });
        }
        parse_image_config(&body)
    }

    /// Download a blob to `dest`, verifying size and digest while it streams.
    /// `progress` receives the number of bytes of each received chunk. A
    /// failed or interrupted transfer leaves nothing behind at `dest`.
    pub async fn download_blob(
        &self,
        reference: &ImageReference,
        blob: &Descriptor,
        dest: &Path,
        progress: &(dyn Fn(u64) + Send + Sync),
    ) -> OciResult<()> {
        digest_hex(&blob.digest)?;
        let mut attempt = 0;
        loop {
            attempt += 1;
            match self
                .download_blob_once(reference, blob, dest, progress)
                .await
            {
                Ok(()) => return Ok(()),
                Err(OciError::Unavailable(message)) if attempt < self.max_attempts => {
                    tracing::debug!(blob = %blob.digest, attempt, %message, "layer download failed, retrying");
                    self.backoff(attempt, None).await;
                }
                Err(error) => return Err(error),
            }
        }
    }

    async fn download_blob_once(
        &self,
        reference: &ImageReference,
        blob: &Descriptor,
        dest: &Path,
        progress: &(dyn Fn(u64) + Send + Sync),
    ) -> OciResult<()> {
        let request = Request {
            method: Method::GET,
            path: format!("/v2/{}/blobs/{}", reference.repository, blob.digest),
            accept: None,
            identity_encoding: true,
        };
        let mut response = self.execute(reference, &request, "a layer").await?;
        let io = |context: &str, error: std::io::Error| {
            OciError::io(format!("{context} {}", dest.display()), error)
        };
        let file = tokio::fs::File::create(dest)
            .await
            .map_err(|e| io("cannot create", e))?;
        let mut writer = tokio::io::BufWriter::with_capacity(256 * 1024, file);
        let mut hasher = Sha256::new();
        let mut received: u64 = 0;
        let result: OciResult<()> = async {
            while let Some(chunk) = response
                .chunk()
                .await
                .map_err(|e| transport_error(reference.api_host(), &e))?
            {
                received += chunk.len() as u64;
                if received > blob.size {
                    return Err(OciError::Registry(format!(
                        "the layer {} is larger than the {} bytes its manifest declares",
                        blob.digest, blob.size
                    )));
                }
                hasher.update(&chunk);
                writer
                    .write_all(&chunk)
                    .await
                    .map_err(|e| io("cannot write", e))?;
                progress(chunk.len() as u64);
            }
            writer.flush().await.map_err(|e| io("cannot write", e))?;
            if received != blob.size {
                return Err(OciError::Unavailable(format!(
                    "the layer {} ended after {received} of {} bytes",
                    blob.digest, blob.size
                )));
            }
            let actual = format!("sha256:{}", hex::encode(hasher.finalize_reset()));
            if actual != blob.digest {
                return Err(OciError::Digest {
                    what: "a layer".to_owned(),
                    expected: blob.digest.clone(),
                    actual,
                });
            }
            Ok(())
        }
        .await;
        drop(writer);
        if result.is_err() {
            let _ = tokio::fs::remove_file(dest).await;
        }
        result
    }

    // -- plumbing ---------------------------------------------------------------

    fn scheme(&self, host_port: &str) -> &'static str {
        if host_is_insecure(&self.insecure, host_port) {
            "http"
        } else {
            "https"
        }
    }

    fn cached_auth(&self, key: &(String, String)) -> Option<String> {
        let mut tokens = self.tokens.lock().unwrap_or_else(|p| p.into_inner());
        match tokens.get(key) {
            Some(cached) if cached.expires > Instant::now() => Some(cached.header.clone()),
            Some(_) => {
                tokens.remove(key);
                None
            }
            None => None,
        }
    }

    fn store_auth(&self, key: (String, String), header: String, ttl: Duration) {
        self.tokens
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .insert(
                key,
                CachedAuth {
                    header,
                    expires: Instant::now() + ttl,
                },
            );
    }

    async fn backoff(&self, attempt: u32, retry_after: Option<Duration>) {
        let exponential = self.retry_base.saturating_mul(1 << (attempt - 1).min(6));
        let wait = retry_after
            .map(|server| server.min(MAX_RETRY_AFTER))
            .unwrap_or(exponential);
        tokio::time::sleep(wait).await;
    }

    async fn send(
        &self,
        request: &Request<'_>,
        url: &Url,
        authorization: Option<&str>,
    ) -> reqwest::Result<Response> {
        let mut builder = self.http.request(request.method.clone(), url.clone());
        if let Some(accept) = request.accept {
            builder = builder.header(ACCEPT, accept);
        }
        if request.identity_encoding {
            builder = builder.header(ACCEPT_ENCODING, "identity");
        }
        if let Some(authorization) = authorization
            && let Ok(mut value) = HeaderValue::from_str(authorization)
        {
            value.set_sensitive(true);
            builder = builder.header(AUTHORIZATION, value);
        }
        builder.send().await
    }

    /// Send a request, authenticating and retrying as needed. Only successful
    /// responses are returned.
    async fn execute(
        &self,
        reference: &ImageReference,
        request: &Request<'_>,
        what: &str,
    ) -> OciResult<Response> {
        let host = reference.api_host();
        let url = Url::parse(&format!("{}://{host}{}", self.scheme(host), request.path))
            .map_err(|e| OciError::Registry(format!("invalid registry URL for {host}: {e}")))?;
        let key = (host.to_owned(), reference.repository.clone());
        let mut authorization = self.cached_auth(&key);
        let mut challenged = false;
        let mut attempt = 0u32;
        loop {
            attempt += 1;
            let response = match self.send(request, &url, authorization.as_deref()).await {
                Ok(response) => response,
                Err(error) => {
                    if attempt < self.max_attempts && is_transient(&error) {
                        self.backoff(attempt, None).await;
                        continue;
                    }
                    return Err(transport_error(host, &error));
                }
            };
            let status = response.status();
            if status.is_success() {
                return Ok(response);
            }
            if status == StatusCode::UNAUTHORIZED && !challenged {
                challenged = true;
                let (header, ttl) = self.authenticate(reference, &response).await?;
                self.store_auth(key.clone(), header.clone(), ttl);
                authorization = Some(header);
                continue;
            }
            if (status == StatusCode::TOO_MANY_REQUESTS || status.is_server_error())
                && attempt < self.max_attempts
            {
                self.backoff(attempt, retry_after(&response)).await;
                continue;
            }
            return Err(self.failure(reference, what, response).await);
        }
    }

    /// Answer a `401` challenge: returns the `Authorization` header value and
    /// how long it stays valid.
    async fn authenticate(
        &self,
        reference: &ImageReference,
        response: &Response,
    ) -> OciResult<(String, Duration)> {
        let host = reference.api_host();
        let values: Vec<&str> = response
            .headers()
            .get_all(reqwest::header::WWW_AUTHENTICATE)
            .iter()
            .filter_map(|value| value.to_str().ok())
            .collect();
        let challenges = parse_challenges(values);
        let credentials = self.credentials.lookup(&reference.registry).await?;

        let bearer = challenges.iter().find_map(|challenge| match challenge {
            Challenge::Bearer {
                realm,
                service,
                scope,
            } => Some((realm, service, scope)),
            _ => None,
        });
        if let Some((realm, service, scope)) = bearer {
            let default_scope = format!("repository:{}:pull", reference.repository);
            let scope = scope.as_deref().unwrap_or(&default_scope);
            let url = self.token_url(realm, service.as_deref(), scope, credentials.as_ref())?;
            return self
                .fetch_token(host, reference, url, credentials.as_ref())
                .await;
        }
        if challenges.contains(&Challenge::Basic) {
            let credentials = credentials.ok_or_else(|| {
                OciError::Auth(format!(
                    "{host} requires authentication for {}, but no credentials are configured for it",
                    reference.repository
                ))
            })?;
            // The request URL is HTTPS unless the operator declared the
            // registry insecure, so the credentials only travel in clear text
            // where that was asked for.
            return Ok((credentials.basic_header(), Duration::from_secs(3600)));
        }
        let offered: Vec<String> = challenges
            .iter()
            .map(|challenge| match challenge {
                Challenge::Bearer { .. } => "Bearer".to_owned(),
                Challenge::Basic => "Basic".to_owned(),
                Challenge::Other(name) => name.clone(),
            })
            .collect();
        Err(OciError::Auth(format!(
            "{host} asked for authentication this client does not support ({})",
            if offered.is_empty() {
                "no challenge".to_owned()
            } else {
                offered.join(", ")
            }
        )))
    }

    fn token_url(
        &self,
        realm: &str,
        service: Option<&str>,
        scope: &str,
        credentials: Option<&Credentials>,
    ) -> OciResult<Url> {
        let mut url = Url::parse(realm).map_err(|e| {
            OciError::Auth(format!("the registry named an invalid token endpoint: {e}"))
        })?;
        if !url.username().is_empty() || url.password().is_some() {
            return Err(OciError::Auth(
                "the registry named a token endpoint with embedded credentials".to_owned(),
            ));
        }
        let host_port = url_host_port(&url);
        match url.scheme() {
            "https" => {}
            "http" if host_is_insecure(&self.insecure, &host_port) => {}
            _ => {
                return Err(OciError::Auth(format!(
                    "refusing to use the non-HTTPS token endpoint on {host_port}"
                )));
            }
        }
        {
            let mut query = url.query_pairs_mut();
            if let Some(service) = service {
                query.append_pair("service", service);
            }
            query.append_pair("scope", scope);
            if let Some(credentials) = credentials {
                query.append_pair("account", &credentials.username);
            }
        }
        Ok(url)
    }

    async fn fetch_token(
        &self,
        host: &str,
        reference: &ImageReference,
        url: Url,
        credentials: Option<&Credentials>,
    ) -> OciResult<(String, Duration)> {
        let mut builder = self.http.get(url);
        if let Some(credentials) = credentials
            && let Ok(mut value) = HeaderValue::from_str(&credentials.basic_header())
        {
            value.set_sensitive(true);
            builder = builder.header(AUTHORIZATION, value);
        }
        let response = builder
            .send()
            .await
            .map_err(|e| transport_error(host, &e))?;
        let status = response.status();
        if status == StatusCode::UNAUTHORIZED || status == StatusCode::FORBIDDEN {
            return Err(OciError::Auth(match credentials {
                Some(_) => format!("{host} rejected the credentials configured for it"),
                None => format!(
                    "{} on {host} requires authentication, but no credentials are configured for it",
                    reference.repository
                ),
            }));
        }
        if !status.is_success() {
            return Err(
                if status.is_server_error() || status == StatusCode::TOO_MANY_REQUESTS {
                    OciError::Unavailable(format!(
                        "the token service of {host} answered HTTP {status}"
                    ))
                } else {
                    OciError::Auth(format!(
                        "the token service of {host} answered HTTP {status}"
                    ))
                },
            );
        }
        let body = self
            .read_limited(reference, response, MAX_TOKEN_RESPONSE_BYTES)
            .await?;
        #[derive(Deserialize)]
        struct TokenResponse {
            token: Option<String>,
            access_token: Option<String>,
            expires_in: Option<u64>,
        }
        let parsed: TokenResponse = serde_json::from_slice(&body).map_err(|e| {
            OciError::Auth(format!(
                "the token service of {host} sent an unusable answer: {e}"
            ))
        })?;
        let token = parsed
            .token
            .or(parsed.access_token)
            .filter(|token| !token.is_empty())
            .ok_or_else(|| OciError::Auth(format!("the token service of {host} sent no token")))?;
        let seconds = parsed.expires_in.unwrap_or(60).clamp(15, 3600) - 10;
        Ok((format!("Bearer {token}"), Duration::from_secs(seconds)))
    }

    async fn read_limited(
        &self,
        reference: &ImageReference,
        mut response: Response,
        limit: usize,
    ) -> OciResult<Vec<u8>> {
        if response
            .content_length()
            .is_some_and(|length| length > limit as u64)
        {
            return Err(OciError::Limit(format!(
                "the answer of {} is larger than {limit} bytes",
                reference.api_host()
            )));
        }
        let mut body = Vec::new();
        while let Some(chunk) = response
            .chunk()
            .await
            .map_err(|e| transport_error(reference.api_host(), &e))?
        {
            if body.len() + chunk.len() > limit {
                return Err(OciError::Limit(format!(
                    "the answer of {} is larger than {limit} bytes",
                    reference.api_host()
                )));
            }
            body.extend_from_slice(&chunk);
        }
        Ok(body)
    }

    async fn failure(
        &self,
        reference: &ImageReference,
        what: &str,
        response: Response,
    ) -> OciError {
        let host = reference.api_host();
        let status = response.status();
        let detail = match self
            .read_limited(reference, response, MAX_ERROR_BODY_BYTES)
            .await
        {
            Ok(body) => registry_error_text(&body),
            Err(_) => None,
        };
        let suffix = detail.map(|d| format!(": {d}")).unwrap_or_default();
        match status {
            StatusCode::NOT_FOUND => OciError::Registry(format!(
                "{what} of {reference} was not found on {host} (HTTP 404){suffix}"
            )),
            StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN => OciError::Auth(format!(
                "access to {} on {host} was denied (HTTP {}){suffix}; the image may not exist, or the credentials for {host} are missing or insufficient",
                reference.repository,
                status.as_u16()
            )),
            StatusCode::TOO_MANY_REQUESTS => OciError::Unavailable(format!(
                "{host} is rate limiting requests (HTTP 429){suffix}"
            )),
            s if s.is_server_error() => {
                OciError::Unavailable(format!("{host} answered HTTP {}{suffix}", s.as_u16()))
            }
            s => OciError::Registry(format!(
                "{host} answered HTTP {} for {what} of {reference}{suffix}",
                s.as_u16()
            )),
        }
    }
}

// ---------------------------------------------------------------------------
// helpers
// ---------------------------------------------------------------------------

fn host_is_insecure(list: &[String], host_port: &str) -> bool {
    let host_port = host_port.to_ascii_lowercase();
    let host = host_only(&host_port);
    list.iter()
        .any(|entry| entry == &host_port || entry == host)
}

fn url_host_port(url: &Url) -> String {
    match (url.host_str(), url.port()) {
        (Some(host), Some(port)) => format!("{host}:{port}"),
        (Some(host), None) => host.to_owned(),
        _ => String::new(),
    }
}

fn is_transient(error: &reqwest::Error) -> bool {
    !error.is_builder() && !error.is_redirect() && !error.is_decode()
}

fn transport_error(host: &str, error: &reqwest::Error) -> OciError {
    let mut message = error.to_string();
    let mut source = error.source();
    while let Some(inner) = source {
        message.push_str(": ");
        message.push_str(&inner.to_string());
        source = inner.source();
    }
    if error.is_redirect() {
        OciError::Registry(format!("{host}: {}", sanitize(&message, 300)))
    } else {
        OciError::Unavailable(format!(
            "cannot talk to {host}: {}",
            sanitize(&message, 300)
        ))
    }
}

fn retry_after(response: &Response) -> Option<Duration> {
    response
        .headers()
        .get(reqwest::header::RETRY_AFTER)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.trim().parse::<u64>().ok())
        .map(Duration::from_secs)
}

/// Single-line, bounded text safe for logs.
fn sanitize(text: &str, max: usize) -> String {
    let cleaned: String = text
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect();
    let cleaned = cleaned.trim();
    if cleaned.chars().count() > max {
        let mut cut: String = cleaned.chars().take(max).collect();
        cut.push('…');
        cut
    } else {
        cleaned.to_owned()
    }
}

/// The human-readable part of a registry error document.
fn registry_error_text(body: &[u8]) -> Option<String> {
    #[derive(Deserialize)]
    struct ErrorItem {
        #[serde(default)]
        code: String,
        #[serde(default)]
        message: String,
    }
    #[derive(Deserialize)]
    struct ErrorDocument {
        #[serde(default)]
        errors: Vec<ErrorItem>,
        #[serde(default)]
        error: Option<String>,
        #[serde(default)]
        error_description: Option<String>,
        #[serde(default)]
        details: Option<String>,
    }
    let document: ErrorDocument = serde_json::from_slice(body).ok()?;
    if let Some(item) = document.errors.first() {
        let text = match (item.code.is_empty(), item.message.is_empty()) {
            (false, false) => format!("{}: {}", item.code, item.message),
            (false, true) => item.code.clone(),
            (true, false) => item.message.clone(),
            (true, true) => return None,
        };
        return Some(sanitize(&text, 300));
    }
    let text = document
        .error_description
        .or(document.details)
        .or(document.error)?;
    Some(sanitize(&text, 300))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::oci::testing::{AuthMode, MockRegistry, TestImage};

    fn client(registry: &MockRegistry) -> RegistryClient {
        client_with(registry, CredentialStore::new())
    }

    fn client_with(registry: &MockRegistry, credentials: CredentialStore) -> RegistryClient {
        RegistryClient::new(RegistryOptions {
            credentials,
            insecure_registries: vec![registry.host(), registry.cdn_host()],
            retry_base: Duration::from_millis(5),
            read_timeout: Duration::from_secs(5),
            ..RegistryOptions::default()
        })
        .unwrap()
    }

    fn reference(registry: &MockRegistry, name: &str) -> ImageReference {
        ImageReference::parse(&format!("{}/{name}", registry.host())).unwrap()
    }

    #[test]
    fn challenges_are_parsed() {
        let challenges = parse_challenges([
            r#"Bearer realm="https://auth.docker.io/token",service="registry.docker.io",scope="repository:library/alpine:pull,push""#,
        ]);
        assert_eq!(
            challenges,
            vec![Challenge::Bearer {
                realm: "https://auth.docker.io/token".to_owned(),
                service: Some("registry.docker.io".to_owned()),
                scope: Some("repository:library/alpine:pull,push".to_owned()),
            }]
        );
        assert_eq!(
            parse_challenges([r#"Basic realm="Registry""#]),
            vec![Challenge::Basic]
        );
        let both = parse_challenges([r#"Bearer realm="https://a/token", Basic realm="x""#]);
        assert_eq!(both.len(), 2);
        assert!(matches!(both[0], Challenge::Bearer { .. }));
        assert_eq!(both[1], Challenge::Basic);
        assert!(matches!(
            parse_challenges([r#"Negotiate"#]).as_slice(),
            [Challenge::Other(name)] if name == "negotiate"
        ));
        assert!(matches!(
            parse_challenges([r#"Bearer service="x""#]).as_slice(),
            [Challenge::Other(_)]
        ));
        // unquoted values and escaped quotes
        let unquoted = parse_challenges([r#"bearer realm=https://a/token,service=s"#]);
        assert_eq!(
            unquoted,
            vec![Challenge::Bearer {
                realm: "https://a/token".to_owned(),
                service: Some("s".to_owned()),
                scope: None
            }]
        );
        assert!(parse_challenges([""]).is_empty());
        assert!(parse_challenges(["===,,,"]).is_empty());
    }

    #[test]
    fn registry_keys_are_normalized() {
        assert_eq!(
            normalize_registry_key("https://index.docker.io/v1/"),
            "docker.io"
        );
        assert_eq!(normalize_registry_key("docker.io"), "docker.io");
        assert_eq!(normalize_registry_key("GHCR.io"), "ghcr.io");
        assert_eq!(
            normalize_registry_key("http://localhost:5000/"),
            "localhost:5000"
        );
    }

    #[test]
    fn insecure_matching_accepts_host_or_host_port() {
        let list = vec!["registry.local".to_owned(), "10.0.0.5:5000".to_owned()];
        assert!(host_is_insecure(&list, "registry.local:5000"));
        assert!(host_is_insecure(&list, "Registry.Local"));
        assert!(host_is_insecure(&list, "10.0.0.5:5000"));
        assert!(!host_is_insecure(&list, "10.0.0.5:5001"));
        assert!(!host_is_insecure(&list, "ghcr.io"));
    }

    #[test]
    fn credentials_never_show_up_in_debug_output() {
        let text = format!("{:?}", Credentials::new("alice", "hunter2"));
        assert!(
            text.contains("alice") && !text.contains("hunter2"),
            "{text}"
        );
    }

    #[tokio::test]
    async fn docker_config_credentials_are_read_in_every_supported_shape() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.json");
        let auth = base64::engine::general_purpose::STANDARD.encode("alice:p:ss");
        std::fs::write(
            &path,
            format!(
                r#"{{"auths":{{
                "https://index.docker.io/v1/":{{"auth":"{auth}"}},
                "ghcr.io":{{"username":"bob","password":"tok"}},
                "quay.io":{{"identitytoken":"abc"}},
                "broken.example":{{"auth":"!!!"}},
                "nocolon.example":{{"auth":"{}"}}
                }}}}"#,
                base64::engine::general_purpose::STANDARD.encode("nocolon")
            ),
        )
        .unwrap();
        let store = CredentialStore::new().with_file(&path);
        let hub = store.lookup("docker.io").await.unwrap().unwrap();
        assert_eq!(
            (hub.username.as_str(), hub.password.as_str()),
            ("alice", "p:ss")
        );
        let ghcr = store.lookup("ghcr.io").await.unwrap().unwrap();
        assert_eq!(
            (ghcr.username.as_str(), ghcr.password.as_str()),
            ("bob", "tok")
        );
        assert!(store.lookup("unknown.example").await.unwrap().is_none());
        assert!(
            store
                .lookup("quay.io")
                .await
                .unwrap_err()
                .to_string()
                .contains("identity token")
        );
        assert!(
            store
                .lookup("broken.example")
                .await
                .unwrap_err()
                .to_string()
                .contains("base64")
        );
        assert!(
            store
                .lookup("nocolon.example")
                .await
                .unwrap_err()
                .to_string()
                .contains("user:password")
        );

        // explicit credentials win over the file
        let store = store.with_credentials("ghcr.io", Credentials::new("carol", "x"));
        assert_eq!(
            store.lookup("ghcr.io").await.unwrap().unwrap().username,
            "carol"
        );

        // a missing file simply means "no credentials"; a malformed one is reported
        let missing = CredentialStore::new().with_file(dir.path().join("nope.json"));
        assert!(missing.lookup("ghcr.io").await.unwrap().is_none());
        std::fs::write(&path, "{not json").unwrap();
        let error = CredentialStore::new()
            .with_file(&path)
            .lookup("ghcr.io")
            .await
            .unwrap_err();
        assert!(
            error.to_string().contains("not a valid Docker config.json"),
            "{error}"
        );
    }

    #[tokio::test]
    async fn credential_helpers_are_asked_for_credentials() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let helper = dir.path().join("docker-credential-onedevtest");
        std::fs::write(
            &helper,
            "#!/bin/sh\nread server\nif [ \"$server\" = \"vault.example\" ]; then\n  echo '{\"ServerURL\":\"vault.example\",\"Username\":\"helper-user\",\"Secret\":\"helper-secret\"}'\nelse\n  echo 'credentials not found in native keychain'\n  exit 1\nfi\n",
        )
        .unwrap();
        std::fs::set_permissions(&helper, std::fs::Permissions::from_mode(0o755)).unwrap();
        let config = dir.path().join("config.json");
        std::fs::write(&config, r#"{"credsStore":"onedevtest"}"#).unwrap();

        let path = std::env::var_os("PATH").unwrap_or_default();
        let mut joined = std::ffi::OsString::from(dir.path());
        joined.push(":");
        joined.push(path);
        // SAFETY: tests of this module are the only ones touching PATH for
        // helper lookup and run before any helper process is spawned here.
        unsafe { std::env::set_var("PATH", &joined) };

        let store = CredentialStore::new().with_file(&config);
        let found = store.lookup("vault.example").await.unwrap().unwrap();
        assert_eq!(
            (found.username.as_str(), found.password.as_str()),
            ("helper-user", "helper-secret")
        );
        assert!(store.lookup("other.example").await.unwrap().is_none());

        let bad = dir.path().join("bad.json");
        std::fs::write(&bad, r#"{"credsStore":"../evil"}"#).unwrap();
        let error = CredentialStore::new()
            .with_file(&bad)
            .lookup("x.example")
            .await
            .unwrap_err();
        assert!(
            error
                .to_string()
                .contains("not a valid credential helper name"),
            "{error}"
        );
    }

    #[tokio::test]
    async fn manifests_configs_and_blobs_are_fetched_and_verified() {
        let registry = MockRegistry::start();
        let image = TestImage::simple();
        let pushed = image.push(&registry, "team/app", "1.0");
        let client = client(&registry);
        let reference = reference(&registry, "team/app:1.0");

        let head = client.head_digest(&reference).await.unwrap();
        assert_eq!(head.as_deref(), Some(pushed.manifest_digest.as_str()));

        let resolved = client
            .resolve(&reference, &Platform::linux_host())
            .await
            .unwrap();
        assert_eq!(resolved.manifest_digest, pushed.manifest_digest);
        assert!(resolved.index_digest.is_none());
        assert_eq!(resolved.manifest.layers.len(), image.layer_count());

        let config = client
            .fetch_config(&reference, &resolved.manifest)
            .await
            .unwrap();
        assert_eq!(config.os, "linux");
        assert!(config.env.iter().any(|e| e.starts_with("PATH=")));

        let dir = tempfile::tempdir().unwrap();
        let dest = dir.path().join("layer0");
        let seen = std::sync::atomic::AtomicU64::new(0);
        client
            .download_blob(&reference, &resolved.manifest.layers[0], &dest, &|n| {
                seen.fetch_add(n, std::sync::atomic::Ordering::SeqCst);
            })
            .await
            .unwrap();
        assert_eq!(
            std::fs::metadata(&dest).unwrap().len(),
            resolved.manifest.layers[0].size
        );
        assert_eq!(
            seen.load(std::sync::atomic::Ordering::SeqCst),
            resolved.manifest.layers[0].size
        );
    }

    #[tokio::test]
    async fn pinned_digests_are_verified_and_missing_images_reported() {
        let registry = MockRegistry::start();
        let pushed = TestImage::simple().push(&registry, "team/app", "latest");
        let client = client(&registry);
        let pinned = ImageReference::parse(&format!(
            "{}/team/app@{}",
            registry.host(),
            pushed.manifest_digest
        ))
        .unwrap();
        client
            .resolve(&pinned, &Platform::linux_host())
            .await
            .unwrap();

        let missing = reference(&registry, "team/ghost:1");
        let error = client
            .resolve(&missing, &Platform::linux_host())
            .await
            .unwrap_err();
        assert!(matches!(error, OciError::Registry(_)), "{error:?}");
        assert!(error.to_string().contains("not found"), "{error}");

        // the registry serves different bytes than the pinned digest
        let other = "sha256:00000000000000000000000000000000000000000000000000000000000000aa";
        let lying =
            ImageReference::parse(&format!("{}/team/app@{other}", registry.host())).unwrap();
        registry.alias_manifest("team/app", other, &pushed.manifest_digest);
        let error = client
            .resolve(&lying, &Platform::linux_host())
            .await
            .unwrap_err();
        assert!(matches!(error, OciError::Digest { .. }), "{error:?}");
    }

    #[tokio::test]
    async fn multi_platform_indexes_are_resolved_for_the_requested_platform() {
        let registry = MockRegistry::start();
        let amd64 =
            TestImage::simple()
                .with_platform("amd64")
                .push(&registry, "team/multi", "amd64-only");
        let arm64 = TestImage::simple()
            .with_marker("arm")
            .with_platform("arm64")
            .push(&registry, "team/multi", "arm64-only");
        let index = registry.push_index(
            "team/multi",
            "latest",
            &[
                ("amd64", &amd64.manifest_digest),
                ("arm64", &arm64.manifest_digest),
            ],
        );
        let client = client(&registry);
        let reference = reference(&registry, "team/multi:latest");
        let resolved = client
            .resolve(&reference, &Platform::new("linux", "arm64", None))
            .await
            .unwrap();
        assert_eq!(resolved.manifest_digest, arm64.manifest_digest);
        assert_eq!(resolved.index_digest.as_deref(), Some(index.as_str()));
        let error = client
            .resolve(&reference, &Platform::new("linux", "s390x", None))
            .await
            .unwrap_err();
        assert!(
            error.to_string().contains("no variant for linux/s390x"),
            "{error}"
        );
    }

    #[tokio::test]
    async fn basic_authentication_uses_the_configured_credentials() {
        let registry = MockRegistry::start();
        registry.set_auth(AuthMode::Basic {
            username: "ci".into(),
            password: "s3cret".into(),
        });
        TestImage::simple().push(&registry, "team/app", "1");
        let reference = reference(&registry, "team/app:1");

        let anonymous = client(&registry);
        let error = anonymous
            .resolve(&reference, &Platform::linux_host())
            .await
            .unwrap_err();
        assert!(matches!(error, OciError::Auth(_)), "{error:?}");
        assert!(
            error.to_string().contains("no credentials are configured"),
            "{error}"
        );

        let wrong = client_with(
            &registry,
            CredentialStore::new()
                .with_credentials(&registry.host(), Credentials::new("ci", "nope")),
        );
        let error = wrong
            .resolve(&reference, &Platform::linux_host())
            .await
            .unwrap_err();
        assert!(matches!(error, OciError::Auth(_)), "{error:?}");

        let right = client_with(
            &registry,
            CredentialStore::new()
                .with_credentials(&registry.host(), Credentials::new("ci", "s3cret")),
        );
        right
            .resolve(&reference, &Platform::linux_host())
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn bearer_tokens_are_fetched_with_credentials_and_cached() {
        let registry = MockRegistry::start();
        registry.set_auth(AuthMode::Bearer {
            credentials: Some(("ci".into(), "s3cret".into())),
        });
        TestImage::simple().push(&registry, "team/app", "1");
        let reference = reference(&registry, "team/app:1");

        let anonymous = client(&registry);
        let error = anonymous
            .resolve(&reference, &Platform::linux_host())
            .await
            .unwrap_err();
        assert!(matches!(error, OciError::Auth(_)), "{error:?}");

        let client = client_with(
            &registry,
            CredentialStore::new()
                .with_credentials(&registry.host(), Credentials::new("ci", "s3cret")),
        );
        client
            .resolve(&reference, &Platform::linux_host())
            .await
            .unwrap();
        let token_requests = registry
            .requests()
            .iter()
            .filter(|r| r.path.starts_with("/token"))
            .count();
        // one failed anonymous attempt above (not counted for `client`), one real fetch
        let before = token_requests;
        client
            .resolve(&reference, &Platform::linux_host())
            .await
            .unwrap();
        let after = registry
            .requests()
            .iter()
            .filter(|r| r.path.starts_with("/token"))
            .count();
        assert_eq!(before, after, "the token is cached across requests");
        let token_calls = registry.requests();
        let last_token = token_calls
            .iter()
            .rev()
            .find(|r| r.path.starts_with("/token"))
            .unwrap();
        assert!(
            last_token
                .path
                .contains("scope=repository%3Ateam%2Fapp%3Apull"),
            "{}",
            last_token.path
        );
        assert!(
            last_token
                .authorization
                .as_deref()
                .is_some_and(|a| a.starts_with("Basic "))
        );
    }

    #[tokio::test]
    async fn anonymous_bearer_tokens_work_for_public_repositories() {
        let registry = MockRegistry::start();
        registry.set_auth(AuthMode::Bearer { credentials: None });
        TestImage::simple().push(&registry, "library/public", "1");
        let client = client(&registry);
        client
            .resolve(
                &reference(&registry, "library/public:1"),
                &Platform::linux_host(),
            )
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn transient_failures_are_retried_and_persistent_ones_reported() {
        let registry = MockRegistry::start();
        let pushed = TestImage::simple().push(&registry, "team/app", "1");
        let client = client(&registry);
        let reference = reference(&registry, "team/app:1");

        registry.fail_next("/manifests/1", &[503, 429]);
        let resolved = client
            .resolve(&reference, &Platform::linux_host())
            .await
            .unwrap();
        assert_eq!(resolved.manifest_digest, pushed.manifest_digest);

        registry.fail_next("/manifests/1", &[500, 500, 500, 500]);
        let error = client
            .resolve(&reference, &Platform::linux_host())
            .await
            .unwrap_err();
        assert!(matches!(error, OciError::Unavailable(_)), "{error:?}");
        registry.clear_failures();

        let unreachable = ImageReference::parse("127.0.0.1:1/x/y:1").unwrap();
        let strict = RegistryClient::new(RegistryOptions {
            insecure_registries: vec!["127.0.0.1:1".into()],
            retry_base: Duration::from_millis(1),
            ..RegistryOptions::default()
        })
        .unwrap();
        let error = strict
            .resolve(&unreachable, &Platform::linux_host())
            .await
            .unwrap_err();
        assert!(matches!(error, OciError::Unavailable(_)), "{error:?}");
    }

    #[tokio::test]
    async fn corrupted_and_truncated_layers_are_rejected_without_leaving_files() {
        let registry = MockRegistry::start();
        let image = TestImage::simple();
        let pushed = image.push(&registry, "team/app", "1");
        let client = client(&registry);
        let reference = reference(&registry, "team/app:1");
        let resolved = client
            .resolve(&reference, &Platform::linux_host())
            .await
            .unwrap();
        let layer = &resolved.manifest.layers[0];
        let dir = tempfile::tempdir().unwrap();

        registry.corrupt_blob(&layer.digest);
        let dest = dir.path().join("corrupt");
        let error = client
            .download_blob(&reference, layer, &dest, &|_| {})
            .await
            .unwrap_err();
        assert!(matches!(error, OciError::Digest { .. }), "{error:?}");
        assert!(!dest.exists(), "partial downloads are removed");

        registry.restore_blob(&layer.digest);
        let mut oversized = layer.clone();
        oversized.size = layer.size - 1;
        let dest = dir.path().join("oversized");
        let error = client
            .download_blob(&reference, &oversized, &dest, &|_| {})
            .await
            .unwrap_err();
        assert!(error.to_string().contains("larger than"), "{error}");
        assert!(!dest.exists());

        let mut inflated = layer.clone();
        inflated.size = layer.size + 10;
        let dest = dir.path().join("short");
        let error = client
            .download_blob(&reference, &inflated, &dest, &|_| {})
            .await
            .unwrap_err();
        assert!(matches!(error, OciError::Unavailable(_)), "{error:?}");
        assert!(!dest.exists());
        let _ = pushed;
    }

    #[tokio::test]
    async fn blob_redirects_are_followed_without_forwarding_credentials() {
        let registry = MockRegistry::start();
        registry.set_auth(AuthMode::Basic {
            username: "ci".into(),
            password: "pw".into(),
        });
        TestImage::simple().push(&registry, "team/app", "1");
        registry.redirect_blobs(true);
        let client = client_with(
            &registry,
            CredentialStore::new().with_credentials(&registry.host(), Credentials::new("ci", "pw")),
        );
        let reference = reference(&registry, "team/app:1");
        let resolved = client
            .resolve(&reference, &Platform::linux_host())
            .await
            .unwrap();
        let dir = tempfile::tempdir().unwrap();
        client
            .download_blob(
                &reference,
                &resolved.manifest.layers[0],
                &dir.path().join("l"),
                &|_| {},
            )
            .await
            .unwrap();
        let requests = registry.requests();
        let cdn = requests
            .iter()
            .find(|r| r.on_cdn)
            .expect("the blob came from the redirect target");
        assert!(
            cdn.authorization.is_none(),
            "credentials are not forwarded to another origin"
        );
    }

    #[tokio::test]
    async fn redirects_to_hosts_that_are_not_declared_insecure_are_refused() {
        let registry = MockRegistry::start();
        TestImage::simple().push(&registry, "team/app", "1");
        registry.redirect_blobs(true);
        // the registry itself may use HTTP, the redirect target may not
        let client = RegistryClient::new(RegistryOptions {
            insecure_registries: vec![registry.host()],
            retry_base: Duration::from_millis(1),
            max_attempts: 1,
            ..RegistryOptions::default()
        })
        .unwrap();
        let reference = reference(&registry, "team/app:1");
        let resolved = client
            .resolve(&reference, &Platform::linux_host())
            .await
            .unwrap();
        let dir = tempfile::tempdir().unwrap();
        let error = client
            .download_blob(
                &reference,
                &resolved.manifest.layers[0],
                &dir.path().join("l"),
                &|_| {},
            )
            .await
            .unwrap_err();
        assert!(matches!(error, OciError::Registry(_)), "{error:?}");
        assert!(error.to_string().contains("redirect"), "{error}");
        assert!(
            registry.requests().iter().all(|r| !r.on_cdn),
            "the CDN was never contacted"
        );
    }

    #[tokio::test]
    async fn stalled_transfers_time_out_instead_of_hanging() {
        let registry = MockRegistry::start();
        TestImage::simple().push(&registry, "team/app", "1");
        let client = RegistryClient::new(RegistryOptions {
            insecure_registries: vec![registry.host()],
            retry_base: Duration::from_millis(1),
            read_timeout: Duration::from_millis(300),
            max_attempts: 2,
            ..RegistryOptions::default()
        })
        .unwrap();
        let reference = reference(&registry, "team/app:1");
        let resolved = client
            .resolve(&reference, &Platform::linux_host())
            .await
            .unwrap();
        let layer = &resolved.manifest.layers[0];
        registry.stall_blob(&layer.digest);
        let dir = tempfile::tempdir().unwrap();
        let dest = dir.path().join("stalled");
        let started = std::time::Instant::now();
        let error = client
            .download_blob(&reference, layer, &dest, &|_| {})
            .await
            .unwrap_err();
        assert!(matches!(error, OciError::Unavailable(_)), "{error:?}");
        assert!(started.elapsed() < Duration::from_secs(10));
        assert!(!dest.exists());
    }
}
