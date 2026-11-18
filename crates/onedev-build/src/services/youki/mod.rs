//! Rootless OCI containers for build steps, run by [youki](https://github.com/youki-dev/youki).
//!
//! This is the isolating alternative to Docker for `CommandStep`s: no daemon,
//! no root, no socket handed to untrusted code. The pieces:
//!
//! * [`crate::services::oci`] pulls the image into a shared, digest-addressed
//!   cache (native registry client, hardened layer unpacking),
//! * [`rootfs`] gives every step a private writable view of the image
//!   (unprivileged overlayfs, or a copy where the kernel cannot do that),
//! * [`spec`] writes the OCI runtime-spec bundle,
//! * the `youki` binary runs it in the foreground: its stdout/stderr are the
//!   step's output and its exit status is the step's exit status,
//! * [`cgroup`] places the container in a cgroup below a delegated parent so
//!   that memory/CPU/pid limits apply,
//! * [`probe`] verifies at start-up that the host supports all of this.
//!
//! # What "rootless" means for steps
//!
//! The container has a single-entry user namespace: the server's user is
//! `root` inside, nobody else exists. Files created by a step belong to the
//! server user on the host (the workspace stays usable), but `chown` to other
//! users, `su`, the `_apt` sandbox user, `runAs` of another user and anything
//! else that needs a second uid cannot work. Steps run with Docker's default
//! capability set *inside* that namespace and `no-new-privileges`.
//!
//! The network is the host's (`YoukiNetwork::Host`, like a plain `docker run
//! --network host`) or none at all. There is no NAT-ed bridge: that needs
//! privileges or helpers such as slirp4netns/pasta.

pub mod cgroup;
pub mod probe;
pub mod rootfs;
pub mod spec;

use std::ffi::OsString;
use std::fmt;
use std::fs::{self, File};
use std::io::{self, Read};
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::time::{Duration, Instant};

use self::cgroup::CgroupSetup;
use self::rootfs::{EtcFiles, OverlayMount};
use self::spec::{Limits, SpecInput};
use crate::services::executor::StopReason;
use crate::services::oci::store::{open_lock_file, remove_tree, try_flock};
use crate::services::oci::unpack::UnpackLimits;
use crate::services::oci::{
    CredentialStore, ImageReference, ImageStore, Platform, PruneOptions, PruneReport, PullEvent,
    PulledImage, RegistryClient, RegistryOptions, StoreOptions,
};

/// How a container's root filesystem is built from the image.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RootfsMode {
    /// Overlay when the host supports it, a copy otherwise.
    #[default]
    Auto,
    /// Overlay; refuse to start if the host cannot do it.
    Overlay,
    /// Always copy the image (slow, needs disk space for every step).
    Copy,
}

/// The network a step sees.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum YoukiNetwork {
    /// The host's network namespace: everything the server can reach, the step
    /// can reach, including services on the server's loopback interface.
    #[default]
    Host,
    /// A network namespace of its own with no interface up (not even
    /// loopback): the step cannot talk to anything.
    None,
}

/// Settings of the youki executor.
#[derive(Debug, Clone)]
pub struct YoukiConfig {
    /// The `youki` executable (looked up in `PATH` when not a path).
    pub binary: PathBuf,
    /// Private working area: runtime state, bundles and the image cache. One
    /// server instance per directory.
    pub state_dir: PathBuf,
    pub rootfs: RootfsMode,
    pub network: YoukiNetwork,
    /// Cgroup below which containers get their cgroup (default: the
    /// server's own cgroup).
    pub cgroup_parent: Option<String>,
    pub limits: Limits,
    /// Registries (`host[:port]`) that are contacted over plain HTTP.
    pub insecure_registries: Vec<String>,
    /// Docker `config.json` with registry credentials.
    pub registry_credentials_file: Option<PathBuf>,
    /// PEM bundle of additional trusted certificate authorities.
    pub registry_ca_file: Option<PathBuf>,
    /// A tag resolved less than this long ago is not looked up again.
    pub image_tag_fresh_for: Duration,
    /// How long a cached image may be used when its registry is unreachable.
    pub image_stale_if_error: Duration,
    /// Upper bound of the unpacked size of one image.
    pub image_max_bytes: u64,
    /// Image cache budget enforced by [`YoukiRuntime::prune_images`].
    pub image_cache_max_bytes: Option<u64>,
    /// Images unused for this long are removed by `prune_images`.
    pub image_cache_unused_for: Duration,
}

