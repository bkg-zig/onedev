//! Test fixtures: tar archives, an in-process registry and image builders.

use std::collections::{HashMap, HashSet, VecDeque};
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use base64::Engine as _;
use flate2::write::GzEncoder;

use super::manifest::{MEDIA_OCI_INDEX, MEDIA_OCI_MANIFEST, Platform};
use super::sha256_digest;

// ---------------------------------------------------------------------------
// tar archives
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    File,
    Dir,
    Symlink,
    Hardlink,
    Fifo,
    Char,
}

/// One archive entry. Everything is written as a raw header so the tests can
/// produce archives (`..` paths, odd link targets) the `tar` builder refuses.
#[derive(Clone)]
pub(crate) struct TarEntry {
    kind: Kind,
    path: Vec<u8>,
    mode: u32,
    content: Vec<u8>,
    link: Vec<u8>,
    mtime: u64,
}

impl TarEntry {
    fn new(kind: Kind, path: &str, mode: u32) -> Self {
        Self {
            kind,
            path: path.as_bytes().to_vec(),
            mode,
            content: Vec::new(),
            link: Vec::new(),
            mtime: 1_700_000_000,
        }
    }

    pub fn file(path: &str, mode: u32, content: impl Into<Vec<u8>>) -> Self {
        let mut entry = Self::new(Kind::File, path, mode);
        entry.content = content.into();
        entry
    }

    /// A regular file with default mode (for odd paths).
    pub fn raw_file(path: &str, content: &str) -> Self {
        Self::file(path, 0o644, content)
    }

    pub fn dir(path: &str, mode: u32) -> Self {
        Self::new(Kind::Dir, path, mode)
    }

    pub fn symlink(path: &str, target: &str) -> Self {
        let mut entry = Self::new(Kind::Symlink, path, 0o777);
        entry.link = target.as_bytes().to_vec();
        entry
    }

    pub fn hardlink(path: &str, target: &str) -> Self {
        let mut entry = Self::new(Kind::Hardlink, path, 0o644);
        entry.link = target.as_bytes().to_vec();
        entry
    }

    pub fn raw_hardlink(path: &str, target: &str) -> Self {
        Self::hardlink(path, target)
    }

    pub fn fifo(path: &str, mode: u32) -> Self {
        Self::new(Kind::Fifo, path, mode)
    }

    pub fn char_device(path: &str) -> Self {
        Self::new(Kind::Char, path, 0o666)
    }

    pub fn with_mtime(mut self, mtime: u64) -> Self {
        self.mtime = mtime;
        self
    }
}

/// An (uncompressed) tar archive of `entries`.
pub(crate) fn tar_bytes(entries: &[TarEntry]) -> Vec<u8> {
    let mut out = Vec::new();
    for entry in entries {
        let mut header = tar::Header::new_gnu();
        header.set_mode(entry.mode);
        header.set_uid(0);
        header.set_gid(0);
        header.set_mtime(entry.mtime);
        header.set_size(if entry.kind == Kind::File {
            entry.content.len() as u64
        } else {
            0
        });
        header.set_entry_type(match entry.kind {
            Kind::File => tar::EntryType::Regular,
            Kind::Dir => tar::EntryType::Directory,
            Kind::Symlink => tar::EntryType::Symlink,
            Kind::Hardlink => tar::EntryType::Link,
            Kind::Fifo => tar::EntryType::Fifo,
            Kind::Char => tar::EntryType::Char,
        });
        {
            let old = header.as_old_mut();
            assert!(
                entry.path.len() < 100 && entry.link.len() < 100,
                "test paths are short"
            );
            old.name[..entry.path.len()].copy_from_slice(&entry.path);
            old.linkname[..entry.link.len()].copy_from_slice(&entry.link);
        }
        header.set_cksum();
        out.extend_from_slice(header.as_bytes());
        if entry.kind == Kind::File {
            out.extend_from_slice(&entry.content);
            let pad = (512 - entry.content.len() % 512) % 512;
            out.extend(std::iter::repeat_n(0u8, pad));
        }
    }
    out.extend(std::iter::repeat_n(0u8, 1024));
    out
}

