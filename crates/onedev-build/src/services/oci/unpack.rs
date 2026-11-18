//! Applying image layers to a directory.
//!
//! The unpacker works without privileges and treats every layer as hostile:
//!
//! * entry paths are normalised and may never contain `..`; every parent
//!   directory is resolved *inside* the target root, following symlinks with
//!   chroot semantics (an absolute link target means "relative to the image
//!   root"), so no entry can end up outside it,
//! * an existing entry is replaced, never written through (no writes via
//!   symlinks, files are created with `O_EXCL|O_NOFOLLOW`),
//! * OCI whiteouts (`.wh.<name>`) and opaque directories (`.wh..wh..opq`) are
//!   applied to what lower layers left behind, but never to entries of the
//!   same layer,
//! * hard links are resolved inside the root and may not point at
//!   directories,
//! * ownership and special bits that need privileges are ignored (device
//!   nodes are skipped), directories stay traversable by the unpacking user
//!   and files readable by it,
//! * the number of entries, the unpacked size and the decompressed stream are
//!   bounded, and an interrupt flag is honoured between entries and chunks.

use std::collections::{HashSet, VecDeque};
use std::ffi::{CString, OsStr, OsString};
use std::fs::{self, OpenOptions};
use std::io::{self, BufReader, Read, Write as _};
use std::os::unix::ffi::{OsStrExt, OsStringExt};
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use super::manifest::LayerCompression;
use super::{OciError, OciResult};

/// Symlinks followed while resolving one path (Linux uses 40).
const MAX_SYMLINK_HOPS: u32 = 40;
const COPY_CHUNK: usize = 128 * 1024;
const WHITEOUT_PREFIX: &[u8] = b".wh.";
const OPAQUE_MARKER: &[u8] = b".wh..wh..opq";

/// Upper bounds for everything unpacked into one root.
#[derive(Debug, Clone, Copy)]
pub struct UnpackLimits {
    /// Sum of the sizes of all regular files.
    pub max_bytes: u64,
    /// Number of archive entries over all layers.
    pub max_entries: u64,
}

impl Default for UnpackLimits {
    fn default() -> Self {
        Self {
            max_bytes: 16 * 1024 * 1024 * 1024,
            max_entries: 2_000_000,
        }
    }
}

/// What was unpacked.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct UnpackStats {
    pub bytes: u64,
    pub entries: u64,
    /// Device nodes that cannot be created without privileges.
    pub skipped_devices: u64,
}

/// Applies layers, in order, to one root directory.
pub struct Unpacker {
    root: PathBuf,
    limits: UnpackLimits,
    stats: UnpackStats,
    cancel: Arc<AtomicBool>,
}

impl Unpacker {
    /// `root` must exist and be an empty (or already partially unpacked)
    /// directory owned by the caller.
    pub fn new(root: impl Into<PathBuf>, limits: UnpackLimits, cancel: Arc<AtomicBool>) -> Self {
        Self {
            root: root.into(),
            limits,
            stats: UnpackStats::default(),
            cancel,
        }
    }

    pub fn stats(&self) -> UnpackStats {
        self.stats
    }

    /// Apply one layer given as a (possibly compressed) tar stream.
    pub fn apply_layer<'r, R: Read + 'r>(
        &mut self,
        reader: R,
        compression: LayerCompression,
    ) -> OciResult<()> {
        // the decompressed stream carries tar framing on top of the file data
        let raw_cap = self
            .limits
            .max_bytes
            .saturating_add(self.limits.max_entries.saturating_mul(2048))
            .saturating_add(64 * 1024 * 1024);
        let limit_hit = Arc::new(AtomicBool::new(false));
        let decoded: Box<dyn Read + 'r> =
            match compression {
                LayerCompression::None => Box::new(BufReader::with_capacity(COPY_CHUNK, reader)),
                LayerCompression::Gzip => Box::new(flate2::read::MultiGzDecoder::new(reader)),
                LayerCompression::Zstd => Box::new(zstd::Decoder::new(reader).map_err(|e| {
                    OciError::Layer(format!("cannot start zstd decompression: {e}"))
                })?),
            };
        let limited = CappedReader {
            inner: decoded,
            remaining: raw_cap,
            hit: limit_hit.clone(),
        };
        let mut archive = tar::Archive::new(limited);
        archive.set_unpack_xattrs(false);
        archive.set_preserve_permissions(false);
        archive.set_preserve_mtime(false);

        let mut created: HashSet<PathBuf> = HashSet::new();
        let entries = archive
            .entries()
            .map_err(|e| layer_error(&limit_hit, None, e))?;
        for entry in entries {
            if self.cancel.load(Ordering::Relaxed) {
                return Err(OciError::Interrupted);
            }
            let mut entry = entry.map_err(|e| layer_error(&limit_hit, None, e))?;
            let path_bytes = entry.path_bytes().into_owned();
            let outcome = self.apply_entry(&mut entry, &path_bytes, &mut created);
            match outcome {
                Ok(()) => {}
                Err(EntryError::Oci(error)) => return Err(error),
                Err(EntryError::Io(error)) => {
                    return Err(layer_error(&limit_hit, Some(&path_bytes), error));
                }
            }
        }
        Ok(())
    }

