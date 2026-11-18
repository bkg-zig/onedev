//! cgroup v2 handling of the youki runtime.
//!
//! Every container gets a cgroup of its own below a *parent* the server may
//! write to. That is what makes resource limits possible. A rootless runtime
//! can neither create cgroups anywhere else nor move a process between
//! arbitrary cgroups: moving needs write access to the common ancestor of the
//! old and the new cgroup, so the parent must live inside the subtree that was
//! delegated to the server (for systemd units: `Delegate=yes`).
//!
//! The runtime does not complain when it cannot move the container process;
//! it just stays in the server's cgroup and no limit applies. That is why this
//! module *tries* the move with a throw-away process instead of trusting
//! directory permissions.
//!
//! cgroup v2 only lets a cgroup hand controllers (memory, pids, ...) to its
//! children while it holds no processes itself. A server that was started in
//! its delegated cgroup is such a process. When limits are configured and the
//! controllers are not enabled yet, the server therefore does what systemd's
//! `DelegateSubgroup=` does (systemd 254 and newer): it moves itself into a
//! leaf cgroup ([`SUPERVISOR_LEAF`]) and enables the controllers for the
//! container cgroups next to that leaf.

use std::collections::BTreeSet;
use std::fs;
use std::io;
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

/// Where the unified cgroup hierarchy is mounted.
pub const CGROUP_ROOT: &str = "/sys/fs/cgroup";

/// Distinguishes the probes of several runtimes started by one process.
static PROBES: AtomicU64 = AtomicU64::new(0);

/// Name of the leaf cgroup the server moves itself into so that its own
/// cgroup can enable controllers for the containers.
pub const SUPERVISOR_LEAF: &str = "onedev-supervisor";

/// The cgroup that holds the containers' cgroups for a process living in
/// `own`: its delegated cgroup, i.e. `own` without the supervisor leaf the
/// server may have moved into earlier.
pub fn base_of(own: &str) -> &str {
    let leaf = format!("/{SUPERVISOR_LEAF}");
    match own.strip_suffix(leaf.as_str()) {
        Some("") => "/",
        Some(base) => base,
        None => own,
    }
}

/// What the server can do with the cgroup parent it was pointed at.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CgroupSetup {
    /// Cgroup path of the parent (`/system.slice/onedev.service`), as seen
    /// from the server's cgroup namespace.
    pub parent: String,
    /// Controllers available to children of the parent.
    pub controllers: BTreeSet<String>,
    /// A process can be moved into a child of the parent, so limits apply.
    pub can_attach: bool,
    /// Why missing controllers could not be enabled (reported with the
    /// error when limits need them).
    pub enable_error: Option<String>,
}

impl CgroupSetup {
    /// `linux.cgroupsPath` of the container called `name`.
    pub fn container_path(&self, name: &str) -> String {
        container_path(&self.parent, name)
    }
}

/// Controllers that resource limits need.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Needed {
    pub memory: bool,
    pub cpu: bool,
    pub pids: bool,
}

impl Needed {
    pub fn any(&self) -> bool {
        self.memory || self.cpu || self.pids
    }

    fn names(&self) -> Vec<&'static str> {
        let mut names = Vec::new();
        if self.memory {
            names.push("memory");
        }
        if self.cpu {
            names.push("cpu");
        }
        if self.pids {
            names.push("pids");
        }
        names
    }
}

pub fn container_path(parent: &str, name: &str) -> String {
    if parent == "/" {
        format!("/{name}")
    } else {
        format!("{parent}/{name}")
    }
}

/// The unified-hierarchy line (`0::/path`) of a `/proc/<pid>/cgroup` file.
pub fn parse_proc_cgroup(text: &str) -> Option<String> {
    text.lines()
        .find_map(|line| line.strip_prefix("0::"))
        .map(|path| path.trim().to_owned())
        .filter(|path| path.starts_with('/'))
}