pub(crate) fn gzip(bytes: &[u8]) -> Vec<u8> {
    let mut encoder = GzEncoder::new(Vec::new(), flate2::Compression::fast());
    encoder.write_all(bytes).expect("gzip in memory");
    encoder.finish().expect("gzip in memory")
}

// ---------------------------------------------------------------------------
// images
// ---------------------------------------------------------------------------

/// An image under construction: uncompressed layer tars plus configuration.
#[derive(Clone)]
pub(crate) struct TestImage {
    layers: Vec<Vec<u8>>,
    pub env: Vec<String>,
    pub working_dir: String,
    pub user: String,
    architecture: String,
    os: String,
    marker: Option<String>,
    layer_type: LayerType,
}

#[derive(Clone, Copy)]
pub(crate) enum LayerType {
    OciGzip,
    DockerGzip,
    OciUncompressed,
}

#[derive(Debug, Clone)]
pub(crate) struct PushedImage {
    pub manifest_digest: String,
    pub config_digest: String,
    pub layer_digests: Vec<String>,
}

impl TestImage {
    /// Two small layers for the current platform.
    pub fn simple() -> Self {
        Self::with_layers(vec![
            tar_bytes(&[
                TarEntry::dir("etc/", 0o755),
                TarEntry::file("etc/os-release", 0o644, "NAME=test\n"),
                TarEntry::dir("tmp/", 0o1777),
            ]),
            tar_bytes(&[TarEntry::file(
                "app/hello.txt",
                0o644,
                "hello from the image\n",
            )]),
        ])
    }

    pub fn with_layers(layers: Vec<Vec<u8>>) -> Self {
        Self {
            layers,
            env: vec![
                "PATH=/usr/local/bin:/usr/bin:/bin".to_owned(),
                "FROM_IMAGE=yes".to_owned(),
            ],
            working_dir: String::new(),
            user: String::new(),
            architecture: Platform::linux_host().architecture,
            os: "linux".to_owned(),
            marker: None,
            layer_type: LayerType::OciGzip,
        }
    }

    pub fn with_platform(mut self, architecture: &str) -> Self {
        self.architecture = architecture.to_owned();
        self
    }

    /// Makes the image content (and so its digest) unique.
    pub fn with_marker(mut self, marker: &str) -> Self {
        self.marker = Some(marker.to_owned());
        self
    }

    pub fn with_layer_type(mut self, layer_type: LayerType) -> Self {
        self.layer_type = layer_type;
        self
    }

    pub fn with_env(mut self, env: &[&str]) -> Self {
        self.env = env.iter().map(|e| (*e).to_owned()).collect();
        self
    }

    pub fn layer_count(&self) -> usize {
        self.layers.len() + usize::from(self.marker.is_some())
    }

