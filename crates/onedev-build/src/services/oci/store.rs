//! A shared on-disk cache of unpacked images.
//!
//! ```text
//! <root>/rootfs/<manifest hex>/        unpacked image (read-only for everyone)
//! <root>/rootfs/<manifest hex>.json    image configuration + bookkeeping
//! <root>/rootfs/<manifest hex>.used    mtime = last use (LRU)
//! <root>/refs/<hash>.json              tag/digest -> manifest digest resolutions
//! <root>/locks/<manifest hex>.lock     flock: shared = in use, exclusive = unpacking/pruning
//! <root>/tmp/pull-<id>/                staging area of one pull
//! ```
//!
//! Images are identified by the digest of their (single-platform) manifest, so
//! a moved tag simply resolves to a new directory. The cache is safe to share
//! between jobs, threads and server processes: pulls of one image serialise on
//! its lock file, unpacking happens in a private staging directory that is
//! renamed into place, and every image in use holds a shared lock
//! ([`ImageLease`]) that keeps [`ImageStore::prune`] away from it.

use std::fs::{self, File, OpenOptions};
use std::io;
use std::os::fd::AsRawFd;
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use futures::StreamExt as _;
use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};

use super::manifest::{
    ImageConfig, ImageManifest, ManifestDocument, Platform, layer_compression, parse_manifest,
};
use super::reference::ImageReference;
use super::registry::{RegistryClient, ResolvedImage};
use super::unpack::{UnpackLimits, Unpacker};
use super::{OciError, OciResult, digest_hex};

const LOCK_POLL: Duration = Duration::from_millis(50);
/// Staging directories without a live owner older than this are garbage.
const STALE_TMP_AGE: Duration = Duration::from_secs(3600);

/// Tunables of an [`ImageStore`].
#[derive(Debug, Clone)]
pub struct StoreOptions {
    pub limits: UnpackLimits,
    /// A tag resolved less than this long ago is not looked up again.
    pub tag_fresh_for: Duration,
    /// When the registry cannot be reached, a cached resolution at most this
    /// old is used instead.
    pub stale_if_error: Duration,
    /// Layers downloaded in parallel (they are unpacked strictly in order).
    pub concurrent_downloads: usize,
}

impl Default for StoreOptions {
    fn default() -> Self {
        Self {
            limits: UnpackLimits::default(),
            tag_fresh_for: Duration::from_secs(60),
            stale_if_error: Duration::from_secs(24 * 3600),
            concurrent_downloads: 3,
        }
    }
}

/// Progress of a pull, for the job log.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PullEvent {
    /// The image was found unpacked in the cache.
    Cached {
        digest: String,
    },
    /// The registry could not be reached; a cached resolution is used.
    UsingStale {
        reason: String,
    },
    /// Layers are about to be downloaded.
    Downloading {
        layers: usize,
        compressed_bytes: u64,
    },
    LayerDone {
        index: usize,
        total: usize,
    },
    Unpacked {
        bytes: u64,
        entries: u64,
    },
}

/// Keeps an image from being pruned while it is in use.
#[derive(Debug)]
pub struct ImageLease {
    _lock: File,
}

/// An image ready to be used as a root filesystem.
#[derive(Debug)]
pub struct PulledImage {
    pub reference: ImageReference,
    /// Digest of the single-platform manifest.
    pub manifest_digest: String,
    /// Unpacked root filesystem. Never modify it.
    pub rootfs: PathBuf,
    pub config: ImageConfig,
    pub lease: ImageLease,
}