impl Default for YoukiConfig {
    fn default() -> Self {
        Self {
            binary: PathBuf::from("youki"),
            state_dir: PathBuf::new(),
            rootfs: RootfsMode::Auto,
            network: YoukiNetwork::Host,
            cgroup_parent: None,
            limits: Limits::default(),
            insecure_registries: Vec::new(),
            registry_credentials_file: None,
            registry_ca_file: None,
            image_tag_fresh_for: Duration::from_secs(60),
            image_stale_if_error: Duration::from_secs(24 * 3600),
            image_max_bytes: 16 * 1024 * 1024 * 1024,
            image_cache_max_bytes: None,
            image_cache_unused_for: Duration::from_secs(14 * 24 * 3600),
        }
    }
}

/// What start-up found and cleaned up.
#[derive(Debug, Clone, Default)]
pub struct StartupReport {
    /// Containers of a previous (crashed) run that were removed.
    pub stale_containers: usize,
    /// Bundle directories of a previous run that were removed.
    pub stale_bundles: usize,
    /// Abandoned image download areas that were removed.
    pub stale_downloads: usize,
    /// Things worth the operator's attention (features that are not available).
    pub warnings: Vec<String>,
}

/// Why pulling an image did not produce an image.
#[derive(Debug)]
pub enum PullFailure {
    /// The build was cancelled or ran out of time while pulling.
    Stopped(StopReason),
    Failed(String),
}

/// What a step needs to run.
pub struct StepRequest<'a> {
    /// Host directory that becomes the workspace (canonical, UTF-8).
    pub workspace: &'a Path,
    pub shell: &'a str,
    pub script: &'a str,
    /// Variables of the step; they win over the image's.
    pub env: &'a [(String, String)],
}

const POLL: Duration = Duration::from_millis(25);
/// Longest `youki kill`/`delete` is allowed to take.
const CONTROL_TIMEOUT: Duration = Duration::from_secs(15);
/// Cap of the resolv.conf copied from the host.
const MAX_RESOLV_CONF: u64 = 64 * 1024;
/// Open files per container, like Docker's default (bounded by the host's limit).
const NOFILE_TARGET: u64 = 1_048_576;
/// Image use that a size-triggered prune never evicts.
const KEEP_RECENT: Duration = Duration::from_secs(3600);

struct Layout {
    /// `youki --root`
    root: PathBuf,
    bundles: PathBuf,
    images: PathBuf,
}

/// The container runtime. One instance per server; cheap to share.
pub struct YoukiRuntime {
    config: YoukiConfig,
    binary: PathBuf,
    version: String,
    layout: Layout,
    store: ImageStore,
    /// Private runtime for the async registry client; build threads block on it.
    runtime: Option<tokio::runtime::Runtime>,
    rootfs: RootfsMode,
    cgroup: CgroupSetup,
    startup: StartupReport,
    /// Keeps the state directory exclusive to this process.
    _instance_lock: File,
}

impl fmt::Debug for YoukiRuntime {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("YoukiRuntime")
            .field("binary", &self.binary)
            .field("state_dir", &self.config.state_dir)
            .field("rootfs", &self.rootfs)
            .field("network", &self.config.network)
            .finish_non_exhaustive()
    }
}

impl Drop for YoukiRuntime {
    fn drop(&mut self) {
        if let Some(runtime) = self.runtime.take() {
            // dropping a runtime blocks, which is forbidden inside async code
            runtime.shutdown_background();
        }
    }
}

fn io_error(what: &str, path: &Path, error: io::Error) -> String {
    format!("{what} {}: {error}", path.display())
}