    /// Upload the image to `registry` as `repo:tag` (and by digest).
    pub fn push(&self, registry: &MockRegistry, repo: &str, tag: &str) -> PushedImage {
        let mut layers = self.layers.clone();
        if let Some(marker) = &self.marker {
            layers.push(tar_bytes(&[TarEntry::file(
                "marker",
                0o644,
                marker.as_str(),
            )]));
        }
        let mut descriptors = Vec::new();
        let mut diff_ids = Vec::new();
        let mut layer_digests = Vec::new();
        for tar in &layers {
            diff_ids.push(sha256_digest(tar));
            let (media_type, blob) = match self.layer_type {
                LayerType::OciGzip => ("application/vnd.oci.image.layer.v1.tar+gzip", gzip(tar)),
                LayerType::DockerGzip => (
                    "application/vnd.docker.image.rootfs.diff.tar.gzip",
                    gzip(tar),
                ),
                LayerType::OciUncompressed => {
                    ("application/vnd.oci.image.layer.v1.tar", tar.clone())
                }
            };
            let digest = registry.put_blob(blob.clone());
            layer_digests.push(digest.clone());
            descriptors.push(serde_json::json!({
                "mediaType": media_type,
                "digest": digest,
                "size": blob.len(),
            }));
        }
        let config = serde_json::json!({
            "architecture": self.architecture,
            "os": self.os,
            "config": {
                "Env": self.env,
                "WorkingDir": self.working_dir,
                "User": self.user,
                "Cmd": ["sh"],
            },
            "rootfs": { "type": "layers", "diff_ids": diff_ids },
        });
        let config_bytes = serde_json::to_vec(&config).unwrap();
        let config_digest = registry.put_blob(config_bytes.clone());
        let manifest = serde_json::json!({
            "schemaVersion": 2,
            "mediaType": MEDIA_OCI_MANIFEST,
            "config": {
                "mediaType": "application/vnd.oci.image.config.v1+json",
                "digest": config_digest,
                "size": config_bytes.len(),
            },
            "layers": descriptors,
        });
        let manifest_digest = registry.put_manifest(
            repo,
            tag,
            MEDIA_OCI_MANIFEST,
            serde_json::to_vec(&manifest).unwrap(),
        );
        PushedImage {
            manifest_digest,
            config_digest,
            layer_digests,
        }
    }
}

// ---------------------------------------------------------------------------
// registry
// ---------------------------------------------------------------------------

pub(crate) enum AuthMode {
    None,
    Basic {
        username: String,
        password: String,
    },
    /// Requires a token from `/token`; the token endpoint demands these
    /// credentials (HTTP Basic) when given, and is anonymous otherwise.
    Bearer {
        credentials: Option<(String, String)>,
    },
}

#[derive(Debug, Clone)]
pub(crate) struct Recorded {
    pub method: String,
    pub path: String,
    pub authorization: Option<String>,
    /// Which listener answered: the registry itself or the blob "CDN".
    pub on_cdn: bool,
}

struct State {
    auth: AuthMode,
    blobs: HashMap<String, Vec<u8>>,
    manifests: HashMap<(String, String), (String, Vec<u8>)>,
    requests: Vec<Recorded>,
    failures: Vec<(String, VecDeque<u16>)>,
    redirect_blobs: bool,
    corrupt: HashSet<String>,
    stall: HashSet<String>,
}

const TOKEN: &str = "mock-token-123";

pub(crate) struct MockRegistry {
    addr: SocketAddr,
    cdn_addr: SocketAddr,
    state: Arc<Mutex<State>>,
    stop: Arc<AtomicBool>,
}

impl MockRegistry {
    pub fn start() -> Self {
        let state = Arc::new(Mutex::new(State {
            auth: AuthMode::None,
            blobs: HashMap::new(),
            manifests: HashMap::new(),
            requests: Vec::new(),
            failures: Vec::new(),
            redirect_blobs: false,
            corrupt: HashSet::new(),
            stall: HashSet::new(),
        }));
        let stop = Arc::new(AtomicBool::new(false));
        let main = TcpListener::bind("127.0.0.1:0").expect("bind the mock registry");
        let cdn = TcpListener::bind("127.0.0.1:0").expect("bind the mock cdn");
        let addr = main.local_addr().unwrap();
        let cdn_addr = cdn.local_addr().unwrap();
        for (listener, on_cdn) in [(main, false), (cdn, true)] {
            let state = state.clone();
            let stop = stop.clone();
            std::thread::spawn(move || {
                for stream in listener.incoming() {
                    if stop.load(Ordering::SeqCst) {
                        break;
                    }
                    let Ok(stream) = stream else { continue };
                    let state = state.clone();
                    let stop = stop.clone();
                    std::thread::spawn(move || {
                        let _ = serve(stream, &state, &stop, addr, cdn_addr, on_cdn);
                    });
                }
            });
        }
        Self {
            addr,
            cdn_addr,
            state,
            stop,
        }
    }

