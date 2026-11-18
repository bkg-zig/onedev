//! Root filesystems of step containers.
//!
//! A pulled image is an immutable directory tree shared by every step that
//! uses it. A step must neither change it nor see what an earlier step did to
//! "its" root filesystem, so each container gets a private view:
//!
//! * **Overlay** (kernel >= 5.11, unprivileged overlayfs in a user
//!   namespace): the bundle's `rootfs/` is only a *skeleton*. For every
//!   top-level directory of the image the container's mount namespace mounts
//!   an overlay (`lowerdir` = the shared image directory, `upperdir`/`workdir`
//!   = fresh directories of the bundle). Creating a step costs a handful of
//!   `mkdir`s instead of a copy of the image, and the writable layer is
//!   removed with the bundle. The runtime mounts the overlays itself (they are
//!   entries of the spec's `mounts`), so nothing needs to be mounted on the
//!   host.
//! * **Copy**: the whole image is copied into the bundle. Slow and disk-hungry,
//!   but works everywhere; used when the host cannot do unprivileged overlay
//!   mounts.
//!
//! Top-level symlinks (`/bin -> usr/bin`) and files (`/entrypoint.sh`) cannot
//! be mount points and are replicated in the skeleton. `/proc`, `/dev`, `/sys`
//! and `/onedev-build` belong to the runtime and are never taken from the
//! image.
//!
//! `/etc/hostname`, `/etc/hosts` and `/etc/resolv.conf` are written into the
//! writable layer (regular files, like Docker's): the image's own versions
//! are shadowed.

use std::fs::{self, OpenOptions};
use std::io;
use std::os::unix::fs::{FileTypeExt, OpenOptionsExt, PermissionsExt, symlink};
use std::path::{Path, PathBuf};

use serde_json::{Value, json};

/// Names the runtime provides itself.
const RESERVED: [&str; 4] = ["proc", "dev", "sys", "onedev-build"];

/// Top-level directories every container needs; created (plain) when the
/// image has none.
const REQUIRED_DIRS: [(&str, u32); 3] = [("tmp", 0o1777), ("root", 0o700), ("etc", 0o755)];

/// One overlay of the container's root filesystem.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OverlayMount {
    /// Mount point inside the container (`/usr`).
    pub destination: String,
    pub lower: PathBuf,
    pub upper: PathBuf,
    pub work: PathBuf,
}

impl OverlayMount {
    /// `mounts` entry of the runtime spec. `userxattr` is what makes
    /// overlayfs usable without privileges (it keeps its bookkeeping in
    /// `user.*` instead of `trusted.*` attributes).
    pub fn to_json(&self) -> Value {
        json!({
            "destination": self.destination,
            "type": "overlay",
            "source": "overlay",
            "options": [
                format!("lowerdir={}", self.lower.display()),
                format!("upperdir={}", self.upper.display()),
                format!("workdir={}", self.work.display()),
                "userxattr",
            ],
        })
    }
}

/// Contents of the files the runtime writes into `/etc`.
#[derive(Debug, Clone)]
pub struct EtcFiles {
    pub hostname: String,
    pub hosts: String,
    pub resolv_conf: String,
}

impl EtcFiles {
    pub fn new(hostname: &str, resolv_conf: String) -> Self {
        Self {
            hostname: format!("{hostname}\n"),
            hosts: format!(
                "127.0.0.1\tlocalhost {hostname}\n::1\tlocalhost ip6-localhost ip6-loopback\n"
            ),
            resolv_conf,
        }
    }

    fn files(&self) -> [(&'static str, &str); 3] {
        [
            ("hostname", &self.hostname),
            ("hosts", &self.hosts),
            ("resolv.conf", &self.resolv_conf),
        ]
    }
}

fn context<T>(result: io::Result<T>, what: &str, path: &Path) -> io::Result<T> {
    result.map_err(|e| io::Error::new(e.kind(), format!("{what} {}: {e}", path.display())))
}

fn invalid(message: String) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, message)
}

/// Characters that cannot appear in an overlay option value (`,` separates
/// options, `:` separates lower directories, `\` escapes).
fn mount_option_safe(path: &Path) -> bool {
    path.to_str()
        .is_some_and(|text| !text.contains([',', ':', '\\', '\0', '\n']))
}