fn safe_path(path: &Path) -> bool {
    path.to_str()
        .is_some_and(|text| !text.contains([',', ':', '\\', '\0', '\n']))
}

impl YoukiRuntime {
    /// Verify the host, take the state directory, clean up after a previous
    /// run and open the image cache.
    pub fn new(config: YoukiConfig) -> Result<Arc<Self>, String> {
        let mut config = config;
        if !config.state_dir.is_absolute() {
            return Err(format!(
                "the youki state directory must be an absolute path (got `{}`)",
                config.state_dir.display()
            ));
        }
        if !safe_path(&config.state_dir) {
            return Err(format!(
                "the youki state directory `{}` must not contain `,`, `:` or `\\` (overlay mount options cannot express them)",
                config.state_dir.display()
            ));
        }
        let binary = which::which(&config.binary).map_err(|e| {
            format!(
                "the youki binary `{}` was not found: {e}",
                config.binary.display()
            )
        })?;
        let version = probe::binary_version(&binary)?;

        let layout = Layout {
            root: config.state_dir.join("runtime"),
            bundles: config.state_dir.join("bundles"),
            images: config.state_dir.join("images"),
        };
        for dir in [&config.state_dir, &layout.root, &layout.bundles] {
            fs::DirBuilder::new()
                .recursive(true)
                .mode(0o700)
                .create(dir)
                .map_err(|e| io_error("cannot create", dir, e))?;
        }
        let lock_path = config.state_dir.join("instance.lock");
        let lock = open_lock_file(&lock_path).map_err(|e| io_error("cannot open", &lock_path, e))?;
        if !try_flock(&lock, libc::LOCK_EX).map_err(|e| io_error("cannot lock", &lock_path, e))? {
            return Err(format!(
                "the youki state directory {} is in use by another process; every server instance needs its own",
                config.state_dir.display()
            ));
        }

        let mut warnings = Vec::new();
        probe::user_namespaces()?;

        let own = cgroup::own_cgroup()?;
        let needed = config.limits.needed_controllers();
        let cgroup = cgroup::detect(
            Path::new(cgroup::CGROUP_ROOT),
            &own,
            config.cgroup_parent.as_deref(),
            needed,
        )?;
        cgroup::require(&cgroup, needed)?;
        if !cgroup.can_attach {
            warnings.push(format!(
                "container processes cannot be moved into cgroups below {}: they stay in the server's cgroup \
                 (no per-step accounting; run the server as a systemd service with `Delegate=yes`)",
                cgroup.parent
            ));
        }

        let rootfs = match config.rootfs {
            RootfsMode::Copy => RootfsMode::Copy,
            RootfsMode::Overlay => {
                probe::overlay_mounts(&config.state_dir.join("probe"))?;
                RootfsMode::Overlay
            }
            RootfsMode::Auto => match probe::overlay_mounts(&config.state_dir.join("probe")) {
                Ok(()) => RootfsMode::Overlay,
                Err(reason) => {
                    warnings.push(format!(
                        "falling back to copying images for every step (slow, needs disk space): {reason}"
                    ));
                    RootfsMode::Copy
                }
            },
        };
        config.rootfs = rootfs;

        let mut registry = RegistryOptions {
            insecure_registries: config.insecure_registries.clone(),
            ..RegistryOptions::default()
        };
        if let Some(file) = &config.registry_credentials_file {
            registry.credentials = CredentialStore::new().with_file(file.clone());
        }
        if let Some(file) = &config.registry_ca_file {
            registry.ca_certificates_pem.push(
                fs::read(file).map_err(|e| io_error("cannot read the registry CA bundle", file, e))?,
            );
        }
        let client = RegistryClient::new(registry).map_err(|e| e.to_string())?;
        let store = ImageStore::open(
            layout.images.clone(),
            client,
            Platform::linux_host(),
            StoreOptions {
                limits: UnpackLimits {
                    max_bytes: config.image_max_bytes,
                    ..UnpackLimits::default()
                },
                tag_fresh_for: config.image_tag_fresh_for,
                stale_if_error: config.image_stale_if_error,
                ..StoreOptions::default()
            },
        )
        .map_err(|e| e.to_string())?;
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .thread_name("onedev-oci")
            .enable_all()
            .build()
            .map_err(|e| format!("cannot start the image download runtime: {e}"))?;

        let mut this = Self {
            config,
            binary,
            version,
            layout,
            store,
            runtime: Some(runtime),
            rootfs,
            cgroup,
            startup: StartupReport {
                warnings,
                ..StartupReport::default()
            },
            _instance_lock: lock,
        };
        let warnings = std::mem::take(&mut this.startup.warnings);
        this.startup = this.reap_leftovers(warnings);
        Ok(Arc::new(this))
    }