    /// `127.0.0.1:<port>`, as used in image references.
    pub fn host(&self) -> String {
        self.addr.to_string()
    }

    pub fn cdn_host(&self) -> String {
        self.cdn_addr.to_string()
    }

    fn with<T>(&self, f: impl FnOnce(&mut State) -> T) -> T {
        f(&mut self.state.lock().unwrap_or_else(|p| p.into_inner()))
    }

    pub fn set_auth(&self, auth: AuthMode) {
        self.with(|s| s.auth = auth);
    }

    pub fn put_blob(&self, bytes: Vec<u8>) -> String {
        let digest = sha256_digest(&bytes);
        self.with(|s| s.blobs.insert(digest.clone(), bytes));
        digest
    }

    /// Store a manifest under `reference` and under its digest.
    pub fn put_manifest(
        &self,
        repo: &str,
        reference: &str,
        content_type: &str,
        body: Vec<u8>,
    ) -> String {
        let digest = sha256_digest(&body);
        self.with(|s| {
            s.manifests.insert(
                (repo.to_owned(), reference.to_owned()),
                (content_type.to_owned(), body.clone()),
            );
            s.manifests.insert(
                (repo.to_owned(), digest.clone()),
                (content_type.to_owned(), body),
            );
        });
        digest
    }

    /// Serve the manifest with `existing_digest` also under `as_reference`.
    pub fn alias_manifest(&self, repo: &str, as_reference: &str, existing_digest: &str) {
        self.with(|s| {
            let entry = s
                .manifests
                .get(&(repo.to_owned(), existing_digest.to_owned()))
                .cloned()
                .expect("manifest exists");
            s.manifests
                .insert((repo.to_owned(), as_reference.to_owned()), entry);
        });
    }

    /// Publish an image index over single-platform manifests.
    pub fn push_index(&self, repo: &str, tag: &str, entries: &[(&str, &str)]) -> String {
        let manifests: Vec<serde_json::Value> = entries
            .iter()
            .map(|(architecture, digest)| {
                serde_json::json!({
                    "mediaType": MEDIA_OCI_MANIFEST,
                    "digest": digest,
                    "size": 1,
                    "platform": {"architecture": architecture, "os": "linux"},
                })
            })
            .collect();
        let index = serde_json::json!({
            "schemaVersion": 2,
            "mediaType": MEDIA_OCI_INDEX,
            "manifests": manifests,
        });
        self.put_manifest(
            repo,
            tag,
            MEDIA_OCI_INDEX,
            serde_json::to_vec(&index).unwrap(),
        )
    }

    pub fn requests(&self) -> Vec<Recorded> {
        self.with(|s| s.requests.clone())
    }

    pub fn clear_requests(&self) {
        self.with(|s| s.requests.clear());
    }

    /// Answer the next requests whose path contains `needle` with `statuses`.
    pub fn fail_next(&self, needle: &str, statuses: &[u16]) {
        self.with(|s| {
            s.failures
                .push((needle.to_owned(), statuses.iter().copied().collect()))
        });
    }

    pub fn clear_failures(&self) {
        self.with(|s| s.failures.clear());
    }

    pub fn corrupt_blob(&self, digest: &str) {
        self.with(|s| s.corrupt.insert(digest.to_owned()));
    }

    pub fn restore_blob(&self, digest: &str) {
        self.with(|s| s.corrupt.remove(digest));
    }

    /// Send half of the blob, then keep the connection open and silent.
    pub fn stall_blob(&self, digest: &str) {
        self.with(|s| s.stall.insert(digest.to_owned()));
    }

    pub fn redirect_blobs(&self, on: bool) {
        self.with(|s| s.redirect_blobs = on);
    }
}