struct TopLevel {
    name: String,
    kind: Kind,
}

enum Kind {
    Dir { mode: u32 },
    Symlink(PathBuf),
    File,
}

/// Top-level entries of the image (without the names the runtime owns),
/// sorted for a deterministic mount order.
fn top_level(image: &Path) -> io::Result<Vec<TopLevel>> {
    let mut entries = Vec::new();
    for entry in context(fs::read_dir(image), "cannot read the image", image)? {
        let entry = context(entry, "cannot read the image", image)?;
        let Some(name) = entry.file_name().to_str().map(str::to_owned) else {
            return Err(invalid(format!(
                "the image has a top-level entry with a name that is not valid UTF-8 ({:?})",
                entry.file_name()
            )));
        };
        if RESERVED.contains(&name.as_str()) {
            continue;
        }
        let path = entry.path();
        let meta = context(fs::symlink_metadata(&path), "cannot inspect", &path)?;
        let file_type = meta.file_type();
        let kind = if file_type.is_dir() {
            Kind::Dir {
                mode: meta.permissions().mode() & 0o7777,
            }
        } else if file_type.is_symlink() {
            Kind::Symlink(context(fs::read_link(&path), "cannot read the link", &path)?)
        } else if file_type.is_file() {
            Kind::File
        } else {
            continue; // sockets, devices: nothing to take over
        };
        entries.push(TopLevel { name, kind });
    }
    entries.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(entries)
}

fn make_dir(path: &Path, mode: u32) -> io::Result<()> {
    context(fs::create_dir_all(path), "cannot create", path)?;
    // explicit, so that the umask cannot weaken (or strengthen) the mode
    context(
        fs::set_permissions(path, fs::Permissions::from_mode(mode)),
        "cannot set the mode of",
        path,
    )
}

fn plain_runtime_dirs(rootfs: &Path) -> io::Result<()> {
    for name in ["proc", "dev", "sys"] {
        make_dir(&rootfs.join(name), 0o755)?;
    }
    make_dir(&rootfs.join("onedev-build").join("workspace"), 0o755)?;
    // the parent was created with the default mode by create_dir_all
    context(
        fs::set_permissions(
            rootfs.join("onedev-build"),
            fs::Permissions::from_mode(0o755),
        ),
        "cannot set the mode of",
        &rootfs.join("onedev-build"),
    )
}

fn ensure_required_dirs(rootfs: &Path) -> io::Result<()> {
    for (name, mode) in REQUIRED_DIRS {
        let path = rootfs.join(name);
        if fs::symlink_metadata(&path).is_err() {
            make_dir(&path, mode)?;
        }
    }
    Ok(())
}

/// Write `content` as a new regular file, never following a link that might
/// sit at `path` (inside an image, `/etc/resolv.conf` is often a symlink to
/// somewhere on the host).
fn write_new_file(path: &Path, content: &str) -> io::Result<()> {
    use std::io::Write as _;
    match fs::remove_file(path) {
        Ok(()) => {}
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return context(Err(error), "cannot replace", path),
    }
    let mut file = context(
        OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o644)
            .custom_flags(libc::O_NOFOLLOW)
            .open(path),
        "cannot create",
        path,
    )?;
    context(file.write_all(content.as_bytes()), "cannot write", path)
}

fn write_etc(dir: &Path, etc: &EtcFiles) -> io::Result<()> {
    let meta = context(fs::symlink_metadata(dir), "cannot inspect", dir)?;
    if !meta.is_dir() {
        return Err(invalid(format!(
            "unsupported image: {} is not a plain directory",
            dir.display()
        )));
    }
    for (name, content) in etc.files() {
        write_new_file(&dir.join(name), content)?;
    }
    Ok(())
}