    /// One line describing the runtime, for the start-up log.
    pub fn describe(&self) -> String {
        format!(
            "{} ({} root filesystems, {} network, cgroup {}{})",
            self.version,
            match self.rootfs {
                RootfsMode::Copy => "copied",
                _ => "overlay",
            },
            match self.config.network {
                YoukiNetwork::Host => "host",
                YoukiNetwork::None => "no",
            },
            self.cgroup.parent,
            if self.cgroup.can_attach { "" } else { ", unattached" }
        )
    }

    pub fn startup_report(&self) -> &StartupReport {
        &self.startup
    }

    pub fn config(&self) -> &YoukiConfig {
        &self.config
    }

    /// The mode root filesystems are really built in (never `Auto`).
    pub fn rootfs_mode(&self) -> RootfsMode {
        self.rootfs
    }

    pub fn image_store(&self) -> &ImageStore {
        &self.store
    }

    fn handle(&self) -> &tokio::runtime::Runtime {
        self.runtime
            .as_ref()
            .expect("the runtime is only taken when the YoukiRuntime is dropped")
    }

    // -- images ---------------------------------------------------------------

    /// Make `image` available as a root filesystem. Blocks until it is there;
    /// `stop` is polled and abandons the download (nothing is left behind).
    pub fn pull(
        &self,
        image: &str,
        stop: &dyn Fn() -> Option<StopReason>,
        events: &(dyn Fn(PullEvent) + Send + Sync),
    ) -> Result<PulledImage, PullFailure> {
        let reference = ImageReference::parse(image).map_err(|e| PullFailure::Failed(e.to_string()))?;
        self.handle().block_on(async {
            let pull = self.store.pull(&reference, events);
            tokio::pin!(pull);
            let mut tick = tokio::time::interval(Duration::from_millis(100));
            loop {
                tokio::select! {
                    result = &mut pull => {
                        return result.map_err(|e| PullFailure::Failed(format!("cannot pull image `{reference}`: {e}")));
                    }
                    _ = tick.tick() => {
                        if let Some(reason) = stop() {
                            // dropping the future cancels the download
                            return Err(PullFailure::Stopped(reason));
                        }
                    }
                }
            }
        })
    }

    /// Remove images that were unused for long and evict the least recently
    /// used ones beyond the size budget. Blocking.
    pub fn prune_images(&self) -> Result<PruneReport, String> {
        self.store
            .prune(&PruneOptions {
                unused_for: self.config.image_cache_unused_for,
                max_total_bytes: self.config.image_cache_max_bytes,
                keep_recent_for: KEEP_RECENT,
            })
            .map_err(|e| e.to_string())
    }

    // -- containers -----------------------------------------------------------