    fn count_entry(&mut self) -> Result<(), EntryError> {
        self.stats.entries += 1;
        if self.stats.entries > self.limits.max_entries {
            return Err(EntryError::Oci(OciError::Limit(format!(
                "the image has more than {} files",
                self.limits.max_entries
            ))));
        }
        Ok(())
    }

    fn apply_entry<R: Read>(
        &mut self,
        entry: &mut tar::Entry<'_, R>,
        path_bytes: &[u8],
        created: &mut HashSet<PathBuf>,
    ) -> Result<(), EntryError> {
        self.count_entry()?;
        let components = split_path(path_bytes)?;
        let Some((last, parents)) = components.split_last() else {
            // the archive root itself (`./`): nothing to do
            return Ok(());
        };
        let kind = entry.header().entry_type();
        let mode = entry.header().mode().unwrap_or(0o644) & 0o7777;
        let mtime = entry.header().mtime().unwrap_or(0);

        // -- whiteouts ----------------------------------------------------------
        if last.as_bytes().starts_with(WHITEOUT_PREFIX) {
            return self.apply_whiteout(parents, last, created);
        }

        // regular entries
        use tar::EntryType as T;
        match kind {
            T::XGlobalHeader | T::XHeader | T::GNULongName | T::GNULongLink => Ok(()),
            T::Char | T::Block => {
                self.stats.skipped_devices += 1;
                Ok(())
            }
            T::GNUSparse => Err(EntryError::Oci(OciError::Unsupported(
                "layers with sparse file entries".to_owned(),
            ))),
            T::Directory => {
                let parent = self.resolve_dir(parents, true)?.expect("created");
                let target = parent.join(last);
                match fs::symlink_metadata(&target) {
                    Ok(meta) if meta.is_dir() => {}
                    Ok(_) => {
                        remove_existing(&target)?;
                        fs::create_dir(&target)?;
                    }
                    Err(error) if error.kind() == io::ErrorKind::NotFound => {
                        fs::create_dir(&target)?
                    }
                    Err(error) => return Err(error.into()),
                }
                // stay able to traverse, fill and delete the tree
                fs::set_permissions(&target, fs::Permissions::from_mode(mode | 0o700))?;
                created.insert(target);
                Ok(())
            }
            T::Symlink => {
                let link = entry
                    .link_name_bytes()
                    .filter(|l| !l.is_empty())
                    .ok_or_else(|| layer_err("symlink entry without a target"))?
                    .into_owned();
                let parent = self.resolve_dir(parents, true)?.expect("created");
                let target = parent.join(last);
                remove_existing(&target)?;
                std::os::unix::fs::symlink(OsStr::from_bytes(&link), &target)?;
                set_mtime(&target, mtime, false);
                created.insert(target);
                Ok(())
            }
            T::Link => {
                let link = entry
                    .link_name_bytes()
                    .filter(|l| !l.is_empty())
                    .ok_or_else(|| layer_err("hard link entry without a target"))?
                    .into_owned();
                let source_components = split_path(&link)?;
                let Some((source_last, source_parents)) = source_components.split_last() else {
                    return Err(layer_err("hard link to the image root"));
                };
                let source_parent = self.resolve_dir(source_parents, false)?.ok_or_else(|| {
                    layer_err(format!(
                        "hard link target {} does not exist",
                        String::from_utf8_lossy(&link)
                    ))
                })?;
                let source = source_parent.join(source_last);
                match fs::symlink_metadata(&source) {
                    Ok(meta) if meta.is_dir() => {
                        return Err(layer_err("hard link to a directory"));
                    }
                    Ok(_) => {}
                    Err(error) if error.kind() == io::ErrorKind::NotFound => {
                        return Err(layer_err(format!(
                            "hard link target {} does not exist",
                            String::from_utf8_lossy(&link)
                        )));
                    }
                    Err(error) => return Err(error.into()),
                }
                let parent = self.resolve_dir(parents, true)?.expect("created");
                let target = parent.join(last);
                remove_existing(&target)?;
                fs::hard_link(&source, &target)?;
                created.insert(target);
                Ok(())
            }
            T::Fifo => {
                let parent = self.resolve_dir(parents, true)?.expect("created");
                let target = parent.join(last);
                remove_existing(&target)?;
                make_fifo(&target, mode)?;
                set_mtime(&target, mtime, false);
                created.insert(target);
                Ok(())
            }
            T::Regular | T::Continuous => {
                let size = entry.size();
                if self.stats.bytes.saturating_add(size) > self.limits.max_bytes {
                    return Err(EntryError::Oci(OciError::Limit(format!(
                        "the unpacked image is larger than {} bytes",
                        self.limits.max_bytes
                    ))));
                }
                let parent = self.resolve_dir(parents, true)?.expect("created");
                let target = parent.join(last);
                remove_existing(&target)?;
                let mut file = OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .custom_flags(libc::O_NOFOLLOW)
                    .mode(0o600)
                    .open(&target)?;
                let mut remaining = size;
                let mut buffer = vec![0u8; COPY_CHUNK.min(size.max(1) as usize)];
                while remaining > 0 {
                    if self.cancel.load(Ordering::Relaxed) {
                        return Err(EntryError::Oci(OciError::Interrupted));
                    }
                    let want = buffer.len().min(remaining as usize);
                    let read = entry.read(&mut buffer[..want])?;
                    if read == 0 {
                        return Err(layer_err("the layer ended inside a file"));
                    }
                    file.write_all(&buffer[..read])?;
                    remaining -= read as u64;
                }
                self.stats.bytes += size;
                // owner-readable at least: copies of the tree run unprivileged
                file.set_permissions(fs::Permissions::from_mode(mode | 0o400))?;
                drop(file);
                set_mtime(&target, mtime, true);
                created.insert(target);
                Ok(())
            }
            other => {
                tracing::debug!(entry_type = ?other, "skipping an archive entry of an unknown type");
                Ok(())
            }
        }
    }