/// Prepare the root filesystem of one container as overlays over `image`.
///
/// Creates `bundle/rootfs` (skeleton), `bundle/upper/*` and `bundle/work/*`
/// and returns the overlay mounts the runtime has to perform, in order.
pub fn prepare_overlay(
    image: &Path,
    bundle: &Path,
    etc: &EtcFiles,
) -> io::Result<Vec<OverlayMount>> {
    let rootfs = bundle.join("rootfs");
    make_dir(&rootfs, 0o755)?;
    let mut mounts = Vec::new();
    let mut etc_in_upper = None;

    for entry in top_level(image)? {
        let target = rootfs.join(&entry.name);
        match entry.kind {
            Kind::Dir { mode } => {
                let lower = image.join(&entry.name);
                let upper = bundle.join("upper").join(&entry.name);
                let work = bundle.join("work").join(&entry.name);
                for path in [&lower, &upper, &work] {
                    if !mount_option_safe(path) {
                        return Err(invalid(format!(
                            "unsupported image: the directory name `{}` contains characters that cannot be used in a mount",
                            entry.name
                        )));
                    }
                }
                make_dir(&target, 0o755)?;
                // The root directory of an overlay shows the upper directory's
                // attributes: they must be the image's (1777 for /tmp, 0700 for /root).
                make_dir(&upper, mode)?;
                make_dir(&work, 0o700)?;
                if entry.name == "etc" {
                    etc_in_upper = Some(upper.clone());
                }
                mounts.push(OverlayMount {
                    destination: format!("/{}", entry.name),
                    lower,
                    upper,
                    work,
                });
            }
            Kind::Symlink(link) => {
                context(symlink(&link, &target), "cannot create the link", &target)?;
            }
            Kind::File => {
                context(
                    fs::copy(image.join(&entry.name), &target),
                    "cannot copy",
                    &target,
                )?;
            }
        }
    }

    plain_runtime_dirs(&rootfs)?;
    ensure_required_dirs(&rootfs)?;
    match etc_in_upper {
        Some(upper) => write_etc(&upper, etc)?,
        None => write_etc(&rootfs.join("etc"), etc)?,
    }
    Ok(mounts)
}

/// Prepare the root filesystem of one container as a full copy of `image`.
pub fn prepare_copy(image: &Path, bundle: &Path, etc: &EtcFiles) -> io::Result<()> {
    let rootfs = bundle.join("rootfs");
    make_dir(&rootfs, 0o755)?;
    for entry in top_level(image)? {
        copy_tree(&image.join(&entry.name), &rootfs.join(&entry.name))?;
    }
    plain_runtime_dirs(&rootfs)?;
    ensure_required_dirs(&rootfs)?;
    write_etc(&rootfs.join("etc"), etc)
}