/// What [`ImageStore::prune`] may remove.
#[derive(Debug, Clone)]
pub struct PruneOptions {
    /// Remove images not used for this long.
    pub unused_for: Duration,
    /// Afterwards, evict least recently used images until the cache is at
    /// most this big.
    pub max_total_bytes: Option<u64>,
    /// Images used within this time are never evicted for size reasons.
    pub keep_recent_for: Duration,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PruneReport {
    pub removed: Vec<String>,
    pub freed_bytes: u64,
    pub kept: usize,
    /// Candidates skipped because a job is using them right now.
    pub in_use: usize,
    pub stale_refs_removed: usize,
    pub tmp_removed: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct StoredImage {
    manifest_digest: String,
    reference: String,
    config: ImageConfig,
    size_bytes: u64,
    entries: u64,
    created_at: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct RefRecord {
    reference: String,
    platform: String,
    index_digest: Option<String>,
    manifest_digest: String,
    resolved_at: u64,
}

struct Resolution {
    manifest_digest: String,
    /// Present when the manifest was downloaded during resolution.
    resolved: Option<ResolvedImage>,
}

pub struct ImageStore {
    root: PathBuf,
    client: RegistryClient,
    options: StoreOptions,
    platform: Platform,
}

impl ImageStore {
    /// Open (and create) the cache below `root` for images of `platform`.
    pub fn open(
        root: impl Into<PathBuf>,
        client: RegistryClient,
        platform: Platform,
        options: StoreOptions,
    ) -> OciResult<Self> {
        let root = root.into();
        for dir in ["rootfs", "refs", "locks", "tmp"] {
            let path = root.join(dir);
            fs::create_dir_all(&path).map_err(|e| {
                OciError::io(
                    format!("cannot create the image cache {}", path.display()),
                    e,
                )
            })?;
        }
        // pulled images may be private: keep the cache to the server user
        fs::set_permissions(&root, fs::Permissions::from_mode(0o700))
            .map_err(|e| OciError::io(format!("cannot protect {}", root.display()), e))?;
        Ok(Self {
            root,
            client,
            options,
            platform,
        })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn platform(&self) -> &Platform {
        &self.platform
    }

    fn rootfs_dir(&self, hex: &str) -> PathBuf {
        self.root.join("rootfs").join(hex)
    }

    fn meta_path(&self, hex: &str) -> PathBuf {
        self.root.join("rootfs").join(format!("{hex}.json"))
    }

    fn used_path(&self, hex: &str) -> PathBuf {
        self.root.join("rootfs").join(format!("{hex}.used"))
    }

    fn lock_path(&self, hex: &str) -> PathBuf {
        self.root.join("locks").join(format!("{hex}.lock"))
    }

    fn is_unpacked(&self, hex: &str) -> bool {
        self.meta_path(hex).is_file() && self.rootfs_dir(hex).is_dir()
    }

    fn read_meta(&self, hex: &str) -> OciResult<StoredImage> {
        let path = self.meta_path(hex);
        let text = fs::read_to_string(&path)
            .map_err(|e| OciError::io(format!("cannot read {}", path.display()), e))?;
        serde_json::from_str(&text).map_err(|e| {
            OciError::Registry(format!(
                "the cached image record {} is corrupt: {e}",
                path.display()
            ))
        })
    }

    fn touch_used(&self, hex: &str) {
        let path = self.used_path(hex);
        if let Ok(file) = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(false)
            .open(&path)
        {
            let _ = file.set_modified(SystemTime::now());
        }
    }

    fn ref_key(&self, reference: &ImageReference) -> String {
        let text = format!("{reference}|{}", self.platform);
        hex::encode(Sha256::digest(text.as_bytes()))
    }

    fn ref_path(&self, key: &str) -> PathBuf {
        self.root.join("refs").join(format!("{key}.json"))
    }

    fn read_ref(&self, key: &str) -> Option<RefRecord> {
        let text = fs::read_to_string(self.ref_path(key)).ok()?;
        let record: RefRecord = serde_json::from_str(&text).ok()?;
        digest_hex(&record.manifest_digest).ok()?;
        Some(record)
    }

    fn write_ref(&self, key: &str, record: &RefRecord) {
        if let Ok(bytes) = serde_json::to_vec(record)
            && let Err(error) = write_atomic(&self.ref_path(key), &bytes)
        {
            tracing::warn!(%error, "cannot record the image resolution");
        }
    }

    // -- pulling ----------------------------------------------------------------

    /// Make `reference` available as an unpacked root filesystem.
    ///
    /// Dropping the returned future cancels the pull: transfers stop, the
    /// unpacking stops between entries and the staging area is removed.
    pub async fn pull(
        &self,
        reference: &ImageReference,
        events: &(dyn Fn(PullEvent) + Send + Sync),
    ) -> OciResult<PulledImage> {
        let interrupt = Arc::new(AtomicBool::new(false));
        let mut guard = InterruptOnDrop {
            flag: interrupt.clone(),
            armed: true,
        };
        let pulled = self.pull_inner(reference, interrupt, events).await;
        // finished (or failed): every task of this pull has ended already
        guard.armed = false;
        pulled
    }

    async fn pull_inner(
        &self,
        reference: &ImageReference,
        interrupt: Arc<AtomicBool>,
        events: &(dyn Fn(PullEvent) + Send + Sync),
    ) -> OciResult<PulledImage> {
        let key = self.ref_key(reference);
        let mut resolution = self.resolve(reference, &key, events).await?;
        let hex = digest_hex(&resolution.manifest_digest)?.to_owned();
        let lock_path = self.lock_path(&hex);

        let mut vanished = 0;
        loop {
            if self.is_unpacked(&hex) {
                // in use from now on: keep pruning away
                let file = lock_file(&lock_path, false).await?;
                if self.is_unpacked(&hex) {
                    events(PullEvent::Cached {
                        digest: resolution.manifest_digest.clone(),
                    });
                    return self.finish(reference, &hex, file);
                }
            } else {
                let file = open_lock_file(&lock_path).map_err(|e| {
                    OciError::io(format!("cannot open the lock {}", lock_path.display()), e)
                })?;
                if !try_flock(&file, libc::LOCK_EX)
                    .map_err(|e| OciError::io("cannot lock the image", e))?
                {
                    // somebody else is unpacking this image: wait for them
                    tokio::time::sleep(LOCK_POLL).await;
                    continue;
                }
                if !self.is_unpacked(&hex) {
                    self.install(
                        reference,
                        &hex,
                        resolution.manifest_digest.clone(),
                        resolution.resolved.take(),
                        interrupt.clone(),
                        events,
                    )
                    .await?;
                }
                self.touch_used(&hex);
                // exclusive -> shared. Not atomic: if someone slipped in between,
                // look at the state again.
                if try_flock(&file, libc::LOCK_SH)
                    .map_err(|e| OciError::io("cannot lock the image", e))?
                    && self.is_unpacked(&hex)
                {
                    return self.finish(reference, &hex, file);
                }
            }
            vanished += 1;
            if vanished > 3 {
                return Err(OciError::Registry(format!(
                    "the image {reference} keeps being removed while it is pulled; is another process pruning the cache aggressively?"
                )));
            }
        }
    }

    fn finish(&self, reference: &ImageReference, hex: &str, lock: File) -> OciResult<PulledImage> {
        let meta = self.read_meta(hex)?;
        self.touch_used(hex);
        Ok(PulledImage {
            reference: reference.clone(),
            manifest_digest: meta.manifest_digest,
            rootfs: self.rootfs_dir(hex),
            config: meta.config,
            lease: ImageLease { _lock: lock },
        })
    }

    async fn resolve(
        &self,
        reference: &ImageReference,
        key: &str,
        events: &(dyn Fn(PullEvent) + Send + Sync),
    ) -> OciResult<Resolution> {
        let cached = self.read_ref(key);
        let now = unix_now();
        let usable = |record: &RefRecord| {
            digest_hex(&record.manifest_digest).is_ok_and(|hex| self.is_unpacked(hex))
        };
        let reuse = |record: &RefRecord| Resolution {
            manifest_digest: record.manifest_digest.clone(),
            resolved: None,
        };

        // A digest never changes its meaning; a fresh tag resolution is trusted.
        if let Some(record) = &cached
            && (reference.digest.is_some()
                || (now.saturating_sub(record.resolved_at) < self.options.tag_fresh_for.as_secs()
                    && usable(record)))
        {
            return Ok(reuse(record));
        }

        let attempt: OciResult<Resolution> = async {
            if let Some(record) = &cached
                && usable(record)
                && let Some(head) = self.client.head_digest(reference).await?
                && record
                    .index_digest
                    .as_deref()
                    .unwrap_or(&record.manifest_digest)
                    == head
            {
                self.write_ref(
                    key,
                    &RefRecord {
                        resolved_at: now,
                        ..record.clone()
                    },
                );
                return Ok(reuse(record));
            }
            let resolved = self.client.resolve(reference, &self.platform).await?;
            self.write_ref(
                key,
                &RefRecord {
                    reference: reference.to_string(),
                    platform: self.platform.to_string(),
                    index_digest: resolved.index_digest.clone(),
                    manifest_digest: resolved.manifest_digest.clone(),
                    resolved_at: now,
                },
            );
            Ok(Resolution {
                manifest_digest: resolved.manifest_digest.clone(),
                resolved: Some(resolved),
            })
        }
        .await;

        match attempt {
            Err(OciError::Unavailable(reason)) => match &cached {
                Some(record)
                    if usable(record)
                        && now.saturating_sub(record.resolved_at)
                            < self.options.stale_if_error.as_secs() =>
                {
                    events(PullEvent::UsingStale { reason });
                    Ok(reuse(record))
                }
                _ => Err(OciError::Unavailable(reason)),
            },
            other => other,
        }
    }

    /// Download and unpack an image into the cache. The caller holds the
    /// exclusive lock of the image.
    async fn install(
        &self,
        reference: &ImageReference,
        hex: &str,
        manifest_digest: String,
        resolved: Option<ResolvedImage>,
        interrupt: Arc<AtomicBool>,
        events: &(dyn Fn(PullEvent) + Send + Sync),
    ) -> OciResult<()> {
        let resolved = match resolved {
            Some(resolved) => resolved,
            None => {
                let fetched = self
                    .client
                    .fetch_manifest(reference, &manifest_digest)
                    .await?;
                match parse_manifest(fetched.media_type.as_deref(), &fetched.body)? {
                    ManifestDocument::Image(manifest) => ResolvedImage {
                        index_digest: None,
                        manifest_digest: manifest_digest.clone(),
                        manifest,
                    },
                    ManifestDocument::Index(_) => {
                        return Err(OciError::Registry(
                            "expected an image manifest but the registry sent an index".to_owned(),
                        ));
                    }
                }
            }
        };
        let config = self
            .client
            .fetch_config(reference, &resolved.manifest)
            .await?;
        if (!config.os.is_empty() && config.os != self.platform.os)
            || (!config.architecture.is_empty()
                && config.architecture != self.platform.architecture)
        {
            return Err(OciError::Unsupported(format!(
                "the image is built for {}/{}, this machine needs {}",
                config.os, config.architecture, self.platform
            )));
        }

        let staging = Staging::create(&self.root.join("tmp"))?;
        let rootfs = staging.path().join("rootfs");
        fs::create_dir(&rootfs)
            .map_err(|e| OciError::io("cannot create the staging directory", e))?;
        fs::set_permissions(&rootfs, fs::Permissions::from_mode(0o755))
            .map_err(|e| OciError::io("cannot prepare the staging directory", e))?;

        let stats = self
            .download_and_unpack(
                reference,
                &resolved.manifest,
                staging.path(),
                &rootfs,
                interrupt,
                events,
            )
            .await?;

        let stored = StoredImage {
            manifest_digest,
            reference: reference.to_string(),
            config,
            size_bytes: stats.bytes,
            entries: stats.entries,
            created_at: unix_now(),
        };
        let meta_bytes = serde_json::to_vec_pretty(&stored)
            .map_err(|e| OciError::Registry(format!("cannot encode the image record: {e}")))?;

        let target = self.rootfs_dir(hex);
        if target.exists() {
            // leftovers of an interrupted earlier install (no record, so unusable)
            remove_tree(&target)
                .map_err(|e| OciError::io("cannot remove a half-installed image", e))?;
        }
        fs::rename(&rootfs, &target).map_err(|e| {
            OciError::io(
                format!("cannot install the image at {}", target.display()),
                e,
            )
        })?;
        write_atomic(&self.meta_path(hex), &meta_bytes)
            .map_err(|e| OciError::io("cannot record the image", e))?;
        self.touch_used(hex);
        events(PullEvent::Unpacked {
            bytes: stats.bytes,
            entries: stats.entries,
        });
        Ok(())
    }

    async fn download_and_unpack(
        &self,
        reference: &ImageReference,
        manifest: &ImageManifest,
        staging: &Path,
        rootfs: &Path,
        interrupt: Arc<AtomicBool>,
        events: &(dyn Fn(PullEvent) + Send + Sync),
    ) -> OciResult<super::unpack::UnpackStats> {
        let total = manifest.layers.len();
        events(PullEvent::Downloading {
            layers: total,
            compressed_bytes: manifest.layers.iter().map(|l| l.size).sum(),
        });
        let mut unpacker = Unpacker::new(rootfs, self.options.limits, interrupt);

        let client = &self.client;
        let downloads =
            futures::stream::iter(manifest.layers.iter().enumerate().map(|(index, layer)| {
                let dest = staging.join(format!("layer-{index}.blob"));
                async move {
                    // reject unsupported layer types before downloading anything
                    let compression = layer_compression(&layer.media_type)?;
                    client
                        .download_blob(reference, layer, &dest, &|_| {})
                        .await?;
                    Ok::<_, OciError>((index, dest, compression))
                }
            }))
            .buffered(self.options.concurrent_downloads.max(1));
        futures::pin_mut!(downloads);

        while let Some(downloaded) = downloads.next().await {
            let (index, path, compression) = downloaded?;
            unpacker = unpack_blocking(unpacker, path, compression).await?;
            events(PullEvent::LayerDone {
                index: index + 1,
                total,
            });
        }
        Ok(unpacker.stats())
    }

    // -- housekeeping -------------------------------------------------------------

    /// Total size of all cached images (from their records).
    pub fn total_bytes(&self) -> u64 {
        self.list_images()
            .iter()
            .map(|image| image.size_bytes)
            .sum()
    }

    fn list_images(&self) -> Vec<ImageEntry> {
        let Ok(entries) = fs::read_dir(self.root.join("rootfs")) else {
            return Vec::new();
        };
        let mut images = Vec::new();
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            let Some(hex) = name.strip_suffix(".json") else {
                continue;
            };
            if digest_hex(&format!("sha256:{hex}")).is_err() {
                continue;
            }
            let Ok(meta) = self.read_meta(hex) else {
                continue;
            };
            let used = fs::metadata(self.used_path(hex))
                .and_then(|m| m.modified())
                .ok()
                .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
                .map_or(meta.created_at, |d| d.as_secs());
            images.push(ImageEntry {
                hex: hex.to_owned(),
                size_bytes: meta.size_bytes,
                last_used: used,
            });
        }
        images
    }

    /// Remove images that are unused or beyond the size budget. Images that a
    /// job is using (shared lock) are never touched. Blocking: call it from a
    /// blocking context.
    pub fn prune(&self, options: &PruneOptions) -> OciResult<PruneReport> {
        let mut report = PruneReport::default();
        let now = unix_now();
        let mut images = self.list_images();
        images.sort_by_key(|image| image.last_used);

        let mut remaining: Vec<ImageEntry> = Vec::new();
        for image in images {
            let idle = now.saturating_sub(image.last_used);
            if idle >= options.unused_for.as_secs() {
                match self.try_remove(&image.hex)? {
                    Removal::Removed => {
                        report.removed.push(format!("sha256:{}", image.hex));
                        report.freed_bytes += image.size_bytes;
                    }
                    Removal::InUse => {
                        report.in_use += 1;
                        remaining.push(image);
                    }
                }
            } else {
                remaining.push(image);
            }
        }
        if let Some(max) = options.max_total_bytes {
            let mut total: u64 = remaining.iter().map(|i| i.size_bytes).sum();
            let mut kept = Vec::new();
            for image in remaining {
                let idle = now.saturating_sub(image.last_used);
                if total > max && idle >= options.keep_recent_for.as_secs() {
                    match self.try_remove(&image.hex)? {
                        Removal::Removed => {
                            total = total.saturating_sub(image.size_bytes);
                            report.removed.push(format!("sha256:{}", image.hex));
                            report.freed_bytes += image.size_bytes;
                            continue;
                        }
                        Removal::InUse => report.in_use += 1,
                    }
                }
                kept.push(image);
            }
            remaining = kept;
        }
        report.kept = remaining.len();
        report.stale_refs_removed = self.remove_stale_refs(options);
        report.tmp_removed = self.clean_tmp();
        Ok(report)
    }

    fn try_remove(&self, hex: &str) -> OciResult<Removal> {
        let lock_path = self.lock_path(hex);
        let file = open_lock_file(&lock_path)
            .map_err(|e| OciError::io("cannot open the image lock", e))?;
        if !try_flock(&file, libc::LOCK_EX).map_err(|e| OciError::io("cannot lock the image", e))? {
            return Ok(Removal::InUse);
        }
        // the record goes first: without it the directory is no image any more
        let _ = fs::remove_file(self.meta_path(hex));
        let _ = fs::remove_file(self.used_path(hex));
        let dir = self.rootfs_dir(hex);
        if dir.exists() {
            let trash = self
                .root
                .join("tmp")
                .join(format!("trash-{hex}-{}", uuid::Uuid::new_v4().simple()));
            let target = if fs::rename(&dir, &trash).is_ok() {
                trash
            } else {
                dir
            };
            remove_tree(&target).map_err(|e| {
                OciError::io(
                    format!("cannot remove the cached image {}", target.display()),
                    e,
                )
            })?;
        }
        Ok(Removal::Removed)
    }

    /// Resolutions of images that are gone and have not been used for a while.
    fn remove_stale_refs(&self, options: &PruneOptions) -> usize {
        let Ok(entries) = fs::read_dir(self.root.join("refs")) else {
            return 0;
        };
        let horizon = options
            .unused_for
            .max(self.options.stale_if_error)
            .as_secs();
        let now = unix_now();
        let mut removed = 0;
        for entry in entries.flatten() {
            let path = entry.path();
            let Ok(text) = fs::read_to_string(&path) else {
                continue;
            };
            let stale = match serde_json::from_str::<RefRecord>(&text) {
                Ok(record) => {
                    let image_gone = digest_hex(&record.manifest_digest)
                        .map_or(true, |hex| !self.is_unpacked(hex));
                    image_gone && now.saturating_sub(record.resolved_at) >= horizon
                }
                Err(_) => true,
            };
            if stale && fs::remove_file(&path).is_ok() {
                removed += 1;
            }
        }
        removed
    }

    /// Remove staging directories whose owner is gone (crashed pulls).
    pub fn clean_tmp(&self) -> usize {
        let Ok(entries) = fs::read_dir(self.root.join("tmp")) else {
            return 0;
        };
        let mut removed = 0;
        for entry in entries.flatten() {
            let path = entry.path();
            let lock_path = path.join(".lock");
            let abandoned = match open_lock_file_existing(&lock_path) {
                Ok(Some(file)) => try_flock(&file, libc::LOCK_EX).unwrap_or(false),
                Ok(None) | Err(_) => {
                    // no lock file: trash or a staging dir that never got one
                    fs::metadata(&path)
                        .and_then(|m| m.modified())
                        .ok()
                        .and_then(|time| time.elapsed().ok())
                        .is_some_and(|age| age >= STALE_TMP_AGE)
                }
            };
            if abandoned && remove_tree(&path).is_ok() {
                removed += 1;
            }
        }
        removed
    }
}

struct ImageEntry {
    hex: String,
    size_bytes: u64,
    last_used: u64,
}

enum Removal {
    Removed,
    InUse,
}

/// Sets the interrupt flag when the pull is abandoned (its future dropped).
struct InterruptOnDrop {
    flag: Arc<AtomicBool>,
    armed: bool,
}

impl Drop for InterruptOnDrop {
    fn drop(&mut self) {
        if self.armed {
            self.flag.store(true, Ordering::SeqCst);
        }
    }
}

/// A private working directory of one pull. Its `.lock` stays flocked while
/// the owner lives, so [`ImageStore::clean_tmp`] can tell crashed pulls apart.
struct Staging {
    path: PathBuf,
    _lock: File,
}

impl Staging {
    fn create(tmp: &Path) -> OciResult<Self> {
        let path = tmp.join(format!("pull-{}", uuid::Uuid::new_v4().simple()));
        fs::create_dir(&path).map_err(|e| OciError::io("cannot create a staging directory", e))?;
        fs::set_permissions(&path, fs::Permissions::from_mode(0o700))
            .map_err(|e| OciError::io("cannot prepare a staging directory", e))?;
        let lock = open_lock_file(&path.join(".lock"))
            .map_err(|e| OciError::io("cannot create a staging lock", e))?;
        try_flock(&lock, libc::LOCK_EX)
            .map_err(|e| OciError::io("cannot lock a staging directory", e))?;
        Ok(Self { path, _lock: lock })
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for Staging {
    fn drop(&mut self) {
        let _ = remove_tree(&self.path);
    }
}

async fn unpack_blocking(
    unpacker: Unpacker,
    path: PathBuf,
    compression: super::manifest::LayerCompression,
) -> OciResult<Unpacker> {
    tokio::task::spawn_blocking(move || {
        let mut unpacker = unpacker;
        let file = File::open(&path)
            .map_err(|e| OciError::io(format!("cannot open {}", path.display()), e))?;
        unpacker.apply_layer(file, compression)?;
        let _ = fs::remove_file(&path);
        Ok(unpacker)
    })
    .await
    .map_err(|e| OciError::Layer(format!("the unpacking task failed: {e}")))?
}

// ---------------------------------------------------------------------------
// files and locks
// ---------------------------------------------------------------------------

fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn write_atomic(path: &Path, bytes: &[u8]) -> io::Result<()> {
    use std::io::Write as _;
    let temp = path.with_extension(format!("tmp-{}", uuid::Uuid::new_v4().simple()));
    let result = (|| {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&temp)?;
        file.write_all(bytes)?;
        file.sync_all()?;
        fs::rename(&temp, path)
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temp);
    }
    result
}

/// Remove a tree that may contain read-only directories.
fn remove_tree(path: &Path) -> io::Result<()> {
    match fs::remove_dir_all(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(_) => {
            // make directories writable and retry (images may carry 0555 dirs)
            make_writable(path);
            match fs::remove_dir_all(path) {
                Ok(()) => Ok(()),
                Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
                Err(error) => Err(error),
            }
        }
    }
}

fn make_writable(path: &Path) {
    let Ok(meta) = fs::symlink_metadata(path) else {
        return;
    };
    if !meta.is_dir() {
        return;
    }
    let _ = fs::set_permissions(path, fs::Permissions::from_mode(0o700));
    if let Ok(entries) = fs::read_dir(path) {
        for entry in entries.flatten() {
            make_writable(&entry.path());
        }
    }
}

fn open_lock_file(path: &Path) -> io::Result<File> {
    OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .mode(0o600)
        .open(path)
}

fn open_lock_file_existing(path: &Path) -> io::Result<Option<File>> {
    match OpenOptions::new().read(true).write(true).open(path) {
        Ok(file) => Ok(Some(file)),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error),
    }
}

/// `flock` without blocking: `Ok(false)` when somebody else holds a
/// conflicting lock.
fn try_flock(file: &File, operation: libc::c_int) -> io::Result<bool> {
    loop {
        // SAFETY: flock(2) on a valid open file descriptor owned by `file`.
        let status = unsafe { libc::flock(file.as_raw_fd(), operation | libc::LOCK_NB) };
        if status == 0 {
            return Ok(true);
        }
        let error = io::Error::last_os_error();
        match error.raw_os_error() {
            Some(libc::EWOULDBLOCK) => return Ok(false),
            Some(libc::EINTR) => continue,
            _ => return Err(error),
        }
    }
}

/// Take a lock, waiting (without blocking a thread) until it is available.
async fn lock_file(path: &Path, exclusive: bool) -> OciResult<File> {
    let file = open_lock_file(path)
        .map_err(|e| OciError::io(format!("cannot open the lock {}", path.display()), e))?;
    let operation = if exclusive {
        libc::LOCK_EX
    } else {
        libc::LOCK_SH
    };
    loop {
        if try_flock(&file, operation).map_err(|e| OciError::io("cannot lock the image", e))? {
            return Ok(file);
        }
        tokio::time::sleep(LOCK_POLL).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::oci::registry::RegistryOptions;
    use crate::services::oci::testing::{LayerType, MockRegistry, TestImage};
    use std::sync::Mutex;

    fn store_with(registry: &MockRegistry, dir: &Path, options: StoreOptions) -> ImageStore {
        let client = RegistryClient::new(RegistryOptions {
            insecure_registries: vec![registry.host(), registry.cdn_host()],
            retry_base: Duration::from_millis(5),
            read_timeout: Duration::from_secs(5),
            max_attempts: 2,
            ..RegistryOptions::default()
        })
        .unwrap();
        ImageStore::open(dir.join("images"), client, Platform::linux_host(), options).unwrap()
    }

    fn options(fresh: u64, stale: u64) -> StoreOptions {
        StoreOptions {
            tag_fresh_for: Duration::from_secs(fresh),
            stale_if_error: Duration::from_secs(stale),
            ..StoreOptions::default()
        }
    }

    fn reference(registry: &MockRegistry, name: &str) -> ImageReference {
        ImageReference::parse(&format!("{}/{name}", registry.host())).unwrap()
    }

    fn recording() -> (
        Arc<Mutex<Vec<PullEvent>>>,
        impl Fn(PullEvent) + Send + Sync + 'static,
    ) {
        let events = Arc::new(Mutex::new(Vec::new()));
        let sink = events.clone();
        (events, move |event| sink.lock().unwrap().push(event))
    }

    fn blob_requests(registry: &MockRegistry) -> usize {
        registry
            .requests()
            .iter()
            .filter(|r| r.method == "GET" && r.path.contains("/blobs/"))
            .count()
    }

    fn entries(dir: &Path) -> Vec<String> {
        let mut names: Vec<String> = fs::read_dir(dir)
            .map(|rd| {
                rd.flatten()
                    .map(|e| e.file_name().to_string_lossy().into_owned())
                    .collect()
            })
            .unwrap_or_default();
        names.sort();
        names
    }

    #[tokio::test]
    async fn pulling_unpacks_the_image_and_exposes_its_configuration() {
        let registry = MockRegistry::start();
        TestImage::simple().push(&registry, "team/app", "1.0");
        let dir = tempfile::tempdir().unwrap();
        let store = store_with(&registry, dir.path(), options(60, 3600));
        let (events, sink) = recording();

        let pulled = store
            .pull(&reference(&registry, "team/app:1.0"), &sink)
            .await
            .unwrap();
        assert_eq!(
            fs::read_to_string(pulled.rootfs.join("app/hello.txt")).unwrap(),
            "hello from the image\n"
        );
        assert_eq!(
            fs::read_to_string(pulled.rootfs.join("etc/os-release")).unwrap(),
            "NAME=test\n"
        );
        assert!(pulled.config.env.contains(&"FROM_IMAGE=yes".to_owned()));
        assert_eq!(pulled.config.os, "linux");
        assert!(pulled.manifest_digest.starts_with("sha256:"));
        assert!(store.total_bytes() > 0);
        let mode = fs::metadata(store.root()).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o700, "the cache is private to the server user");

        let events = events.lock().unwrap().clone();
        assert!(events.contains(&PullEvent::Downloading {
            layers: 2,
            compressed_bytes: events_bytes(&events)
        }));
        assert!(events.contains(&PullEvent::LayerDone { index: 1, total: 2 }));
        assert!(events.contains(&PullEvent::LayerDone { index: 2, total: 2 }));
        assert!(
            events
                .iter()
                .any(|e| matches!(e, PullEvent::Unpacked { entries, .. } if *entries >= 4))
        );
        assert!(
            entries(&store.root().join("tmp")).is_empty(),
            "no staging leftovers"
        );
    }

    #[tokio::test]
    async fn the_runtime_defaults_of_the_image_configuration_are_exposed() {
        let registry = MockRegistry::start();
        let mut image = TestImage::simple().with_env(&["PATH=/opt/bin:/bin", "JAVA_HOME=/opt/jdk"]);
        image.working_dir = "/app".to_owned();
        image.user = "1000:1000".to_owned();
        let pushed = image.push(&registry, "team/app", "1.0");
        let dir = tempfile::tempdir().unwrap();
        let store = store_with(&registry, dir.path(), options(60, 3600));

        let pulled = store
            .pull(&reference(&registry, "team/app:1.0"), &|_| {})
            .await
            .unwrap();
        assert_eq!(
            pulled.config.env,
            vec!["PATH=/opt/bin:/bin", "JAVA_HOME=/opt/jdk"]
        );
        assert_eq!(pulled.config.working_dir, "/app");
        assert_eq!(pulled.config.user, "1000:1000");
        assert_eq!(pulled.config.diff_ids, 2);
        assert_eq!(pulled.manifest_digest, pushed.manifest_digest);
        assert!(pushed.config_digest.starts_with("sha256:"));
        assert_ne!(pushed.config_digest, pushed.manifest_digest);
    }

    fn events_bytes(events: &[PullEvent]) -> u64 {
        events
            .iter()
            .find_map(|e| match e {
                PullEvent::Downloading {
                    compressed_bytes, ..
                } => Some(*compressed_bytes),
                _ => None,
            })
            .unwrap()
    }

    #[tokio::test]
    async fn a_second_pull_is_served_from_the_cache() {
        let registry = MockRegistry::start();
        TestImage::simple().push(&registry, "team/app", "1.0");
        let dir = tempfile::tempdir().unwrap();
        let store = store_with(&registry, dir.path(), options(3600, 3600));
        let reference = reference(&registry, "team/app:1.0");
        let first = store.pull(&reference, &|_| {}).await.unwrap();
        let expected_requests = registry.requests().len();
        assert!(expected_requests > 0);

        registry.clear_requests();
        let (events, sink) = recording();
        let second = store.pull(&reference, &sink).await.unwrap();
        assert_eq!(second.rootfs, first.rootfs);
        assert!(
            registry.requests().is_empty(),
            "a fresh tag needs no registry traffic"
        );
        assert!(matches!(
            events.lock().unwrap().as_slice(),
            [PullEvent::Cached { .. }]
        ));
    }

    #[tokio::test]
    async fn stale_tags_are_revalidated_with_a_head_request_only() {
        let registry = MockRegistry::start();
        TestImage::simple().push(&registry, "team/app", "1.0");
        let dir = tempfile::tempdir().unwrap();
        let store = store_with(&registry, dir.path(), options(0, 3600));
        let reference = reference(&registry, "team/app:1.0");
        store.pull(&reference, &|_| {}).await.unwrap();

        registry.clear_requests();
        store.pull(&reference, &|_| {}).await.unwrap();
        let requests = registry.requests();
        assert_eq!(requests.len(), 1, "{requests:?}");
        assert_eq!(requests[0].method, "HEAD");
        assert_eq!(blob_requests(&registry), 0);
    }

    #[tokio::test]
    async fn a_moved_tag_resolves_to_the_new_image() {
        let registry = MockRegistry::start();
        let first = TestImage::simple()
            .with_marker("one")
            .push(&registry, "team/app", "latest");
        let dir = tempfile::tempdir().unwrap();
        let store = store_with(&registry, dir.path(), options(0, 3600));
        let reference = reference(&registry, "team/app:latest");
        let before = store.pull(&reference, &|_| {}).await.unwrap();
        assert_eq!(before.manifest_digest, first.manifest_digest);
        assert_eq!(
            fs::read_to_string(before.rootfs.join("marker")).unwrap(),
            "one"
        );

        let second = TestImage::simple()
            .with_marker("two")
            .push(&registry, "team/app", "latest");
        let after = store.pull(&reference, &|_| {}).await.unwrap();
        assert_eq!(after.manifest_digest, second.manifest_digest);
        assert_ne!(after.rootfs, before.rootfs);
        assert_eq!(
            fs::read_to_string(after.rootfs.join("marker")).unwrap(),
            "two"
        );
        assert!(
            before.rootfs.is_dir(),
            "the old image stays available to running jobs"
        );
    }

    #[tokio::test]
    async fn pinned_digests_never_ask_the_registry_again() {
        let registry = MockRegistry::start();
        let pushed = TestImage::simple().push(&registry, "team/app", "1.0");
        let dir = tempfile::tempdir().unwrap();
        let store = store_with(&registry, dir.path(), options(0, 0));
        let pinned = ImageReference::parse(&format!(
            "{}/team/app@{}",
            registry.host(),
            pushed.manifest_digest
        ))
        .unwrap();
        store.pull(&pinned, &|_| {}).await.unwrap();
        registry.clear_requests();
        store.pull(&pinned, &|_| {}).await.unwrap();
        assert!(registry.requests().is_empty());
    }

    #[tokio::test]
    async fn multi_platform_images_unpack_the_matching_variant() {
        let registry = MockRegistry::start();
        let host = Platform::linux_host();
        let mine = TestImage::simple()
            .with_marker("mine")
            .push(&registry, "team/multi", "mine");
        let other_arch = if host.architecture == "s390x" {
            "arm64"
        } else {
            "s390x"
        };
        let other = TestImage::simple()
            .with_marker("other")
            .with_platform(other_arch)
            .push(&registry, "team/multi", "other");
        registry.push_index(
            "team/multi",
            "latest",
            &[
                (other_arch, &other.manifest_digest),
                (host.architecture.as_str(), &mine.manifest_digest),
            ],
        );
        let dir = tempfile::tempdir().unwrap();
        let store = store_with(&registry, dir.path(), options(60, 60));
        let pulled = store
            .pull(&reference(&registry, "team/multi:latest"), &|_| {})
            .await
            .unwrap();
        assert_eq!(pulled.manifest_digest, mine.manifest_digest);
        assert_eq!(
            fs::read_to_string(pulled.rootfs.join("marker")).unwrap(),
            "mine"
        );
    }

    #[tokio::test]
    async fn an_unreachable_registry_falls_back_to_the_cache_within_the_limit() {
        let registry = MockRegistry::start();
        TestImage::simple().push(&registry, "team/app", "1.0");
        let dir = tempfile::tempdir().unwrap();
        let reference = reference(&registry, "team/app:1.0");

        let tolerant = store_with(&registry, dir.path(), options(0, 3600));
        tolerant.pull(&reference, &|_| {}).await.unwrap();
        registry.fail_next("/manifests/", &[503; 50]);
        let (events, sink) = recording();
        tolerant.pull(&reference, &sink).await.unwrap();
        assert!(
            events
                .lock()
                .unwrap()
                .iter()
                .any(|e| matches!(e, PullEvent::UsingStale { .. })),
            "the fallback is announced"
        );

        let strict = store_with(&registry, dir.path(), options(0, 0));
        let error = strict.pull(&reference, &|_| {}).await.unwrap_err();
        assert!(matches!(error, OciError::Unavailable(_)), "{error:?}");

        // auth and not-found problems are never papered over by the cache
        registry.clear_failures();
        let missing = self::reference(&registry, "team/ghost:1");
        assert!(tolerant.pull(&missing, &|_| {}).await.is_err());
    }

    #[tokio::test]
    async fn concurrent_pulls_of_one_image_download_it_once() {
        let registry = MockRegistry::start();
        let image = TestImage::simple();
        image.push(&registry, "team/app", "1.0");
        let dir = tempfile::tempdir().unwrap();
        let store = Arc::new(store_with(&registry, dir.path(), options(60, 60)));
        let reference = reference(&registry, "team/app:1.0");

        let results = futures::future::join_all((0..4).map(|_| {
            let store = store.clone();
            let reference = reference.clone();
            async move { store.pull(&reference, &|_| {}).await }
        }))
        .await;
        let pulled: Vec<PulledImage> = results.into_iter().map(|r| r.unwrap()).collect();
        assert!(pulled.iter().all(|p| p.rootfs == pulled[0].rootfs));
        assert_eq!(
            blob_requests(&registry),
            image.layer_count() + 1,
            "layers + config, once"
        );
    }

    #[tokio::test]
    async fn images_for_other_platforms_are_rejected() {
        let registry = MockRegistry::start();
        let host = Platform::linux_host();
        let other_arch = if host.architecture == "s390x" {
            "arm64"
        } else {
            "s390x"
        };
        TestImage::simple()
            .with_platform(other_arch)
            .push(&registry, "team/foreign", "1");
        let dir = tempfile::tempdir().unwrap();
        let store = store_with(&registry, dir.path(), options(60, 60));
        let error = store
            .pull(&reference(&registry, "team/foreign:1"), &|_| {})
            .await
            .unwrap_err();
        assert!(matches!(error, OciError::Unsupported(_)), "{error:?}");
        assert!(error.to_string().contains(other_arch), "{error}");
        assert!(entries(&store.root().join("rootfs")).is_empty());
    }

    #[tokio::test]
    async fn a_corrupted_layer_installs_nothing_and_cleans_up() {
        let registry = MockRegistry::start();
        let pushed = TestImage::simple().push(&registry, "team/app", "1.0");
        registry.corrupt_blob(&pushed.layer_digests[1]);
        let dir = tempfile::tempdir().unwrap();
        let store = store_with(&registry, dir.path(), options(60, 60));
        let error = store
            .pull(&reference(&registry, "team/app:1.0"), &|_| {})
            .await
            .unwrap_err();
        assert!(matches!(error, OciError::Digest { .. }), "{error:?}");
        assert!(
            entries(&store.root().join("rootfs")).is_empty(),
            "nothing was installed"
        );
        assert!(
            entries(&store.root().join("tmp")).is_empty(),
            "the staging area is gone"
        );

        // the failure is not sticky
        registry.restore_blob(&pushed.layer_digests[1]);
        store
            .pull(&reference(&registry, "team/app:1.0"), &|_| {})
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn abandoning_a_pull_stops_it_and_leaves_no_staging_area_behind() {
        let registry = MockRegistry::start();
        let pushed = TestImage::simple().push(&registry, "team/app", "1.0");
        registry.stall_blob(&pushed.layer_digests[0]);
        let dir = tempfile::tempdir().unwrap();
        let store = store_with(&registry, dir.path(), options(60, 60));
        let reference = reference(&registry, "team/app:1.0");

        let outcome =
            tokio::time::timeout(Duration::from_millis(600), store.pull(&reference, &|_| {})).await;
        assert!(outcome.is_err(), "the stalled download is still running");
        for _ in 0..40 {
            if entries(&store.root().join("tmp")).is_empty() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        assert!(entries(&store.root().join("tmp")).is_empty());
        assert!(entries(&store.root().join("rootfs")).is_empty());
    }

    #[tokio::test]
    async fn docker_and_uncompressed_layer_types_are_supported() {
        let registry = MockRegistry::start();
        TestImage::simple()
            .with_layer_type(LayerType::DockerGzip)
            .push(&registry, "team/docker", "1");
        TestImage::simple()
            .with_layer_type(LayerType::OciUncompressed)
            .push(&registry, "team/plain", "1");
        let dir = tempfile::tempdir().unwrap();
        let store = store_with(&registry, dir.path(), options(60, 60));
        for name in ["team/docker:1", "team/plain:1"] {
            let pulled = store
                .pull(&reference(&registry, name), &|_| {})
                .await
                .unwrap();
            assert!(pulled.rootfs.join("app/hello.txt").is_file(), "{name}");
        }
    }

    #[tokio::test]
    async fn a_half_installed_image_is_repaired_by_the_next_pull() {
        let registry = MockRegistry::start();
        TestImage::simple().push(&registry, "team/app", "1.0");
        let dir = tempfile::tempdir().unwrap();
        let store = store_with(&registry, dir.path(), options(0, 0));
        let reference = reference(&registry, "team/app:1.0");
        let pulled = store.pull(&reference, &|_| {}).await.unwrap();
        let hex = digest_hex(&pulled.manifest_digest).unwrap().to_owned();
        let rootfs = pulled.rootfs.clone();
        drop(pulled);

        // crash between "directory renamed" and "record written"
        fs::remove_file(store.meta_path(&hex)).unwrap();
        fs::write(rootfs.join("garbage"), "x").unwrap();
        let repaired = store.pull(&reference, &|_| {}).await.unwrap();
        assert!(repaired.rootfs.join("app/hello.txt").is_file());
        assert!(!repaired.rootfs.join("garbage").exists());
    }

    fn age(store: &ImageStore, digest: &str, seconds: u64) {
        let hex = digest_hex(digest).unwrap();
        let file = OpenOptions::new()
            .write(true)
            .open(store.used_path(hex))
            .unwrap();
        file.set_modified(SystemTime::now() - Duration::from_secs(seconds))
            .unwrap();
    }

    #[tokio::test]
    async fn prune_removes_unused_images_but_never_those_in_use() {
        let registry = MockRegistry::start();
        TestImage::simple()
            .with_marker("a")
            .push(&registry, "team/app", "a");
        TestImage::simple()
            .with_marker("b")
            .push(&registry, "team/app", "b");
        let dir = tempfile::tempdir().unwrap();
        let store = store_with(&registry, dir.path(), options(60, 60));
        let a = store
            .pull(&reference(&registry, "team/app:a"), &|_| {})
            .await
            .unwrap();
        let b = store
            .pull(&reference(&registry, "team/app:b"), &|_| {})
            .await
            .unwrap();
        let (digest_a, rootfs_a) = (a.manifest_digest.clone(), a.rootfs.clone());
        age(&store, &a.manifest_digest, 7200);
        age(&store, &b.manifest_digest, 7200);
        let policy = PruneOptions {
            unused_for: Duration::from_secs(3600),
            max_total_bytes: None,
            keep_recent_for: Duration::ZERO,
        };

        // `a` is leased by a running job and survives, `b` is idle and goes
        drop(b);
        let report = store.prune(&policy).unwrap();
        assert_eq!(report.in_use, 1, "{report:?}");
        assert_eq!(report.removed.len(), 1, "{report:?}");
        assert!(report.freed_bytes > 0);
        assert!(rootfs_a.is_dir());
        assert!(
            !a.rootfs
                .join("..")
                .join(digest_hex(&report.removed[0]).unwrap())
                .exists()
        );

        drop(a);
        let report = store.prune(&policy).unwrap();
        assert_eq!(report.removed, vec![digest_a]);
        assert!(!rootfs_a.exists());
        assert_eq!(store.total_bytes(), 0);
        assert!(entries(&store.root().join("tmp")).is_empty());
        // pruned images can be pulled again
        let again = store
            .pull(&reference(&registry, "team/app:a"), &|_| {})
            .await
            .unwrap();
        assert!(again.rootfs.join("marker").is_file());
    }

    #[tokio::test]
    async fn prune_evicts_the_least_recently_used_images_beyond_the_size_budget() {
        let registry = MockRegistry::start();
        let dir = tempfile::tempdir().unwrap();
        let store = store_with(&registry, dir.path(), options(60, 60));
        let mut digests = Vec::new();
        for (index, name) in ["one", "two", "three"].iter().enumerate() {
            TestImage::simple()
                .with_marker(name)
                .push(&registry, "team/app", name);
            let pulled = store
                .pull(&reference(&registry, &format!("team/app:{name}")), &|_| {})
                .await
                .unwrap();
            age(
                &store,
                &pulled.manifest_digest,
                100 * (3 - index as u64) + 10,
            );
            digests.push(pulled.manifest_digest.clone());
        }
        let per_image = store.total_bytes() / 3;
        let policy = PruneOptions {
            unused_for: Duration::from_secs(86_400),
            max_total_bytes: Some(per_image * 2 + per_image / 2),
            keep_recent_for: Duration::ZERO,
        };
        let report = store.prune(&policy).unwrap();
        assert_eq!(
            report.removed,
            vec![digests[0].clone()],
            "only the oldest goes: {report:?}"
        );
        assert_eq!(report.kept, 2);

        // recent images are protected from size eviction
        let strict = PruneOptions {
            unused_for: Duration::from_secs(86_400),
            max_total_bytes: Some(0),
            keep_recent_for: Duration::from_secs(3600),
        };
        let report = store.prune(&strict).unwrap();
        assert!(report.removed.is_empty(), "{report:?}");
    }

    #[tokio::test]
    async fn abandoned_staging_directories_are_cleaned_up_but_live_ones_are_kept() {
        let registry = MockRegistry::start();
        let dir = tempfile::tempdir().unwrap();
        let store = store_with(&registry, dir.path(), options(60, 60));
        let tmp = store.root().join("tmp");

        let dead = tmp.join("pull-dead");
        fs::create_dir_all(dead.join("rootfs/deep")).unwrap();
        File::create(dead.join(".lock")).unwrap();
        let live = Staging::create(&tmp).unwrap();
        let live_path = live.path().to_owned();
        let trash = tmp.join("trash-old");
        fs::create_dir(&trash).unwrap();
        fs::write(trash.join("f"), "x").unwrap();
        fs::set_permissions(&trash, fs::Permissions::from_mode(0o500)).unwrap();
        File::open(&trash)
            .unwrap()
            .set_modified(SystemTime::now() - Duration::from_secs(7200))
            .unwrap();

        let removed = store.clean_tmp();
        assert_eq!(removed, 2, "{:?}", entries(&tmp));
        assert!(!dead.exists());
        assert!(!trash.exists());
        assert!(live_path.exists(), "a pull in progress is left alone");
        drop(live);
        assert!(!live_path.exists(), "the staging area removes itself");
    }

    #[tokio::test]
    async fn stale_resolutions_of_vanished_images_are_forgotten() {
        let registry = MockRegistry::start();
        TestImage::simple().push(&registry, "team/app", "1.0");
        let dir = tempfile::tempdir().unwrap();
        let store = store_with(&registry, dir.path(), options(0, 0));
        let pulled = store
            .pull(&reference(&registry, "team/app:1.0"), &|_| {})
            .await
            .unwrap();
        age(&store, &pulled.manifest_digest, 100_000);
        drop(pulled);
        let report = store
            .prune(&PruneOptions {
                unused_for: Duration::from_secs(10),
                max_total_bytes: None,
                keep_recent_for: Duration::ZERO,
            })
            .unwrap();
        assert_eq!(report.removed.len(), 1);
        assert_eq!(
            report.stale_refs_removed, 0,
            "the resolution is still recent"
        );
        let key = store.ref_key(&reference(&registry, "team/app:1.0"));
        let record = store.read_ref(&key).unwrap();
        let mut old = record.clone();
        old.resolved_at = 1;
        store.write_ref(&key, &old);
        let report = store
            .prune(&PruneOptions {
                unused_for: Duration::from_secs(10),
                max_total_bytes: None,
                keep_recent_for: Duration::ZERO,
            })
            .unwrap();
        assert_eq!(report.stale_refs_removed, 1);
        assert!(entries(&store.root().join("refs")).is_empty());
    }
}