    fn apply_whiteout(
        &mut self,
        parents: &[OsString],
        last: &OsStr,
        created: &mut HashSet<PathBuf>,
    ) -> Result<(), EntryError> {
        if last.as_bytes() == OPAQUE_MARKER {
            // hide everything lower layers put into the directory
            let directory = self.resolve_dir(parents, true)?.expect("created");
            for child in fs::read_dir(&directory)? {
                let child = child?.path();
                if !created.contains(&child) {
                    remove_existing(&child)?;
                }
            }
            return Ok(());
        }
        let name = &last.as_bytes()[WHITEOUT_PREFIX.len()..];
        if name.is_empty() || name == b"." || name == b".." || name.contains(&0) {
            return Err(layer_err("whiteout with an invalid name"));
        }
        let Some(directory) = self.resolve_dir(parents, false)? else {
            return Ok(()); // nothing there to hide
        };
        let victim = directory.join(OsStr::from_bytes(name));
        if !created.contains(&victim) {
            remove_existing(&victim)?;
        }
        Ok(())
    }

    /// The real path of the directory `components` below the root, with all
    /// symlinks resolved as if the root were `/`. Missing directories are
    /// created when `create` is set, otherwise `Ok(None)` is returned.
    fn resolve_dir(
        &self,
        components: &[OsString],
        create: bool,
    ) -> Result<Option<PathBuf>, EntryError> {
        let mut queue: VecDeque<OsString> = components.iter().cloned().collect();
        let mut stack: Vec<OsString> = Vec::new();
        let mut hops = 0u32;
        while let Some(component) = queue.pop_front() {
            if component == ".." {
                stack.pop();
                continue;
            }
            if component == "." || component.is_empty() {
                continue;
            }
            let mut candidate = self.root.clone();
            candidate.extend(&stack);
            candidate.push(&component);
            match fs::symlink_metadata(&candidate) {
                Ok(meta) if meta.file_type().is_symlink() => {
                    hops += 1;
                    if hops > MAX_SYMLINK_HOPS {
                        return Err(layer_err("too many levels of symbolic links"));
                    }
                    let link = fs::read_link(&candidate)?;
                    let bytes = link.as_os_str().as_bytes();
                    if bytes.starts_with(b"/") {
                        stack.clear();
                    }
                    let parts: Vec<OsString> = bytes
                        .split(|b| *b == b'/')
                        .filter(|part| !part.is_empty())
                        .map(|part| OsString::from_vec(part.to_vec()))
                        .collect();
                    for part in parts.into_iter().rev() {
                        queue.push_front(part);
                    }
                }
                Ok(meta) if meta.is_dir() => stack.push(component),
                Ok(_) => {
                    return Err(layer_err(format!(
                        "{} is not a directory",
                        Path::new(&component).display()
                    )));
                }
                Err(error) if error.kind() == io::ErrorKind::NotFound => {
                    if !create {
                        return Ok(None);
                    }
                    fs::create_dir(&candidate)?;
                    fs::set_permissions(&candidate, fs::Permissions::from_mode(0o755))?;
                    stack.push(component);
                }
                Err(error) => return Err(error.into()),
            }
        }
        let mut resolved = self.root.clone();
        resolved.extend(&stack);
        Ok(Some(resolved))
    }
}