    /// Build the bundle (root filesystem + `config.json`) of one step. The
    /// returned handle removes everything again when dropped.
    pub fn prepare(
        self: &Arc<Self>,
        image: PulledImage,
        request: &StepRequest<'_>,
    ) -> Result<PreparedStep, String> {
        let PulledImage {
            rootfs: image_rootfs,
            config: image_config,
            lease,
            ..
        } = image;
        let workspace = request
            .workspace
            .to_str()
            .ok_or("the workspace path is not valid UTF-8")?;

        let uuid = uuid::Uuid::new_v4().simple().to_string();
        let id = format!("onedev-{uuid}");
        let hostname = uuid[..12].to_owned();
        let dir = self.layout.bundles.join(&id);
        fs::DirBuilder::new()
            .mode(0o700)
            .create(&dir)
            .map_err(|e| io_error("cannot create", &dir, e))?;
        // from here on `step` removes the bundle again, whatever fails
        let mut step = PreparedStep {
            runtime: self.clone(),
            id: id.clone(),
            dir: dir.clone(),
            notes: Vec::new(),
            _lease: lease,
        };

        let resolv_conf = host_resolv_conf();
        let etc = EtcFiles::new(&hostname, resolv_conf);
        let overlays: Vec<OverlayMount> = match self.rootfs {
            RootfsMode::Copy => {
                rootfs::prepare_copy(&image_rootfs, &dir, &etc)
                    .map_err(|e| format!("cannot prepare the root filesystem: {e}"))?;
                Vec::new()
            }
            _ => rootfs::prepare_overlay(&image_rootfs, &dir, &etc)
                .map_err(|e| format!("cannot prepare the root filesystem: {e}"))?,
        };

        let env = spec::merge_env(&image_config.env, &hostname, request.env)?;
        let args = vec![
            request.shell.to_owned(),
            "-c".to_owned(),
            request.script.to_owned(),
        ];
        let cgroups_path = self.cgroup.container_path(&id);
        let document = spec::build(&SpecInput {
            hostname: &hostname,
            uid: current_uid(),
            gid: current_gid(),
            args: &args,
            env: &env,
            cwd: crate::services::executor::CONTAINER_WORKSPACE,
            workspace,
            overlays: &overlays,
            network: self.config.network,
            cgroups_path: &cgroups_path,
            limits: self.config.limits,
            nofile: nofile_limit(),
        });
        let config_path = dir.join("config.json");
        write_private(
            &config_path,
            &serde_json::to_vec_pretty(&document).map_err(|e| e.to_string())?,
        )
        .map_err(|e| io_error("cannot write", &config_path, e))?;

        let user = image_config.user.trim();
        if !matches!(user, "" | "0" | "root" | "0:0" | "root:root" | "0:root" | "root:0") {
            step.notes.push(format!(
                "note: the image's USER `{user}` is ignored; the step runs as root of a single-user namespace \
                 (the server's own user on the host)"
            ));
        }
        Ok(step)
    }

    /// `youki ...` with this runtime's state directory.
    fn control(&self) -> Command {
        let mut command = Command::new(&self.binary);
        command
            .arg("--root")
            .arg(&self.layout.root)
            .stdin(Stdio::null());
        command
    }

    /// Run a control command, bounded in time; its output is discarded.
    fn control_status(&self, args: &[&str]) -> bool {
        let Ok(mut child) = self
            .control()
            .args(args)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
        else {
            return false;
        };
        let deadline = Instant::now() + CONTROL_TIMEOUT;
        loop {
            match child.try_wait() {
                Ok(Some(status)) => return status.success(),
                Ok(None) if Instant::now() < deadline => std::thread::sleep(Duration::from_millis(5)),
                _ => {
                    let _ = child.kill();
                    let _ = child.wait();
                    return false;
                }
            }
        }
    }

    /// Stop a running container: SIGTERM when its init process can receive it
    /// (a PID 1 without a handler silently drops SIGTERM, so asking would
    /// only waste the grace period), SIGKILL when it did not stop in time.
    /// Killing the init process takes the whole process tree of the container
    /// with it.
    pub fn terminate(&self, id: &str, grace: Duration) {
        let pid = read_pid(&self.layout.bundles.join(id).join("init.pid"));
        if let Some(pid) = pid
            && handles_signal(pid, libc::SIGTERM)
        {
            self.control_status(&["kill", id, "TERM"]);
            let deadline = Instant::now() + grace;
            while Instant::now() < deadline && process_exists(pid) {
                std::thread::sleep(POLL);
            }
        }
        self.control_status(&["kill", id, "KILL"]);
        if let Some(pid) = pid {
            let deadline = Instant::now() + Duration::from_secs(3);
            while Instant::now() < deadline && process_exists(pid) {
                std::thread::sleep(POLL);
            }
        }
    }