/// Copy a tree preserving modes, symlinks and FIFOs. Iterative: an image can
/// nest directories as deep as the path length allows.
fn copy_tree(src: &Path, dst: &Path) -> io::Result<()> {
    let mut pending = vec![(src.to_path_buf(), dst.to_path_buf())];
    // directory modes are applied after their content (a mode without write
    // permission must not stop us from filling the directory)
    let mut modes: Vec<(PathBuf, fs::Permissions)> = Vec::new();
    while let Some((from, to)) = pending.pop() {
        let meta = context(fs::symlink_metadata(&from), "cannot inspect", &from)?;
        let file_type = meta.file_type();
        if file_type.is_dir() {
            context(fs::create_dir(&to), "cannot create", &to)?;
            // a directory we may not read cannot be part of a usable image
            for child in context(fs::read_dir(&from), "cannot read", &from)? {
                let child = context(child, "cannot read", &from)?;
                pending.push((child.path(), to.join(child.file_name())));
            }
            modes.push((to, meta.permissions()));
        } else if file_type.is_symlink() {
            let link = context(fs::read_link(&from), "cannot read the link", &from)?;
            context(symlink(link, &to), "cannot create the link", &to)?;
        } else if file_type.is_file() {
            context(fs::copy(&from, &to), "cannot copy to", &to)?;
        } else if file_type.is_fifo() {
            let c_path = std::ffi::CString::new(to.as_os_str().as_encoded_bytes())
                .map_err(|_| invalid(format!("invalid path {}", to.display())))?;
            // SAFETY: mkfifo(3) with a valid NUL-terminated path.
            let status =
                unsafe { libc::mkfifo(c_path.as_ptr(), (meta.permissions().mode() & 0o7777) as libc::mode_t) };
            if status != 0 {
                return context(Err(io::Error::last_os_error()), "cannot create the FIFO", &to);
            }
        }
    }
    for (path, permissions) in modes.into_iter().rev() {
        context(fs::set_permissions(&path, permissions), "cannot set the mode of", &path)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::os::unix::fs::MetadataExt;

    use super::*;

    fn write(path: &Path, content: &str, mode: u32) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, content).unwrap();
        fs::set_permissions(path, fs::Permissions::from_mode(mode)).unwrap();
    }

    fn dir(path: &Path, mode: u32) {
        fs::create_dir_all(path).unwrap();
        fs::set_permissions(path, fs::Permissions::from_mode(mode)).unwrap();
    }

    fn mode_of(path: &Path) -> u32 {
        fs::symlink_metadata(path).unwrap().permissions().mode() & 0o7777
    }

    fn etc() -> EtcFiles {
        EtcFiles::new("host1", "nameserver 192.0.2.53\n".to_owned())
    }

    /// An image with the usual suspects.
    fn sample_image(root: &Path) {
        dir(&root.join("bin"), 0o755);
        write(&root.join("bin/sh"), "#!shell", 0o755);
        dir(&root.join("etc"), 0o755);
        write(&root.join("etc/os-release"), "NAME=test\n", 0o644);
        symlink("/run/systemd/resolve/stub-resolv.conf", root.join("etc/resolv.conf")).unwrap();
        dir(&root.join("tmp"), 0o1777);
        dir(&root.join("root"), 0o700);
        dir(&root.join("usr"), 0o755);
        dir(&root.join("usr/lib"), 0o755);
        write(&root.join("usr/lib/libx.so"), "x", 0o644);
        symlink("usr/lib", root.join("lib")).unwrap();
        write(&root.join("entrypoint.sh"), "#!entry", 0o750);
        // provided by the runtime: whatever the image has there is ignored
        write(&root.join("proc/leftover"), "x", 0o644);
        write(&root.join("onedev-build/image-file"), "x", 0o644);
    }

    #[test]
    fn the_overlay_skeleton_mirrors_the_images_top_level() {
        let temp = tempfile::tempdir().unwrap();
        let image = temp.path().join("image");
        let bundle = temp.path().join("bundle");
        sample_image(&image);
        fs::create_dir(&bundle).unwrap();

        let mounts = prepare_overlay(&image, &bundle, &etc()).unwrap();
        let destinations: Vec<&str> = mounts.iter().map(|m| m.destination.as_str()).collect();
        assert_eq!(destinations, ["/bin", "/etc", "/root", "/tmp", "/usr"]);
        let usr = mounts.iter().find(|m| m.destination == "/usr").unwrap();
        assert_eq!(usr.lower, image.join("usr"));
        assert_eq!(usr.upper, bundle.join("upper/usr"));
        assert_eq!(usr.work, bundle.join("work/usr"));

        let rootfs = bundle.join("rootfs");
        // mount points are empty plain directories: the image stays untouched
        for name in ["bin", "etc", "root", "tmp", "usr"] {
            assert!(rootfs.join(name).is_dir(), "{name}");
            assert_eq!(fs::read_dir(rootfs.join(name)).unwrap().count(), 0, "{name} is only a mount point");
        }
        // replicated top-level link and file
        assert_eq!(fs::read_link(rootfs.join("lib")).unwrap(), PathBuf::from("usr/lib"));
        assert_eq!(fs::read_to_string(rootfs.join("entrypoint.sh")).unwrap(), "#!entry");
        assert_eq!(mode_of(&rootfs.join("entrypoint.sh")), 0o750);
        // the runtime's own directories, not the image's
        assert!(rootfs.join("proc").is_dir());
        assert!(!rootfs.join("proc/leftover").exists());
        assert!(rootfs.join("dev").is_dir() && rootfs.join("sys").is_dir());
        assert!(rootfs.join("onedev-build/workspace").is_dir());
        assert!(!rootfs.join("onedev-build/image-file").exists());

        // the root of an overlay shows the upper directory's mode
        assert_eq!(mode_of(&bundle.join("upper/tmp")), 0o1777);
        assert_eq!(mode_of(&bundle.join("upper/root")), 0o700);
        assert_eq!(mode_of(&bundle.join("upper/usr")), 0o755);
        assert!(bundle.join("work/usr").is_dir());
    }

    #[test]
    fn etc_files_land_in_the_writable_layer_and_shadow_the_images() {
        let temp = tempfile::tempdir().unwrap();
        let image = temp.path().join("image");
        let bundle = temp.path().join("bundle");
        sample_image(&image);
        fs::create_dir(&bundle).unwrap();
        prepare_overlay(&image, &bundle, &etc()).unwrap();

        let upper = bundle.join("upper/etc");
        assert_eq!(fs::read_to_string(upper.join("hostname")).unwrap(), "host1\n");
        let hosts = fs::read_to_string(upper.join("hosts")).unwrap();
        assert!(hosts.contains("127.0.0.1\tlocalhost host1"), "{hosts}");
        assert!(hosts.contains("::1"), "{hosts}");
        assert_eq!(
            fs::read_to_string(upper.join("resolv.conf")).unwrap(),
            "nameserver 192.0.2.53\n"
        );
        assert_eq!(mode_of(&upper.join("hosts")), 0o644);
        // the image itself is never written to
        assert!(fs::symlink_metadata(image.join("etc/resolv.conf")).unwrap().file_type().is_symlink());
        assert!(!image.join("etc/hostname").exists());
    }

    #[test]
    fn missing_standard_directories_are_created() {
        let temp = tempfile::tempdir().unwrap();
        let image = temp.path().join("image");
        let bundle = temp.path().join("bundle");
        write(&image.join("bin/tool"), "x", 0o755);
        fs::create_dir(&bundle).unwrap();

        let mounts = prepare_overlay(&image, &bundle, &etc()).unwrap();
        assert_eq!(mounts.len(), 1, "only /bin comes from the image");
        let rootfs = bundle.join("rootfs");
        assert_eq!(mode_of(&rootfs.join("tmp")), 0o1777);
        assert_eq!(mode_of(&rootfs.join("root")), 0o700);
        assert!(rootfs.join("etc/hostname").is_file(), "etc files go straight into the skeleton");
    }

    #[test]
    fn a_symlinked_etc_is_refused_instead_of_followed() {
        let temp = tempfile::tempdir().unwrap();
        let image = temp.path().join("image");
        let bundle = temp.path().join("bundle");
        let outside = temp.path().join("outside");
        fs::create_dir_all(&image).unwrap();
        fs::create_dir_all(&outside).unwrap();
        fs::create_dir(&bundle).unwrap();
        symlink(&outside, image.join("etc")).unwrap();

        let error = prepare_overlay(&image, &bundle, &etc()).unwrap_err().to_string();
        assert!(error.contains("not a plain directory"), "{error}");
        assert_eq!(fs::read_dir(&outside).unwrap().count(), 0, "nothing was written through the link");
    }

    #[test]
    fn names_that_cannot_be_mount_options_are_rejected() {
        for name in ["a,b", "a:b", "a\\b"] {
            let temp = tempfile::tempdir().unwrap();
            let image = temp.path().join("image");
            let bundle = temp.path().join("bundle");
            dir(&image.join(name), 0o755);
            fs::create_dir(&bundle).unwrap();
            let error = prepare_overlay(&image, &bundle, &etc()).unwrap_err();
            assert_eq!(error.kind(), io::ErrorKind::InvalidInput, "{name}");
            assert!(error.to_string().contains("cannot be used in a mount"), "{error}");
        }
    }

    #[test]
    fn overlay_mounts_render_as_runtime_spec_entries() {
        let mount = OverlayMount {
            destination: "/usr".to_owned(),
            lower: PathBuf::from("/l"),
            upper: PathBuf::from("/u"),
            work: PathBuf::from("/w"),
        };
        assert_eq!(
            mount.to_json(),
            json!({
                "destination": "/usr",
                "type": "overlay",
                "source": "overlay",
                "options": ["lowerdir=/l", "upperdir=/u", "workdir=/w", "userxattr"]
            })
        );
    }

    #[test]
    fn copy_mode_copies_the_whole_image_and_keeps_the_original_pristine() {
        let temp = tempfile::tempdir().unwrap();
        let image = temp.path().join("image");
        let bundle = temp.path().join("bundle");
        sample_image(&image);
        // an absolute link must stay a link, not be resolved on the host
        let host_file = temp.path().join("host-resolv.conf");
        fs::write(&host_file, "host content\n").unwrap();
        fs::remove_file(image.join("etc/resolv.conf")).unwrap();
        symlink(&host_file, image.join("etc/resolv.conf")).unwrap();
        fs::create_dir(&bundle).unwrap();

        prepare_copy(&image, &bundle, &etc()).unwrap();
        let rootfs = bundle.join("rootfs");
        assert_eq!(fs::read_to_string(rootfs.join("usr/lib/libx.so")).unwrap(), "x");
        assert_eq!(fs::read_to_string(rootfs.join("bin/sh")).unwrap(), "#!shell");
        assert_eq!(mode_of(&rootfs.join("bin/sh")), 0o755);
        assert_eq!(mode_of(&rootfs.join("tmp")), 0o1777);
        assert_eq!(mode_of(&rootfs.join("root")), 0o700);
        assert_eq!(fs::read_link(rootfs.join("lib")).unwrap(), PathBuf::from("usr/lib"));
        assert!(!rootfs.join("proc/leftover").exists());
        assert!(rootfs.join("onedev-build/workspace").is_dir());

        // /etc/resolv.conf was a link to a host file: it is replaced, the target untouched
        assert_eq!(
            fs::read_to_string(rootfs.join("etc/resolv.conf")).unwrap(),
            "nameserver 192.0.2.53\n"
        );
        assert!(!fs::symlink_metadata(rootfs.join("etc/resolv.conf")).unwrap().file_type().is_symlink());
        assert_eq!(fs::read_to_string(&host_file).unwrap(), "host content\n");
        assert_eq!(fs::read_to_string(rootfs.join("etc/os-release")).unwrap(), "NAME=test\n");
        assert!(!image.join("etc/hostname").exists(), "the image is never modified");

        // a copy is independent: writing to it leaves the image alone
        fs::write(rootfs.join("usr/lib/libx.so"), "changed").unwrap();
        assert_eq!(fs::read_to_string(image.join("usr/lib/libx.so")).unwrap(), "x");
        assert_ne!(
            fs::metadata(rootfs.join("usr/lib/libx.so")).unwrap().ino(),
            fs::metadata(image.join("usr/lib/libx.so")).unwrap().ino()
        );
    }

    #[test]
    fn copy_mode_handles_fifos_restrictive_directories_and_deep_trees() {
        let temp = tempfile::tempdir().unwrap();
        let image = temp.path().join("image");
        let bundle = temp.path().join("bundle");
        dir(&image.join("opt/readonly"), 0o755);
        write(&image.join("opt/readonly/file"), "data", 0o444);
        fs::set_permissions(image.join("opt/readonly"), fs::Permissions::from_mode(0o555)).unwrap();
        let fifo = image.join("opt/pipe");
        let c_path = std::ffi::CString::new(fifo.as_os_str().as_encoded_bytes()).unwrap();
        assert_eq!(unsafe { libc::mkfifo(c_path.as_ptr(), 0o600) }, 0);
        // far deeper than recursion would survive on a small stack, within PATH_MAX
        let mut deep = image.join("deep");
        for _ in 0..900 {
            deep.push("d");
        }
        fs::create_dir_all(&deep).unwrap();
        fs::write(deep.join("leaf"), "leaf").unwrap();
        fs::create_dir(&bundle).unwrap();

        let result = prepare_copy(&image, &bundle, &etc());
        // let the temp dir be cleaned up even when an assertion fails
        fs::set_permissions(image.join("opt/readonly"), fs::Permissions::from_mode(0o755)).unwrap();
        result.unwrap();
        fs::set_permissions(
            bundle.join("rootfs/opt/readonly"),
            fs::Permissions::from_mode(0o755),
        )
        .unwrap();

        let rootfs = bundle.join("rootfs");
        assert_eq!(fs::read_to_string(rootfs.join("opt/readonly/file")).unwrap(), "data");
        assert!(fs::symlink_metadata(rootfs.join("opt/pipe")).unwrap().file_type().is_fifo());
        let mut copied = rootfs.join("deep");
        for _ in 0..900 {
            copied.push("d");
        }
        assert_eq!(fs::read_to_string(copied.join("leaf")).unwrap(), "leaf");
    }
}