/// The cgroup this process lives in.
pub fn own_cgroup() -> Result<String, String> {
    let text = fs::read_to_string("/proc/self/cgroup")
        .map_err(|e| format!("cannot read /proc/self/cgroup: {e}"))?;
    parse_proc_cgroup(&text).ok_or_else(|| {
        "this system does not use the unified cgroup v2 hierarchy, which the youki executor needs"
            .to_owned()
    })
}

/// Canonical form of a configured cgroup path: absolute, no `.`/`..`.
pub fn normalize(path: &str) -> Result<String, String> {
    let path = path.trim();
    if !path.starts_with('/') {
        return Err(format!("cgroup path `{path}` must be absolute"));
    }
    let mut parts = Vec::new();
    for part in path.split('/') {
        match part {
            "" | "." => {}
            ".." => return Err(format!("cgroup path `{path}` must not contain `..`")),
            part if part.chars().any(|c| c.is_control()) => {
                return Err(format!("cgroup path `{path}` contains control characters"));
            }
            part => parts.push(part),
        }
    }
    Ok(if parts.is_empty() {
        "/".to_owned()
    } else {
        format!("/{}", parts.join("/"))
    })
}

/// What a probe of the cgroup parent found.
struct Probe {
    controllers: BTreeSet<String>,
    can_attach: bool,
}

/// Create a short-lived child of `parent_dir` and see what it gets: the
/// controllers the parent enables for its children, and whether a process can
/// be moved into it.
fn probe(parent_dir: &Path, parent: &str) -> Result<Probe, String> {
    let probe = parent_dir.join(format!(
        "onedev-probe-{}-{}",
        std::process::id(),
        PROBES.fetch_add(1, Ordering::Relaxed)
    ));
    // a stale probe of an earlier crash of a process with the same pid
    let _ = remove_cgroup(&probe);
    fs::create_dir(&probe).map_err(|e| {
        format!(
            "cannot create cgroups below {parent}: {e}; run the server in a delegated cgroup \
             (systemd: `Delegate=yes`) or point the youki cgroup parent at a cgroup the server user owns"
        )
    })?;
    let controllers = fs::read_to_string(probe.join("cgroup.controllers"))
        .map(|text| text.split_whitespace().map(str::to_owned).collect())
        .unwrap_or_default();
    let can_attach = can_attach(&probe);
    let _ = remove_cgroup(&probe);
    Ok(Probe {
        controllers,
        can_attach,
    })
}

/// Enable `names` for the children of `parent_dir`. When the kernel refuses
/// because processes live in `parent_dir` and that is the server's own cgroup,
/// the server moves itself into a leaf below it first.
fn enable_controllers(
    parent_dir: &Path,
    parent: &str,
    names: &[&str],
    own_is_parent: bool,
) -> Result<(), String> {
    let request = names
        .iter()
        .map(|name| format!("+{name}"))
        .collect::<Vec<_>>()
        .join(" ");
    let control = parent_dir.join("cgroup.subtree_control");
    let refused = |e: io::Error| {
        format!(
            "cannot enable the cgroup controllers {} below {parent}: {e}; delegate them to the server \
             (systemd: `Delegate=yes`, for the cpu controller `Delegate=cpu memory pids`)",
            names.join(", ")
        )
    };
    match fs::write(&control, &request) {
        Ok(()) => return Ok(()),
        Err(e) if e.raw_os_error() == Some(libc::EBUSY) && own_is_parent => {}
        Err(e) => return Err(refused(e)),
    }
    // Processes live in the parent (at least this one): make room.
    let leaf = parent_dir.join(SUPERVISOR_LEAF);
    match fs::create_dir(&leaf) {
        Ok(()) => {}
        Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {}
        Err(e) => return Err(refused(e)),
    }
    fs::write(leaf.join("cgroup.procs"), std::process::id().to_string()).map_err(|e| {
        format!(
            "cannot move the server into {parent}/{SUPERVISOR_LEAF} to enable the cgroup controllers {}: {e}",
            names.join(", ")
        )
    })?;
    fs::write(&control, &request).map_err(|e| {
        format!(
            "cannot enable the cgroup controllers {} below {parent}: {e} (other processes besides the \
             server live in that cgroup; they must be in a cgroup of their own for controllers to be \
             delegated)",
            names.join(", ")
        )
    })
}