    /// Forget a container and delete its bundle. Never fails: this runs on
    /// error paths and from `Drop`.
    fn destroy(&self, id: &str, dir: &Path) {
        self.control_status(&["delete", "--force", id]);
        let _ = remove_tree(dir);
    }

    /// Remove what a previous run of the server left behind: containers
    /// (their processes included), bundles and half-finished downloads. Only
    /// safe at start-up, before the first step.
    fn reap_leftovers(&self, warnings: Vec<String>) -> StartupReport {
        let mut report = StartupReport {
            warnings,
            ..StartupReport::default()
        };
        #[derive(serde::Deserialize)]
        struct Listed {
            id: String,
        }
        let listed = self
            .control()
            .args(["list", "--format", "json"])
            .stderr(Stdio::null())
            .output()
            .ok()
            .filter(|output| output.status.success())
            .and_then(|output| serde_json::from_slice::<Vec<Listed>>(&output.stdout).ok())
            .unwrap_or_default();
        for container in listed {
            self.control_status(&["kill", &container.id, "KILL"]);
            self.control_status(&["delete", "--force", &container.id]);
            report.stale_containers += 1;
        }
        if let Ok(entries) = fs::read_dir(&self.layout.bundles) {
            for entry in entries.flatten() {
                if remove_tree(&entry.path()).is_ok() {
                    report.stale_bundles += 1;
                }
            }
        }
        report.stale_downloads = self.store.clean_tmp();
        report
    }
}

/// A step's bundle on disk. Dropping it deletes the container (if it is
/// still around) and the bundle.
pub struct PreparedStep {
    runtime: Arc<YoukiRuntime>,
    id: String,
    dir: PathBuf,
    /// Lines for the step's log (things the user should know about).
    pub notes: Vec<String>,
    /// Keeps the image from being pruned while the step runs.
    _lease: crate::services::oci::ImageLease,
}

impl PreparedStep {
    pub fn id(&self) -> &str {
        &self.id
    }

    /// The runtime executable.
    pub fn program(&self) -> &Path {
        &self.runtime.binary
    }

    /// Arguments that run the container in the foreground. The runtime's own
    /// messages go to a log file in the bundle, so they cannot mix with (or be
    /// forged by) the step's output.
    pub fn arguments(&self) -> Vec<OsString> {
        let mut args: Vec<OsString> = vec!["--root".into(), self.runtime.layout.root.clone().into()];
        args.extend([
            "--log".into(),
            self.dir.join("youki.log").into(),
            "--log-format".into(),
            "json".into(),
            "run".into(),
            "--bundle".into(),
            self.dir.clone().into(),
            "--pid-file".into(),
            self.dir.join("init.pid").into(),
            self.id.clone().into(),
        ]);
        args
    }

    /// Working directory for the runtime process.
    pub fn directory(&self) -> &Path {
        &self.dir
    }

    /// Why the *runtime* (not the step) failed, if it did. The runtime exits
    /// with 255 and logs an error when it cannot create or start the
    /// container (image without a shell, unusable mount, ...); a step that
    /// merely exits with 255 leaves no error in the log.
    pub fn runtime_failure(&self, exit_code: Option<i32>) -> Option<String> {
        if exit_code != Some(255) {
            return None;
        }
        let mut text = String::new();
        File::open(self.dir.join("youki.log"))
            .ok()?
            .take(1024 * 1024)
            .read_to_string(&mut text)
            .ok()?;
        runtime_failure_from_log(&text)
    }
}

impl Drop for PreparedStep {
    fn drop(&mut self) {
        self.runtime.destroy(&self.id, &self.dir);
    }
}