impl Drop for MockRegistry {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        // wake the accept loops so the threads end
        let _ = TcpStream::connect(self.addr);
        let _ = TcpStream::connect(self.cdn_addr);
    }
}

fn status_text(status: u16) -> &'static str {
    match status {
        200 => "OK",
        307 => "Temporary Redirect",
        401 => "Unauthorized",
        404 => "Not Found",
        429 => "Too Many Requests",
        500 => "Internal Server Error",
        503 => "Service Unavailable",
        _ => "Status",
    }
}

fn respond(
    stream: &mut TcpStream,
    status: u16,
    headers: &[(&str, String)],
    body: &[u8],
    head_only: bool,
) -> std::io::Result<()> {
    let mut text = format!("HTTP/1.1 {status} {}\r\n", status_text(status));
    for (name, value) in headers {
        text.push_str(&format!("{name}: {value}\r\n"));
    }
    text.push_str(&format!(
        "Content-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    ));
    stream.write_all(text.as_bytes())?;
    if !head_only {
        stream.write_all(body)?;
    }
    stream.flush()
}

fn error_body(code: &str, message: &str) -> Vec<u8> {
    serde_json::to_vec(&serde_json::json!({"errors": [{"code": code, "message": message}]}))
        .unwrap()
}

fn serve(
    mut stream: TcpStream,
    state: &Mutex<State>,
    stop: &AtomicBool,
    addr: SocketAddr,
    cdn_addr: SocketAddr,
    on_cdn: bool,
) -> std::io::Result<()> {
    stream.set_read_timeout(Some(Duration::from_secs(10)))?;
    let mut buffer = Vec::new();
    let mut chunk = [0u8; 4096];
    while !buffer.windows(4).any(|w| w == b"\r\n\r\n") {
        let read = stream.read(&mut chunk)?;
        if read == 0 {
            return Ok(());
        }
        buffer.extend_from_slice(&chunk[..read]);
        if buffer.len() > 64 * 1024 {
            return Ok(());
        }
    }
    let text = String::from_utf8_lossy(&buffer).into_owned();
    let mut lines = text.split("\r\n");
    let request_line = lines.next().unwrap_or_default();
    let mut parts = request_line.split(' ');
    let method = parts.next().unwrap_or_default().to_owned();
    let path = parts.next().unwrap_or_default().to_owned();
    let authorization = lines
        .filter_map(|line| line.split_once(':'))
        .find(|(name, _)| name.eq_ignore_ascii_case("authorization"))
        .map(|(_, value)| value.trim().to_owned());
    if method.is_empty() || path.is_empty() {
        return Ok(());
    }
    let head = method == "HEAD";

    let mut guard = state.lock().unwrap_or_else(|p| p.into_inner());
    guard.requests.push(Recorded {
        method: method.clone(),
        path: path.clone(),
        authorization: authorization.clone(),
        on_cdn,
    });

    // scripted failures
    if let Some((_, queue)) = guard
        .failures
        .iter_mut()
        .find(|(needle, queue)| path.contains(needle.as_str()) && !queue.is_empty())
    {
        let status = queue.pop_front().expect("non-empty");
        drop(guard);
        return respond(
            &mut stream,
            status,
            &[("Retry-After", "0".to_owned())],
            b"",
            head,
        );
    }

    // blob "CDN": no authentication, no registry semantics
    if on_cdn {
        let digest = path.rsplit('/').next().unwrap_or_default().to_owned();
        let blob = guard.blobs.get(&digest).cloned();
        drop(guard);
        return match blob {
            Some(blob) => respond(&mut stream, 200, &[], &blob, head),
            None => respond(&mut stream, 404, &[], b"", head),
        };
    }

    // token endpoint
    if path.starts_with("/token") {
        let required = match &guard.auth {
            AuthMode::Bearer { credentials } => credentials.clone(),
            _ => {
                drop(guard);
                return respond(&mut stream, 404, &[], b"", head);
            }
        };
        drop(guard);
        if let Some((user, password)) = required {
            let expected = format!(
                "Basic {}",
                base64::engine::general_purpose::STANDARD.encode(format!("{user}:{password}"))
            );
            if authorization.as_deref() != Some(expected.as_str()) {
                return respond(
                    &mut stream,
                    401,
                    &[],
                    &error_body("UNAUTHORIZED", "authentication required"),
                    head,
                );
            }
        }
        let body =
            serde_json::to_vec(&serde_json::json!({"token": TOKEN, "expires_in": 300})).unwrap();
        return respond(
            &mut stream,
            200,
            &[("Content-Type", "application/json".to_owned())],
            &body,
            head,
        );
    }

    // registry API
    let Some(rest) = path.strip_prefix("/v2/") else {
        drop(guard);
        return respond(&mut stream, 404, &[], b"", head);
    };
    let repo_hint = rest
        .rsplit_once("/manifests/")
        .or_else(|| rest.rsplit_once("/blobs/"))
        .map(|(repo, _)| repo.to_owned())
        .unwrap_or_default();

    let challenge = match &guard.auth {
        AuthMode::None => None,
        AuthMode::Basic { username, password } => {
            let expected = format!(
                "Basic {}",
                base64::engine::general_purpose::STANDARD.encode(format!("{username}:{password}"))
            );
            (authorization.as_deref() != Some(expected.as_str()))
                .then(|| "Basic realm=\"mock\"".to_owned())
        }
        AuthMode::Bearer { .. } => (authorization.as_deref() != Some(&format!("Bearer {TOKEN}"))).then(|| {
            format!(
                "Bearer realm=\"http://{addr}/token\",service=\"mock\",scope=\"repository:{repo_hint}:pull\""
            )
        }),
    };
    if let Some(challenge) = challenge {
        drop(guard);
        return respond(
            &mut stream,
            401,
            &[("WWW-Authenticate", challenge)],
            &error_body("UNAUTHORIZED", "authentication required"),
            head,
        );
    }

    if let Some((repo, reference)) = rest.split_once("/manifests/") {
        let found = guard
            .manifests
            .get(&(repo.to_owned(), reference.to_owned()))
            .cloned();
        drop(guard);
        return match found {
            Some((content_type, body)) => respond(
                &mut stream,
                200,
                &[
                    ("Content-Type", content_type),
                    ("Docker-Content-Digest", sha256_digest(&body)),
                ],
                &body,
                head,
            ),
            None => respond(
                &mut stream,
                404,
                &[],
                &error_body("MANIFEST_UNKNOWN", "manifest unknown"),
                head,
            ),
        };
    }
    if let Some((_, digest)) = rest.split_once("/blobs/") {
        let redirect = guard.redirect_blobs;
        let blob = guard.blobs.get(digest).cloned();
        let corrupt = guard.corrupt.contains(digest);
        let stall = guard.stall.contains(digest);
        drop(guard);
        let Some(mut blob) = blob else {
            return respond(
                &mut stream,
                404,
                &[],
                &error_body("BLOB_UNKNOWN", "blob unknown"),
                head,
            );
        };
        if redirect {
            let location = format!("http://{cdn_addr}/cdn/blobs/{digest}");
            return respond(&mut stream, 307, &[("Location", location)], b"", head);
        }
        if corrupt && !blob.is_empty() {
            let last = blob.len() - 1;
            blob[last] ^= 0xff;
        }
        if stall {
            let half = blob.len() / 2;
            stream.write_all(
                format!(
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    blob.len()
                )
                .as_bytes(),
            )?;
            stream.write_all(&blob[..half])?;
            stream.flush()?;
            while !stop.load(Ordering::SeqCst) {
                std::thread::sleep(Duration::from_millis(50));
            }
            return Ok(());
        }
        return respond(&mut stream, 200, &[], &blob, head);
    }
    drop(guard);
    respond(&mut stream, 404, &[], b"", head)
}