/// Inspect the cgroup parent and find out what it allows.
///
/// `root` is the cgroup mount point (injectable for tests), `own` the cgroup
/// of this process, `configured` the operator's choice (default: the cgroup
/// the server was delegated, see [`base_of`]) and `needed` the controllers the
/// configured resource limits depend on. Missing controllers are enabled when
/// the parent is the server's own cgroup (see the module documentation).
pub fn detect(
    root: &Path,
    own: &str,
    configured: Option<&str>,
    needed: Needed,
) -> Result<CgroupSetup, String> {
    if !root.join("cgroup.controllers").exists() {
        return Err(format!(
            "the cgroup v2 hierarchy is not mounted at {}",
            root.display()
        ));
    }
    let own_base = normalize(base_of(own))?;
    let parent = match configured {
        Some(path) => normalize(path)?,
        None => own_base.clone(),
    };
    let parent_dir = if parent == "/" {
        root.to_path_buf()
    } else {
        root.join(parent.trim_start_matches('/'))
    };
    if !parent_dir.is_dir() {
        return Err(format!(
            "the cgroup parent {parent} does not exist below {}",
            root.display()
        ));
    }

    let mut found = probe(&parent_dir, &parent)?;
    let mut enable_error = None;
    let missing: Vec<&str> = needed
        .names()
        .into_iter()
        .filter(|name| !found.controllers.contains(*name))
        .collect();
    if !missing.is_empty() && found.can_attach {
        match enable_controllers(&parent_dir, &parent, &missing, parent == own_base) {
            Ok(()) => found = probe(&parent_dir, &parent)?,
            // `require` reports what is still missing, with this as the reason
            Err(reason) => enable_error = Some(reason),
        }
    }

    Ok(CgroupSetup {
        parent,
        controllers: found.controllers,
        can_attach: found.can_attach,
        enable_error,
    })
}

/// Can this process place a (child) process into `cgroup`? A freshly spawned
/// child starts in our own cgroup, exactly like the container process does.
fn can_attach(cgroup: &Path) -> bool {
    let Ok(mut child) = Command::new("sleep")
        .arg("30")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
    else {
        return false;
    };
    let moved = fs::write(cgroup.join("cgroup.procs"), child.id().to_string()).is_ok();
    let _ = child.kill();
    let _ = child.wait();
    moved
}

/// `rmdir` of a cgroup; the kernel needs a moment to notice that the last
/// process is gone.
fn remove_cgroup(path: &Path) -> io::Result<()> {
    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        match fs::remove_dir(path) {
            Ok(()) => return Ok(()),
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
            Err(error) if error.raw_os_error() == Some(libc::EBUSY) && Instant::now() < deadline => {
                std::thread::sleep(Duration::from_millis(20));
            }
            Err(error) => return Err(error),
        }
    }
}