/// The message of the runtime's final error, from its JSON log.
fn runtime_failure_from_log(log: &str) -> Option<String> {
    let mut errors: Vec<(String, String)> = Vec::new();
    for line in log.lines() {
        let Ok(record) = serde_json::from_str::<serde_json::Value>(line) else {
            continue;
        };
        if record["level"] != "ERROR" {
            continue;
        }
        let Some(message) = record["message"].as_str() else {
            continue;
        };
        let target = record["target"].as_str().unwrap_or("").to_owned();
        errors.push((target, message.to_owned()));
    }
    // the top-level error carries the whole chain of causes
    let top = errors
        .iter()
        .rev()
        .find(|(target, _)| target == "youki")
        .map(|(_, message)| message.as_str())
        .map(|message| message.strip_prefix("error in executing command: ").unwrap_or(message))
        .or_else(|| errors.last().map(|(_, message)| message.as_str()))?;
    Some(top.chars().take(2000).collect())
}

// -- small helpers -------------------------------------------------------------

fn current_uid() -> u32 {
    // SAFETY: geteuid(2) has no preconditions and cannot fail.
    unsafe { libc::geteuid() }
}

fn current_gid() -> u32 {
    // SAFETY: getegid(2) has no preconditions and cannot fail.
    unsafe { libc::getegid() }
}

/// Open-file limit for containers: Docker's default, within what the host allows.
fn nofile_limit() -> Option<u64> {
    let mut limit = libc::rlimit {
        rlim_cur: 0,
        rlim_max: 0,
    };
    // SAFETY: getrlimit(2) writes into the struct we own.
    let status = unsafe { libc::getrlimit(libc::RLIMIT_NOFILE, &mut limit) };
    if status != 0 {
        return None;
    }
    let hard = if limit.rlim_max == libc::RLIM_INFINITY {
        NOFILE_TARGET
    } else {
        u64::from(limit.rlim_max).min(NOFILE_TARGET)
    };
    (hard > 0).then_some(hard)
}

/// The host's resolver configuration (the container shares the host's network
/// namespace, so its nameservers are reachable); a public fallback otherwise.
fn host_resolv_conf() -> String {
    let mut text = String::new();
    let read = File::open("/etc/resolv.conf")
        .and_then(|file| file.take(MAX_RESOLV_CONF).read_to_string(&mut text));
    if read.is_ok() && text.lines().any(|line| line.trim_start().starts_with("nameserver")) {
        text
    } else {
        "nameserver 1.1.1.1\nnameserver 8.8.8.8\n".to_owned()
    }
}

fn write_private(path: &Path, bytes: &[u8]) -> io::Result<()> {
    use std::io::Write as _;
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)?;
    file.write_all(bytes)
}

fn read_pid(path: &Path) -> Option<u32> {
    fs::read_to_string(path).ok()?.trim().parse().ok()
}

fn process_exists(pid: u32) -> bool {
    // SAFETY: kill(2) with signal 0 only checks that the process exists.
    let status = unsafe { libc::kill(pid as libc::pid_t, 0) };
    status == 0 || io::Error::last_os_error().raw_os_error() == Some(libc::EPERM)
}

/// Does the process have a handler installed for `signal`? (`SigCgt` of
/// `/proc/<pid>/status`.)
fn handles_signal(pid: u32, signal: libc::c_int) -> bool {
    fs::read_to_string(format!("/proc/{pid}/status"))
        .ok()
        .and_then(|status| caught_signals(&status))
        .is_some_and(|mask| signal >= 1 && mask & (1u64 << (signal - 1)) != 0)
}

fn caught_signals(status: &str) -> Option<u64> {
    let value = status
        .lines()
        .find_map(|line| line.strip_prefix("SigCgt:"))?
        .trim();
    u64::from_str_radix(value, 16).ok()
}