// ---------------------------------------------------------------------------
// helpers
// ---------------------------------------------------------------------------

enum EntryError {
    Oci(OciError),
    Io(io::Error),
}

impl From<io::Error> for EntryError {
    fn from(error: io::Error) -> Self {
        EntryError::Io(error)
    }
}

fn layer_err(message: impl Into<String>) -> EntryError {
    EntryError::Oci(OciError::Layer(message.into()))
}

fn layer_error(limit_hit: &AtomicBool, path: Option<&[u8]>, error: io::Error) -> OciError {
    if limit_hit.load(Ordering::Relaxed) {
        return OciError::Limit(
            "the decompressed layer is larger than the allowed size".to_owned(),
        );
    }
    match path {
        Some(path) => OciError::Layer(format!("{}: {error}", String::from_utf8_lossy(path))),
        None => OciError::Layer(error.to_string()),
    }
}

/// Split an archive path into components. Leading `/`, `./` and empty parts
/// are dropped; `..` is an error.
fn split_path(raw: &[u8]) -> Result<Vec<OsString>, EntryError> {
    let mut components = Vec::new();
    for part in raw.split(|b| *b == b'/') {
        match part {
            b"" | b"." => {}
            b".." => {
                return Err(layer_err(format!(
                    "the path {} escapes the image root",
                    String::from_utf8_lossy(raw)
                )));
            }
            part if part.contains(&0) => return Err(layer_err("path with a NUL byte")),
            part => components.push(OsString::from_vec(part.to_vec())),
        }
    }
    Ok(components)
}

fn remove_existing(path: &Path) -> io::Result<()> {
    match fs::symlink_metadata(path) {
        Ok(meta) if meta.is_dir() => fs::remove_dir_all(path),
        Ok(_) => fs::remove_file(path),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
    }
}

fn make_fifo(path: &Path, mode: u32) -> io::Result<()> {
    let c_path = CString::new(path.as_os_str().as_bytes())
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "path with a NUL byte"))?;
    // SAFETY: `c_path` is a valid NUL-terminated string that outlives the call.
    let status = unsafe { libc::mkfifo(c_path.as_ptr(), (mode | 0o600) as libc::mode_t) };
    if status != 0 {
        return Err(io::Error::last_os_error());
    }
    fs::set_permissions(path, fs::Permissions::from_mode(mode | 0o600))
}

/// Best effort: timestamps are a convenience, not a correctness matter.
fn set_mtime(path: &Path, mtime: u64, follow: bool) {
    let Ok(seconds) = libc::time_t::try_from(mtime) else {
        return;
    };
    let Ok(c_path) = CString::new(path.as_os_str().as_bytes()) else {
        return;
    };
    let stamp = libc::timespec {
        tv_sec: seconds,
        tv_nsec: 0,
    };
    let times = [stamp, stamp];
    let flags = if follow { 0 } else { libc::AT_SYMLINK_NOFOLLOW };
    // SAFETY: `c_path` is NUL-terminated and `times` holds the two timespecs
    // utimensat(2) reads; both outlive the call.
    unsafe {
        libc::utimensat(libc::AT_FDCWD, c_path.as_ptr(), times.as_ptr(), flags);
    }
}

/// Fails once more than `remaining` bytes were read (decompression bombs).
struct CappedReader<'r> {
    inner: Box<dyn Read + 'r>,
    remaining: u64,
    hit: Arc<AtomicBool>,
}