/// Check that the limits an operator asked for can be enforced.
pub fn require(setup: &CgroupSetup, needed: Needed) -> Result<(), String> {
    if !needed.any() {
        return Ok(());
    }
    let missing: Vec<&str> = needed
        .names()
        .into_iter()
        .filter(|name| !setup.controllers.contains(*name))
        .collect();
    if !missing.is_empty() {
        return Err(format!(
            "resource limits need the cgroup controller(s) {} below {}, but only [{}] are available; \
             delegate them (systemd: `Delegate=yes`, for the cpu controller `Delegate=cpu memory pids`) \
             or remove the limits{}",
            missing.join(", "),
            setup.parent,
            setup.controllers.iter().cloned().collect::<Vec<_>>().join(" "),
            setup
                .enable_error
                .as_ref()
                .map(|reason| format!(" ({reason})"))
                .unwrap_or_default()
        ));
    }
    if !setup.can_attach {
        return Err(format!(
            "resource limits are configured, but this process may not move container processes into \
             cgroups below {} (the common ancestor with the server's own cgroup is not writable); run the \
             server as a systemd service with `Delegate=yes` or set a cgroup parent inside the server's \
             own delegated cgroup",
            setup.parent
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_unified_line_of_proc_cgroup_is_found() {
        assert_eq!(
            parse_proc_cgroup("0::/user.slice/user-1000.slice/session-3.scope\n").as_deref(),
            Some("/user.slice/user-1000.slice/session-3.scope")
        );
        assert_eq!(
            parse_proc_cgroup("12:memory:/x\n1:name=systemd:/y\n0::/\n").as_deref(),
            Some("/")
        );
        // a v1-only system has no unified line
        assert_eq!(parse_proc_cgroup("12:memory:/x\n1:name=systemd:/y\n"), None);
        assert_eq!(parse_proc_cgroup("0::relative\n"), None);
        assert_eq!(parse_proc_cgroup(""), None);
    }

    #[test]
    fn paths_are_normalised_and_escapes_rejected() {
        assert_eq!(normalize("/a//b/./c/").unwrap(), "/a/b/c");
        assert_eq!(normalize(" / ").unwrap(), "/");
        assert!(normalize("a/b").is_err());
        assert!(normalize("/a/../b").is_err());
        assert!(normalize("/a/b\nc").is_err());
    }

    #[test]
    fn container_paths_hang_below_the_parent() {
        assert_eq!(container_path("/svc/onedev", "onedev-1"), "/svc/onedev/onedev-1");
        assert_eq!(container_path("/", "onedev-1"), "/onedev-1");
    }

    /// A directory tree standing in for the cgroup mount. Real cgroupfs has
    /// kernel-provided files; the stand-in only needs `cgroup.controllers`.
    fn fake_root() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("cgroup.controllers"), "cpu memory pids\n").unwrap();
        fs::create_dir_all(dir.path().join("system.slice/onedev.service")).unwrap();
        dir
    }

    #[test]
    fn a_writable_parent_is_accepted() {
        let root = fake_root();
        // the stand-in has no cgroup.procs semantics: a plain write "works"
        let setup = detect(
            root.path(),
            "/somewhere/else",
            Some("/system.slice/onedev.service"),
            Needed::default(),
        )
        .unwrap();
        assert_eq!(setup.parent, "/system.slice/onedev.service");
        assert!(setup.can_attach);
        assert_eq!(
            setup.container_path("onedev-x"),
            "/system.slice/onedev.service/onedev-x"
        );

        // without a configured parent the server's own cgroup is used
        let own = detect(
            root.path(),
            "/system.slice/onedev.service",
            None,
            Needed::default(),
        )
        .unwrap();
        assert_eq!(own.parent, "/system.slice/onedev.service");

        // ... and so is its delegated cgroup when the server sits in the supervisor leaf
        let leaf = detect(
            root.path(),
            "/system.slice/onedev.service/onedev-supervisor",
            None,
            Needed::default(),
        )
        .unwrap();
        assert_eq!(leaf.parent, "/system.slice/onedev.service");
        assert_eq!(leaf.enable_error, None);
    }

    #[test]
    fn the_supervisor_leaf_is_stripped_from_the_servers_own_cgroup() {
        assert_eq!(base_of("/system.slice/onedev.service"), "/system.slice/onedev.service");
        assert_eq!(
            base_of("/system.slice/onedev.service/onedev-supervisor"),
            "/system.slice/onedev.service"
        );
        assert_eq!(base_of("/onedev-supervisor"), "/");
        assert_eq!(base_of("/"), "/");
        // only the exact leaf name counts
        assert_eq!(base_of("/a/not-onedev-supervisor"), "/a/not-onedev-supervisor");
        assert_eq!(base_of("/a/onedev-supervisor/b"), "/a/onedev-supervisor/b");
    }

    #[test]
    fn controllers_are_requested_with_a_plus_each() {
        let root = fake_root();
        let parent = root.path().join("system.slice/onedev.service");
        enable_controllers(&parent, "/system.slice/onedev.service", &["memory", "pids"], true).unwrap();
        assert_eq!(
            fs::read_to_string(parent.join("cgroup.subtree_control")).unwrap(),
            "+memory +pids"
        );
        // nothing to relocate into when the parent does not exist
        let error = enable_controllers(&root.path().join("gone"), "/gone", &["memory"], true).unwrap_err();
        assert!(error.contains("cannot enable the cgroup controllers memory below /gone"), "{error}");
        assert!(error.contains("Delegate=yes"), "{error}");
    }

    #[test]
    fn what_could_not_be_enabled_is_part_of_the_limit_error() {
        use std::os::unix::fs::PermissionsExt;
        let root = fake_root();
        let control = root
            .path()
            .join("system.slice/onedev.service/cgroup.subtree_control");
        fs::write(&control, "").unwrap();
        fs::set_permissions(&control, fs::Permissions::from_mode(0o444)).unwrap();
        if fs::write(&control, "x").is_ok() {
            // running as root: permission bits do not apply, nothing to test
            return;
        }
        let needed = Needed {
            memory: true,
            ..Needed::default()
        };
        let setup = detect(root.path(), "/system.slice/onedev.service", None, needed).unwrap();
        assert!(setup.controllers.is_empty());
        let reason = setup.enable_error.clone().unwrap();
        assert!(
            reason.contains("cannot enable the cgroup controllers memory"),
            "{reason}"
        );
        let error = require(&setup, needed).unwrap_err();
        assert!(error.contains(&reason), "{error}");
    }

    #[test]
    fn missing_parents_and_unmounted_hierarchies_are_reported() {
        let root = fake_root();
        let none = Needed::default();
        let error = detect(root.path(), "/", Some("/does/not/exist"), none).unwrap_err();
        assert!(error.contains("does not exist"), "{error}");

        let empty = tempfile::tempdir().unwrap();
        let error = detect(empty.path(), "/", None, none).unwrap_err();
        assert!(error.contains("not mounted"), "{error}");

        let error = detect(root.path(), "/", Some("relative"), none).unwrap_err();
        assert!(error.contains("absolute"), "{error}");
    }

    #[test]
    fn an_unwritable_parent_says_how_to_fix_it() {
        use std::os::unix::fs::PermissionsExt;
        let root = fake_root();
        let parent = root.path().join("system.slice/onedev.service");
        fs::set_permissions(&parent, fs::Permissions::from_mode(0o555)).unwrap();
        if fs::create_dir(parent.join("probe-writable-check")).is_ok() {
            // running as root: permission bits do not apply, nothing to test
            return;
        }
        let error = detect(
            root.path(),
            "/",
            Some("/system.slice/onedev.service"),
            Needed::default(),
        )
        .unwrap_err();
        assert!(error.contains("Delegate=yes"), "{error}");
        fs::set_permissions(&parent, fs::Permissions::from_mode(0o755)).unwrap();
    }

    #[test]
    fn limits_need_delegated_controllers_and_the_ability_to_attach() {
        let setup = CgroupSetup {
            parent: "/svc".to_owned(),
            controllers: ["memory", "pids"].iter().map(|s| (*s).to_owned()).collect(),
            can_attach: true,
            enable_error: None,
        };
        assert!(require(&setup, Needed::default()).is_ok());
        assert!(require(&setup, Needed { memory: true, pids: true, cpu: false }).is_ok());
        let error = require(&setup, Needed { cpu: true, ..Needed::default() }).unwrap_err();
        assert!(error.contains("cpu") && error.contains("memory pids"), "{error}");

        let refused = CgroupSetup {
            enable_error: Some("other processes live there".to_owned()),
            ..setup.clone()
        };
        let error = require(&refused, Needed { cpu: true, ..Needed::default() }).unwrap_err();
        assert!(error.ends_with("(other processes live there)"), "{error}");

        let stuck = CgroupSetup {
            can_attach: false,
            ..setup
        };
        assert!(require(&stuck, Needed::default()).is_ok(), "no limits, no need to attach");
        let error = require(&stuck, Needed { memory: true, ..Needed::default() }).unwrap_err();
        assert!(error.contains("may not move"), "{error}");
    }
}