#[cfg(test)]
mod e2e;

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// The youki binary to test with: `ONEDEV_TEST_YOUKI`, one from `PATH` or
    /// the release build of the youki checkout next to this repository on the
    /// BKG hosts; `None` when there is none (tests that need it are skipped).
    pub(crate) fn youki_binary() -> Option<PathBuf> {
        if let Some(path) = std::env::var_os("ONEDEV_TEST_YOUKI") {
            return Some(PathBuf::from(path));
        }
        which::which("youki").ok().or_else(|| {
            let checkout =
                PathBuf::from("/srv/bkg/services/bkg-core-admin/infra/youki/target/release/youki");
            checkout.is_file().then_some(checkout)
        })
    }

    #[test]
    fn the_runtimes_final_error_is_extracted_from_its_json_log() {
        let log = [
            r#"{"timestamp":"t","level":"ERROR","message":"executable for container process not found in PATH","target":"libcontainer::workload::default"}"#,
            r#"{"timestamp":"t","level":"WARN","message":"something minor","target":"libcontainer"}"#,
            r#"{"timestamp":"t","level":"ERROR","message":"error in executing command: failed to create container: exec process failed with error executable 'x' not found in $PATH","target":"youki"}"#,
            "not json at all",
        ]
        .join("\n");
        assert_eq!(
            runtime_failure_from_log(&log).unwrap(),
            "failed to create container: exec process failed with error executable 'x' not found in $PATH"
        );

        // without the top-level record the last error is the best there is
        let partial = r#"{"level":"ERROR","message":"first","target":"a"}
{"level":"ERROR","message":"second","target":"b"}"#;
        assert_eq!(runtime_failure_from_log(partial).unwrap(), "second");
        assert_eq!(runtime_failure_from_log(""), None);
        assert_eq!(runtime_failure_from_log(r#"{"level":"INFO","message":"fine"}"#), None);
        assert_eq!(runtime_failure_from_log("garbage"), None);
    }

    #[test]
    fn caught_signals_are_read_from_proc_status() {
        let status = "Name:\tsh\nSigBlk:\t0000000000000000\nSigIgn:\t0000000000001004\nSigCgt:\t0000000000004002\n";
        let mask = caught_signals(status).unwrap();
        assert_eq!(mask, 0x4002);
        // bit 14 is SIGTERM (15), bit 1 is SIGINT (2)
        assert!(mask & (1 << (libc::SIGTERM - 1)) != 0);
        assert!(mask & (1 << (libc::SIGINT - 1)) != 0);
        assert!(mask & (1 << (libc::SIGHUP - 1)) == 0);
        assert_eq!(caught_signals("Name:\tx\n"), None);
        assert_eq!(caught_signals("SigCgt:\tzzzz\n"), None);
    }

    #[test]
    fn the_signal_handling_of_a_real_process_can_be_asked() {
        // this very test process: the harness installs handlers for some
        // signals but SIGWINCH is ignored/default, SIGSEGV is typically caught
        // by the Rust runtime
        let own = std::process::id();
        assert!(handles_signal(own, libc::SIGSEGV) || !handles_signal(own, libc::SIGSEGV));
        assert!(!handles_signal(u32::MAX - 1, libc::SIGTERM), "no such process");
        assert!(process_exists(own));
        assert!(!process_exists(u32::MAX - 1));
    }

    #[test]
    fn the_nofile_limit_is_within_what_the_host_allows() {
        let limit = nofile_limit().unwrap();
        assert!(limit >= 1 && limit <= NOFILE_TARGET);
    }

    #[test]
    fn resolver_configuration_always_has_a_nameserver() {
        let text = host_resolv_conf();
        assert!(text.lines().any(|line| line.trim_start().starts_with("nameserver")), "{text}");
    }

    #[test]
    fn invalid_configurations_are_refused_before_anything_is_touched() {
        let relative = YoukiConfig {
            state_dir: PathBuf::from("relative/dir"),
            ..YoukiConfig::default()
        };
        assert!(YoukiRuntime::new(relative).unwrap_err().contains("absolute"));

        let comma = YoukiConfig {
            state_dir: PathBuf::from("/tmp/a,b"),
            ..YoukiConfig::default()
        };
        assert!(YoukiRuntime::new(comma).unwrap_err().contains("must not contain"));

        let missing = YoukiConfig {
            binary: PathBuf::from("/definitely/not/youki"),
            state_dir: PathBuf::from("/tmp/onedev-never-created"),
            ..YoukiConfig::default()
        };
        assert!(YoukiRuntime::new(missing).unwrap_err().contains("was not found"));
        assert!(!Path::new("/tmp/onedev-never-created").exists());
    }
}