impl Read for CappedReader<'_> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let read = self.inner.read(buf)?;
        if read as u64 > self.remaining {
            self.hit.store(true, Ordering::Relaxed);
            return Err(io::Error::other("decompressed size limit exceeded"));
        }
        self.remaining -= read as u64;
        Ok(read)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::oci::testing::{TarEntry, tar_bytes};
    use std::os::unix::fs::MetadataExt;

    fn unpack_layers(layers: &[Vec<u8>]) -> (tempfile::TempDir, OciResult<UnpackStats>) {
        unpack_with(layers, UnpackLimits::default())
    }

    fn unpack_with(
        layers: &[Vec<u8>],
        limits: UnpackLimits,
    ) -> (tempfile::TempDir, OciResult<UnpackStats>) {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("root");
        fs::create_dir(&root).unwrap();
        let mut unpacker = Unpacker::new(&root, limits, Arc::new(AtomicBool::new(false)));
        let mut result = Ok(());
        for layer in layers {
            result = unpacker.apply_layer(layer.as_slice(), LayerCompression::None);
            if result.is_err() {
                break;
            }
        }
        (dir, result.map(|()| unpacker.stats()))
    }

    fn root(dir: &tempfile::TempDir) -> PathBuf {
        dir.path().join("root")
    }

    fn read(dir: &tempfile::TempDir, relative: &str) -> String {
        fs::read_to_string(root(dir).join(relative)).unwrap_or_else(|e| panic!("{relative}: {e}"))
    }

    fn exists(dir: &tempfile::TempDir, relative: &str) -> bool {
        fs::symlink_metadata(root(dir).join(relative)).is_ok()
    }

    #[test]
    fn files_directories_and_symlinks_are_created() {
        let layer = tar_bytes(&[
            TarEntry::dir("./", 0o755),
            TarEntry::dir("etc/", 0o755),
            TarEntry::file("etc/hostname", 0o644, "builder\n"),
            TarEntry::file("bin/tool", 0o755, "#!/bin/sh\n"),
            TarEntry::symlink("bin/sh", "tool"),
            TarEntry::symlink("lib64", "/lib"),
        ]);
        let (dir, result) = unpack_layers(&[layer]);
        let stats = result.unwrap();
        assert_eq!(
            stats.bytes,
            "builder\n".len() as u64 + "#!/bin/sh\n".len() as u64
        );
        assert_eq!(read(&dir, "etc/hostname"), "builder\n");
        assert_eq!(
            fs::metadata(root(&dir).join("bin/tool"))
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o755
        );
        assert_eq!(
            fs::read_link(root(&dir).join("bin/sh")).unwrap(),
            PathBuf::from("tool")
        );
        assert_eq!(
            fs::read_link(root(&dir).join("lib64")).unwrap(),
            PathBuf::from("/lib"),
            "link targets are stored verbatim"
        );
        // missing parent directories are created
        let (dir, result) =
            unpack_layers(&[tar_bytes(&[TarEntry::file("a/b/c/file", 0o600, "x")])]);
        result.unwrap();
        assert_eq!(read(&dir, "a/b/c/file"), "x");
    }

    #[test]
    fn modes_stay_usable_for_the_unpacking_user() {
        let layer = tar_bytes(&[
            TarEntry::dir("locked/", 0o000),
            TarEntry::file("locked/secret", 0o000, "s"),
            TarEntry::file("readonly", 0o444, "r"),
            TarEntry::file("setuid", 0o4755, "x"),
        ]);
        let (dir, result) = unpack_layers(&[layer]);
        result.unwrap();
        let mode = |p: &str| {
            fs::symlink_metadata(root(&dir).join(p))
                .unwrap()
                .permissions()
                .mode()
                & 0o7777
        };
        assert_eq!(
            mode("locked") & 0o700,
            0o700,
            "directories stay traversable"
        );
        assert_eq!(
            mode("locked/secret") & 0o400,
            0o400,
            "files stay readable by the owner"
        );
        assert_eq!(mode("readonly"), 0o444);
        assert_eq!(mode("setuid"), 0o4755, "special bits are kept");
        // the whole tree can be removed again
        fs::remove_dir_all(root(&dir)).unwrap();
    }

    #[test]
    fn later_layers_replace_earlier_entries() {
        let base = tar_bytes(&[
            TarEntry::file("etc/conf", 0o644, "old"),
            TarEntry::file("etc/keep", 0o644, "keep"),
            TarEntry::dir("opt/app/", 0o755),
            TarEntry::file("opt/app/file", 0o644, "x"),
            TarEntry::symlink("link", "etc/conf"),
        ]);
        let upper = tar_bytes(&[
            TarEntry::file("etc/conf", 0o644, "new"),
            // a file becomes a directory and vice versa
            TarEntry::file("opt/app", 0o644, "now a file"),
            TarEntry::file("link", 0o644, "link replaced by a file"),
        ]);
        let (dir, result) = unpack_layers(&[base, upper]);
        result.unwrap();
        assert_eq!(read(&dir, "etc/conf"), "new");
        assert_eq!(read(&dir, "etc/keep"), "keep");
        assert_eq!(read(&dir, "opt/app"), "now a file");
        assert!(
            root(&dir)
                .join("link")
                .symlink_metadata()
                .unwrap()
                .is_file()
        );
        assert_eq!(read(&dir, "link"), "link replaced by a file");
        assert_eq!(
            read(&dir, "etc/conf"),
            "new",
            "the symlink did not redirect the write"
        );
    }

    #[test]
    fn writing_through_a_symlink_replaces_it_instead() {
        let base = tar_bytes(&[
            TarEntry::file("target", 0o644, "precious"),
            TarEntry::symlink("alias", "target"),
        ]);
        let upper = tar_bytes(&[TarEntry::file("alias", 0o644, "overwritten")]);
        let (dir, result) = unpack_layers(&[base, upper]);
        result.unwrap();
        assert_eq!(read(&dir, "target"), "precious");
        assert_eq!(read(&dir, "alias"), "overwritten");
    }

    #[test]
    fn whiteouts_remove_lower_entries_but_not_entries_of_the_same_layer() {
        let base = tar_bytes(&[
            TarEntry::file("a/gone", 0o644, "x"),
            TarEntry::file("a/stays", 0o644, "x"),
            TarEntry::file("tree/deep/file", 0o644, "x"),
            TarEntry::file("opaque/old1", 0o644, "x"),
            TarEntry::file("opaque/old2", 0o644, "x"),
            TarEntry::file("opaque/sub/old3", 0o644, "x"),
        ]);
        let upper = tar_bytes(&[
            TarEntry::file("a/.wh.gone", 0o000, ""),
            TarEntry::file(".wh.tree", 0o000, ""),
            TarEntry::file("a/.wh.never-existed", 0o000, ""),
            TarEntry::file("missing-dir/.wh.x", 0o000, ""),
            TarEntry::file("opaque/.wh..wh..opq", 0o000, ""),
            TarEntry::file("opaque/fresh", 0o644, "fresh"),
            // same layer: a whiteout must not delete what this layer created
            TarEntry::file("same/created", 0o644, "kept"),
            TarEntry::file("same/.wh.created", 0o000, ""),
        ]);
        let (dir, result) = unpack_layers(&[base, upper]);
        result.unwrap();
        assert!(!exists(&dir, "a/gone"));
        assert!(
            !exists(&dir, "a/.wh.gone"),
            "whiteout markers are not materialised"
        );
        assert!(exists(&dir, "a/stays"));
        assert!(!exists(&dir, "tree"));
        assert!(
            !exists(&dir, "opaque/old1")
                && !exists(&dir, "opaque/old2")
                && !exists(&dir, "opaque/sub")
        );
        assert_eq!(read(&dir, "opaque/fresh"), "fresh");
        assert!(!exists(&dir, "opaque/.wh..wh..opq"));
        assert_eq!(read(&dir, "same/created"), "kept");
    }

    #[test]
    fn an_opaque_marker_after_the_directory_content_keeps_the_new_content() {
        let base = tar_bytes(&[TarEntry::file("d/old", 0o644, "x")]);
        let upper = tar_bytes(&[
            TarEntry::file("d/new", 0o644, "n"),
            TarEntry::file("d/.wh..wh..opq", 0o000, ""),
        ]);
        let (dir, result) = unpack_layers(&[base, upper]);
        result.unwrap();
        assert!(!exists(&dir, "d/old"));
        assert_eq!(read(&dir, "d/new"), "n");
    }

    #[test]
    fn hard_links_share_content_and_are_confined_to_the_root() {
        let layer = tar_bytes(&[
            TarEntry::file("bin/busybox", 0o755, "bb"),
            TarEntry::hardlink("bin/ls", "bin/busybox"),
            TarEntry::hardlink("bin/cat", "./bin/busybox"),
        ]);
        let (dir, result) = unpack_layers(&[layer]);
        result.unwrap();
        let a = fs::metadata(root(&dir).join("bin/busybox")).unwrap();
        let b = fs::metadata(root(&dir).join("bin/ls")).unwrap();
        assert_eq!(a.ino(), b.ino());
        assert_eq!(a.nlink(), 3);

        let escape = tar_bytes(&[TarEntry::raw_hardlink("x", "../../etc/passwd")]);
        let (_dir, result) = unpack_layers(&[escape]);
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("escapes the image root")
        );

        let missing = tar_bytes(&[TarEntry::hardlink("x", "does/not/exist")]);
        let (_dir, result) = unpack_layers(&[missing]);
        assert!(result.unwrap_err().to_string().contains("does not exist"));

        let to_dir = tar_bytes(&[TarEntry::dir("d/", 0o755), TarEntry::hardlink("l", "d")]);
        let (_dir, result) = unpack_layers(&[to_dir]);
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("hard link to a directory")
        );
    }

    #[test]
    fn path_traversal_is_rejected() {
        for bad in [
            "../escape",
            "a/../../escape",
            "/../escape",
            "a/b/../../../x",
        ] {
            let layer = tar_bytes(&[TarEntry::raw_file(bad, "payload")]);
            let (dir, result) = unpack_layers(&[layer]);
            let error = result.unwrap_err().to_string();
            assert!(error.contains("escapes the image root"), "{bad}: {error}");
            assert!(
                !dir.path().join("escape").exists(),
                "{bad} did not write outside of the root"
            );
        }
    }

    #[test]
    fn absolute_paths_and_dot_segments_stay_inside_the_root() {
        let layer = tar_bytes(&[
            TarEntry::raw_file("/abs/file", "a"),
            TarEntry::raw_file("./dot/./file", "b"),
            TarEntry::raw_file("//double//slash", "c"),
        ]);
        let (dir, result) = unpack_layers(&[layer]);
        result.unwrap();
        assert_eq!(read(&dir, "abs/file"), "a");
        assert_eq!(read(&dir, "dot/file"), "b");
        assert_eq!(read(&dir, "double/slash"), "c");
    }

    #[test]
    fn symlinks_cannot_be_used_to_escape_the_root() {
        // an absolute link means "inside the image root", never the host
        let layer = tar_bytes(&[
            TarEntry::symlink("escape", "/etc"),
            TarEntry::file("escape/passwd-copy", 0o644, "inside"),
        ]);
        let (dir, result) = unpack_layers(&[layer]);
        result.unwrap();
        assert_eq!(read(&dir, "etc/passwd-copy"), "inside");

        // relative links that climb out are clamped at the root as well
        let layer = tar_bytes(&[
            TarEntry::dir("a/b/", 0o755),
            TarEntry::symlink("a/b/up", "../../../../../../"),
            TarEntry::file("a/b/up/clamped", 0o644, "inside"),
        ]);
        let (dir, result) = unpack_layers(&[layer]);
        result.unwrap();
        assert_eq!(read(&dir, "clamped"), "inside");
        assert!(!dir.path().join("clamped").exists());
    }

    #[test]
    fn symlink_loops_and_files_as_parents_are_errors() {
        let layer = tar_bytes(&[
            TarEntry::symlink("a", "b"),
            TarEntry::symlink("b", "a"),
            TarEntry::file("a/file", 0o644, "x"),
        ]);
        let (_dir, result) = unpack_layers(&[layer]);
        assert!(result.unwrap_err().to_string().contains("symbolic links"));

        let layer = tar_bytes(&[
            TarEntry::file("f", 0o644, "x"),
            TarEntry::file("f/child", 0o644, "y"),
        ]);
        let (_dir, result) = unpack_layers(&[layer]);
        assert!(result.unwrap_err().to_string().contains("not a directory"));
    }

    #[test]
    fn fifos_are_created_and_devices_skipped() {
        let layer = tar_bytes(&[
            TarEntry::fifo("run/pipe", 0o600),
            TarEntry::char_device("dev/null"),
            TarEntry::file("after", 0o644, "ok"),
        ]);
        let (dir, result) = unpack_layers(&[layer]);
        let stats = result.unwrap();
        assert_eq!(stats.skipped_devices, 1);
        let ft = fs::symlink_metadata(root(&dir).join("run/pipe"))
            .unwrap()
            .file_type();
        assert!(std::os::unix::fs::FileTypeExt::is_fifo(&ft));
        assert!(!exists(&dir, "dev/null"));
        assert_eq!(read(&dir, "after"), "ok");
    }

    #[test]
    fn mtimes_of_files_are_restored() {
        let layer = tar_bytes(&[TarEntry::file("f", 0o644, "x").with_mtime(1_000_000_000)]);
        let (dir, result) = unpack_layers(&[layer]);
        result.unwrap();
        assert_eq!(
            fs::metadata(root(&dir).join("f")).unwrap().mtime(),
            1_000_000_000
        );
    }

    #[test]
    fn limits_on_size_and_entries_are_enforced() {
        let layer = tar_bytes(&[
            TarEntry::file("a", 0o644, "0123456789"),
            TarEntry::file("b", 0o644, "0123456789"),
        ]);
        let (_dir, result) = unpack_with(
            std::slice::from_ref(&layer),
            UnpackLimits {
                max_bytes: 15,
                max_entries: 100,
            },
        );
        assert!(matches!(result.unwrap_err(), OciError::Limit(_)));
        let (_dir, result) = unpack_with(
            &[layer],
            UnpackLimits {
                max_bytes: 100,
                max_entries: 1,
            },
        );
        assert!(matches!(result.unwrap_err(), OciError::Limit(_)));
    }

    #[test]
    fn a_decompression_bomb_is_stopped() {
        use flate2::write::GzEncoder;
        let big = vec![0u8; 8 * 1024 * 1024];
        let tar = tar_bytes(&[TarEntry::file("zeros", 0o644, big)]);
        let mut encoder = GzEncoder::new(Vec::new(), flate2::Compression::best());
        encoder.write_all(&tar).unwrap();
        let gz = encoder.finish().unwrap();
        assert!(gz.len() < 64 * 1024, "the bomb is small compressed");

        let dir = tempfile::tempdir().unwrap();
        fs::create_dir(dir.path().join("root")).unwrap();
        let mut unpacker = Unpacker::new(
            dir.path().join("root"),
            UnpackLimits {
                max_bytes: 1024 * 1024,
                max_entries: 10,
            },
            Arc::new(AtomicBool::new(false)),
        );
        let error = unpacker
            .apply_layer(gz.as_slice(), LayerCompression::Gzip)
            .unwrap_err();
        assert!(matches!(error, OciError::Limit(_)), "{error:?}");
    }

    #[test]
    fn compressed_layers_are_decoded() {
        use flate2::write::GzEncoder;
        let tar = tar_bytes(&[TarEntry::file("hello", 0o644, "gzip")]);
        let mut encoder = GzEncoder::new(Vec::new(), flate2::Compression::default());
        encoder.write_all(&tar).unwrap();
        let gz = encoder.finish().unwrap();
        let zst = zstd::encode_all(tar.as_slice(), 3).unwrap();

        for (bytes, compression) in [(gz, LayerCompression::Gzip), (zst, LayerCompression::Zstd)] {
            let dir = tempfile::tempdir().unwrap();
            fs::create_dir(dir.path().join("root")).unwrap();
            let mut unpacker = Unpacker::new(
                dir.path().join("root"),
                UnpackLimits::default(),
                Arc::new(AtomicBool::new(false)),
            );
            unpacker.apply_layer(bytes.as_slice(), compression).unwrap();
            assert_eq!(
                fs::read_to_string(dir.path().join("root/hello")).unwrap(),
                "gzip"
            );
        }

        let dir = tempfile::tempdir().unwrap();
        fs::create_dir(dir.path().join("root")).unwrap();
        let mut unpacker = Unpacker::new(
            dir.path().join("root"),
            UnpackLimits::default(),
            Arc::new(AtomicBool::new(false)),
        );
        assert!(
            unpacker
                .apply_layer(&b"not a gzip stream"[..], LayerCompression::Gzip)
                .is_err()
        );
    }

    #[test]
    fn truncated_archives_are_errors_and_interrupts_are_honoured() {
        let mut tar = tar_bytes(&[TarEntry::file("big", 0o644, vec![7u8; 4096])]);
        tar.truncate(1024);
        let (_dir, result) = unpack_layers(&[tar]);
        assert!(result.is_err());

        let dir = tempfile::tempdir().unwrap();
        fs::create_dir(dir.path().join("root")).unwrap();
        let cancel = Arc::new(AtomicBool::new(true));
        let mut unpacker = Unpacker::new(dir.path().join("root"), UnpackLimits::default(), cancel);
        let layer = tar_bytes(&[TarEntry::file("f", 0o644, "x")]);
        let error = unpacker
            .apply_layer(layer.as_slice(), LayerCompression::None)
            .unwrap_err();
        assert!(matches!(error, OciError::Interrupted));
    }
}
