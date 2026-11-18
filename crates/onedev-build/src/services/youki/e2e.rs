//! End-to-end tests of the youki executor: real containers, started by the
//! real `youki` binary, from an image that is pulled from the in-process
//! registry mock and run through the real [`JobExecutor`].
//!
//! The tests need a youki binary (`ONEDEV_TEST_YOUKI` or `PATH`), a statically
//! linked `busybox` (the only program the test image contains), user
//! namespaces and a cgroup the test process may create children in. When
//! the host lacks one of them the tests report why and pass without running;
//! `ONEDEV_REQUIRE_YOUKI_TESTS=1` turns that into a failure.
//!
//! Where to run containers is chosen like in production: the cgroup of the
//! test process, or `ONEDEV_TEST_YOUKI_CGROUP_PARENT`. Resource limits need a
//! delegated cgroup, e.g. `systemd-run --user --scope -p Delegate=yes -- cargo test ...`.

use std::collections::HashMap;
use std::fs;
use std::os::unix::fs::MetadataExt;
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::time::{Duration, Instant};

use super::spec::Limits;
use super::{PullFailure, RootfsMode, StepRequest, YoukiConfig, YoukiNetwork, YoukiRuntime};
use crate::models::{
    BuildStatus, ExecuteCondition, Interpreter, JobSpec, LogLine, Step, StepResult,
};
use crate::services::executor::{
    CancelToken, ExecutorConfig, ExecutorMode, JobContext, JobExecutor, JobOutcome, LogSink,
    StopReason,
};
use crate::services::interpolation::{SecretMasker, Variables};
use crate::services::oci::testing::{MockRegistry, TarEntry, TestImage, tar_bytes};

const APPLETS: &[&str] = &[
    "sh", "ash", "cat", "echo", "id", "ls", "stat", "mkdir", "touch", "rm", "sleep", "hostname",
    "env", "pwd", "test", "[", "kill", "ps", "wc", "grep", "head", "tail", "uname", "mount",
    "date", "tr", "readlink", "cp", "mv", "chmod", "ln", "seq", "df", "ip", "ifconfig", "dd",
    "yes", "cut", "sed", "awk", "xargs", "timeout", "true", "false",
];

/// A statically linked busybox of the host: a whole userland in one file.
fn static_busybox() -> Option<Vec<u8>> {
    for path in ["/usr/bin/busybox", "/bin/busybox"] {
        let Ok(bytes) = fs::read(path) else { continue };
        if is_static_elf(&bytes) {
            return Some(bytes);
        }
    }
    None
}

/// ELF64 (little endian) without a `PT_INTERP` program header.
fn is_static_elf(bytes: &[u8]) -> bool {
    if bytes.len() < 64 || &bytes[..4] != b"\x7fELF" || bytes[4] != 2 || bytes[5] != 1 {
        return false;
    }
    let phoff = u64::from_le_bytes(bytes[32..40].try_into().unwrap()) as usize;
    let phentsize = u16::from_le_bytes(bytes[54..56].try_into().unwrap()) as usize;
    let phnum = u16::from_le_bytes(bytes[56..58].try_into().unwrap()) as usize;
    (0..phnum).all(|index| {
        let at = phoff + index * phentsize;
        bytes
            .get(at..at + 4)
            .is_some_and(|kind| u32::from_le_bytes(kind.try_into().unwrap()) != 3)
    })
}

fn image_layer(busybox: &[u8]) -> Vec<u8> {
    let mut entries = vec![
        TarEntry::dir("bin/", 0o755),
        TarEntry::file("bin/busybox", 0o755, busybox.to_vec()),
        TarEntry::dir("etc/", 0o755),
        TarEntry::file("etc/passwd", 0o644, "root:x:0:0:root:/root:/bin/sh\n"),
        TarEntry::file("etc/group", 0o644, "root:x:0:\n"),
        TarEntry::dir("tmp/", 0o1777),
        TarEntry::dir("root/", 0o700),
        TarEntry::dir("usr/", 0o755),
        TarEntry::dir("usr/share/", 0o755),
        TarEntry::file("usr/share/greeting", 0o644, "hello from the image\n"),
        TarEntry::dir("var/", 0o755),
        TarEntry::dir("var/lib/", 0o755),
    ];
    for applet in APPLETS {
        entries.push(TarEntry::symlink(&format!("bin/{applet}"), "busybox"));
    }
    tar_bytes(&entries)
}

fn require_tests() -> bool {
    std::env::var("ONEDEV_REQUIRE_YOUKI_TESTS").is_ok_and(|value| value == "1")
}

fn skip<T>(reason: String) -> Option<T> {
    assert!(
        !require_tests(),
        "the youki end-to-end tests are required but cannot run: {reason}"
    );
    eprintln!("skipping the youki end-to-end test: {reason}");
    None
}

/// A registry with the test image, a youki runtime and a workspace.
struct World {
    registry: MockRegistry,
    runtime: Arc<YoukiRuntime>,
    image: String,
    work: tempfile::TempDir,
    /// Removed when the world is dropped.
    _state: tempfile::TempDir,
}

fn world(rootfs: RootfsMode, network: YoukiNetwork, limits: Limits) -> Option<World> {
    let Some(binary) = super::tests::youki_binary() else {
        return skip("no youki binary (set ONEDEV_TEST_YOUKI or put youki in PATH)".to_owned());
    };
    let Some(busybox) = static_busybox() else {
        return skip("no statically linked busybox on this host".to_owned());
    };
    let registry = MockRegistry::start();
    TestImage::with_layers(vec![image_layer(&busybox)]).push(&registry, "e2e/busybox", "1");
    let state = tempfile::tempdir().unwrap();
    let runtime = match YoukiRuntime::new(YoukiConfig {
        binary,
        state_dir: state.path().join("youki"),
        rootfs,
        network,
        limits,
        cgroup_parent: std::env::var("ONEDEV_TEST_YOUKI_CGROUP_PARENT").ok(),
        insecure_registries: vec![registry.host()],
        ..YoukiConfig::default()
    }) {
        Ok(runtime) => runtime,
        Err(reason) => return skip(format!("this host cannot run youki containers: {reason}")),
    };
    let image = format!("{}/e2e/busybox:1", registry.host());
    Some(World {
        registry,
        runtime,
        image,
        work: tempfile::tempdir().unwrap(),
        _state: state,
    })
}

macro_rules! overlay_world {
    () => {
        match world(RootfsMode::Overlay, YoukiNetwork::Host, Limits::default()) {
            Some(world) => world,
            None => return,
        }
    };
}

fn command_step(name: &str, commands: &str) -> Step {
    Step {
        step_type: "CommandStep".into(),
        name: Some(name.into()),
        condition: ExecuteCondition::Successful,
        optional: false,
        run_in_container: None,
        image: None,
        interpreter: Some(Interpreter {
            interpreter_type: "PosixInterpreter".into(),
            shell: "sh".into(),
            powershell: None,
            commands: commands.into(),
        }),
        run_as: None,
        use_tty: None,
        run: None,
        with_lfs: None,
        with_submodules: None,
        clone_depth: None,
        checkout_path: None,
        source_path: None,
        artifacts: None,
        template_name: None,
        extra: Default::default(),
    }
}

fn job(steps: Vec<Step>) -> JobSpec {
    JobSpec {
        name: "j".into(),
        execute_in_sandbox: false,
        job_executor: None,
        sandbox_template: None,
        steps,
        param_specs: vec![],
        job_dependencies: vec![],
        project_dependencies: vec![],
        required_services: vec![],
        triggers: vec![],
        timeout: 3600,
        post_build_actions: vec![],
        sequential_group: None,
        retry_condition: "never".into(),
        max_attempts: 1,
        retry_delay: 0,
        env: HashMap::new(),
    }
}

struct Run {
    outcome: JobOutcome,
    log: Vec<LogLine>,
}

impl Run {
    fn step(&self, index: usize) -> &StepResult {
        &self.outcome.steps[index]
    }

    fn stdout(&self) -> Vec<&str> {
        self.log
            .iter()
            .filter(|line| !line.stderr)
            .map(|line| line.line.as_str())
            .collect()
    }

    fn stderr(&self) -> Vec<&str> {
        self.log
            .iter()
            .filter(|line| line.stderr)
            .map(|line| line.line.as_str())
            .collect()
    }

    fn has(&self, needle: &str) -> bool {
        self.log.iter().any(|line| line.line.contains(needle))
    }

    /// The value of the first `<key>: <value>` output line.
    fn value(&self, key: &str) -> Option<&str> {
        self.stdout()
            .into_iter()
            .find_map(|line| line.strip_prefix(key)?.strip_prefix(": "))
    }
}

impl World {
    fn executor_config(&self, default_image: Option<String>) -> ExecutorConfig {
        ExecutorConfig {
            mode: ExecutorMode::Youki,
            youki: Some(self.runtime.clone()),
            default_image,
            term_grace: Duration::from_secs(2),
            ..ExecutorConfig::default()
        }
    }

    fn exec(
        &self,
        job: &JobSpec,
        cancel: &CancelToken,
        deadline: Option<Instant>,
        default_image: Option<String>,
    ) -> Run {
        let (tx, rx) = flume::unbounded();
        let config = self.executor_config(default_image);
        let variables = Variables::new();
        let ctx = JobContext {
            work_dir: self.work.path(),
            artifact_dir: None,
            log: LogSink::new(Some(tx), SecretMasker::default()),
            cancel,
            deadline,
            checkout: None,
            variables: &variables,
            config: &config,
            network: None,
            hooks: None,
        };
        let outcome = JobExecutor.execute(job, &job.steps, &ctx).unwrap();
        drop(ctx);
        Run {
            outcome,
            log: rx.try_iter().collect(),
        }
    }

    fn run(&self, job: &JobSpec) -> Run {
        self.exec(job, &CancelToken::new(), None, Some(self.image.clone()))
    }

    fn script(&self, script: &str) -> Run {
        self.run(&job(vec![command_step("s", script)]))
    }

    fn containers(&self) -> Vec<String> {
        let output = self
            .runtime
            .control()
            .args(["list", "--format", "json"])
            .output()
            .expect("youki list");
        serde_json::from_slice::<Vec<serde_json::Value>>(&output.stdout)
            .unwrap_or_default()
            .iter()
            .filter_map(|container| container["id"].as_str().map(str::to_owned))
            .collect()
    }

    fn bundles(&self) -> usize {
        fs::read_dir(&self.runtime.layout.bundles).unwrap().count()
    }

    fn assert_nothing_left_behind(&self) {
        assert_eq!(self.containers(), Vec::<String>::new(), "containers");
        assert_eq!(self.bundles(), 0, "bundle directories");
    }
}

/// Is a process whose command line contains `needle` running (on the host)?
fn host_process_running(needle: &str) -> bool {
    let Ok(pgrep) = which::which("pgrep") else {
        return false;
    };
    Command::new(pgrep)
        .args(["-f", needle])
        .stdout(Stdio::null())
        .status()
        .is_ok_and(|status| status.success())
}

fn wait_for(path: &Path, timeout: Duration) {
    let deadline = Instant::now() + timeout;
    while !path.exists() {
        assert!(Instant::now() < deadline, "{} never appeared", path.display());
        std::thread::sleep(Duration::from_millis(20));
    }
}

fn run_with_cancel_after_start(world: &World, script: &str, marker: &str) -> (Run, Duration) {
    let cancel = CancelToken::new();
    let started = world.work.path().join(marker);
    let step = command_step("long", &format!("touch {marker}\n{script}"));
    let job = job(vec![step]);
    let begin = Instant::now();
    let run = std::thread::scope(|scope| {
        scope.spawn(|| {
            wait_for(&started, Duration::from_secs(60));
            cancel.cancel();
        });
        world.exec(&job, &cancel, None, Some(world.image.clone()))
    });
    (run, begin.elapsed())
}

// -----------------------------------------------------------------------------
// the basics
// -----------------------------------------------------------------------------

#[test]
fn a_step_runs_in_a_container_and_streams_its_output() {
    let world = overlay_world!();
    let run = world.script(
        "echo \"greeting: $(cat /usr/share/greeting)\"\n\
         echo \"cwd: $(pwd)\"\n\
         echo \"ids: $(id -u) $(id -g)\"\n\
         echo \"hostname: $(hostname)\"\n\
         echo \"pid: $$\"\n\
         echo \"to-stderr\" >&2",
    );
    assert_eq!(run.step(0).status, BuildStatus::Success, "{:?}", run.log);
    assert_eq!(run.step(0).exit_code, Some(0));
    assert_eq!(run.value("greeting"), Some("hello from the image"));
    assert_eq!(run.value("cwd"), Some("/onedev-build/workspace"));
    assert_eq!(run.value("ids"), Some("0 0"));
    assert_eq!(run.value("pid"), Some("1"), "the step is the init process of its own pid namespace");
    let hostname = run.value("hostname").unwrap();
    assert_eq!(hostname.len(), 12, "{hostname}");
    assert!(hostname.chars().all(|c| c.is_ascii_hexdigit()), "{hostname}");
    assert!(run.stderr().contains(&"to-stderr"), "{:?}", run.stderr());
    // the progress of the pull is part of the job log
    assert!(run.has("pulling image"), "{:?}", run.log);
    assert!(run.has("image unpacked"), "{:?}", run.log);
    world.assert_nothing_left_behind();
}

#[test]
fn the_workspace_is_shared_and_its_files_belong_to_the_server_user() {
    let world = overlay_world!();
    fs::write(world.work.path().join("input.txt"), "from the host\n").unwrap();
    let run = world.script(
        "cat input.txt > copy.txt\n\
         echo created > new.txt\n\
         mkdir -p sub/dir && touch sub/dir/f",
    );
    assert_eq!(run.step(0).status, BuildStatus::Success, "{:?}", run.log);
    assert_eq!(
        fs::read_to_string(world.work.path().join("copy.txt")).unwrap(),
        "from the host\n"
    );
    // SAFETY: geteuid(2) has no preconditions.
    let (uid, gid) = unsafe { (libc::geteuid(), libc::getegid()) };
    for file in ["copy.txt", "new.txt", "sub", "sub/dir/f"] {
        let meta = fs::metadata(world.work.path().join(file)).unwrap();
        assert_eq!((meta.uid(), meta.gid()), (uid, gid), "{file}");
    }
}

#[test]
fn exit_codes_are_the_exit_codes_of_the_steps() {
    let world = overlay_world!();
    let failing = world.script("echo before\nexit 3");
    assert_eq!(failing.step(0).status, BuildStatus::Failed);
    assert_eq!(failing.step(0).exit_code, Some(3));
    assert!(failing.outcome.failed);

    // 255 is also what the runtime itself exits with when it fails: a step
    // that merely exits with it is a failed step, not a broken runtime
    let step_255 = world.script("exit 255");
    assert_eq!(step_255.step(0).status, BuildStatus::Failed);
    assert_eq!(step_255.step(0).exit_code, Some(255));
    assert!(
        !step_255.has("container runtime failed"),
        "{:?}",
        step_255.log
    );

    // `set -e`: the first failing command ends the step
    let set_e = world.script("false\necho not-reached");
    assert_eq!(set_e.step(0).status, BuildStatus::Failed);
    // (the log also echoes the script itself, so look at real output lines)
    assert!(!set_e.stdout().contains(&"not-reached"), "{:?}", set_e.log);

    // a child killed by a signal fails the step with the usual 128 + signal
    let killed = world.script("sh -c 'kill -9 $$'");
    assert_eq!(killed.step(0).status, BuildStatus::Failed);
    assert_eq!(killed.step(0).exit_code, Some(137));
    world.assert_nothing_left_behind();
}

#[test]
fn the_environment_merges_the_image_the_job_and_the_build() {
    let world = overlay_world!();
    let mut spec = job(vec![command_step("env", "echo \"vars: $FROM_IMAGE $FOO $HOME $CI $ONEDEV_JOB_NAME $ONEDEV_STEP_NAME\"\necho \"path: $PATH\"\necho \"hostname: $HOSTNAME $(hostname)\"")]);
    spec.env.insert("FOO".into(), "bar".into());
    let run = world.run(&spec);
    assert_eq!(run.step(0).status, BuildStatus::Success, "{:?}", run.log);
    assert_eq!(run.value("vars"), Some("yes bar /root true j env"));
    assert_eq!(run.value("path"), Some("/usr/local/bin:/usr/bin:/bin"));
    let hostnames: Vec<&str> = run.value("hostname").unwrap().split(' ').collect();
    assert_eq!(hostnames[0], hostnames[1], "$HOSTNAME is the container's hostname");
}

#[test]
fn the_container_looks_like_a_container() {
    let world = overlay_world!();
    let run = world.script(
        "echo \"tmp: $(stat -c %a /tmp)\"\n\
         echo \"root: $(stat -c %a /root)\"\n\
         echo \"hostfile: $(cat /etc/hostname)\"\n\
         grep -q '^nameserver' /etc/resolv.conf\n\
         grep -q localhost /etc/hosts\n\
         grep -q '^root:x:0:0' /etc/passwd\n\
         test -d /proc/1\n\
         test -c /dev/null\n\
         test -c /dev/urandom\n\
         echo \"zero: $(head -c 4 /dev/zero | wc -c)\"\n\
         echo \"caps: $(grep CapEff /proc/self/status | cut -f2)\"\n\
         echo \"nonewprivs: $(grep NoNewPrivs /proc/self/status | cut -f2)\"\n\
         echo \"uidmap: $(awk '{print $1, $2}' /proc/self/uid_map)\"\n\
         echo \"processes: $(ls /proc | grep -c '^[0-9]')\"",
    );
    assert_eq!(run.step(0).status, BuildStatus::Success, "{:?}", run.log);
    assert_eq!(run.value("tmp"), Some("1777"));
    assert_eq!(run.value("root"), Some("700"));
    assert_eq!(run.value("hostfile").map(str::len), Some(12));
    assert_eq!(run.value("zero"), Some("4"));
    // Docker's default capability set, and nothing a setuid binary could add
    assert_eq!(run.value("caps"), Some("00000000a80425fb"));
    assert_eq!(run.value("nonewprivs"), Some("1"));
    // one id maps to root: the server's user
    // SAFETY: geteuid(2) has no preconditions.
    let uid = unsafe { libc::geteuid() };
    assert_eq!(run.value("uidmap"), Some(format!("0 {uid}").as_str()));
    // a private pid namespace: just the shell and the commands it started
    let processes: usize = run.value("processes").unwrap().parse().unwrap();
    assert!(processes <= 5, "{processes}");
}

#[test]
fn steps_can_use_a_job_image_or_name_their_own() {
    let world = overlay_world!();
    let mut own_image = command_step("own", "cat /usr/share/greeting");
    own_image.image = Some(world.image.clone());
    let spec = job(vec![own_image]);
    // no default image configured: the step's own image is used
    let run = world.exec(&spec, &CancelToken::new(), None, None);
    assert_eq!(run.step(0).status, BuildStatus::Success, "{:?}", run.log);
    assert!(run.stdout().contains(&"hello from the image"));

    // neither: an explicit error, not a guess
    let run = world.exec(
        &job(vec![command_step("none", "true")]),
        &CancelToken::new(),
        None,
        None,
    );
    assert_eq!(run.step(0).status, BuildStatus::Failed);
    assert!(
        run.has("the step names no image and no default image is configured"),
        "{:?}",
        run.log
    );
}

// -----------------------------------------------------------------------------
// isolation of steps from each other and from the image cache
// -----------------------------------------------------------------------------

fn fresh_root_filesystem(mode: RootfsMode) {
    let Some(world) = world(mode, YoukiNetwork::Host, Limits::default()) else {
        return;
    };
    let run = world.run(&job(vec![
        command_step(
            "pollute",
            "echo leftover > /tmp/marker\n\
             echo changed > /usr/share/greeting\n\
             echo added > /usr/share/new\n\
             mkdir /var/lib/created\n\
             echo hacked > /etc/passwd\n\
             rm /bin/ls",
        ),
        command_step(
            "check",
            "test ! -e /tmp/marker\n\
             test ! -e /usr/share/new\n\
             test ! -e /var/lib/created\n\
             test \"$(cat /usr/share/greeting)\" = 'hello from the image'\n\
             grep -q '^root:x:0:0' /etc/passwd\n\
             test -e /bin/ls\n\
             echo ok",
        ),
    ]));
    assert_eq!(run.step(0).status, BuildStatus::Success, "{:?}", run.log);
    assert_eq!(run.step(1).status, BuildStatus::Success, "{:?}", run.log);

    // the cached image itself was never touched
    let pulled = world
        .runtime
        .pull(&world.image, &|| None, &|_| {})
        .map_err(|_| "pull")
        .unwrap();
    assert!(!pulled.rootfs.join("tmp/marker").exists());
    assert!(!pulled.rootfs.join("usr/share/new").exists());
    assert_eq!(
        fs::read_to_string(pulled.rootfs.join("usr/share/greeting")).unwrap(),
        "hello from the image\n"
    );
    assert!(pulled.rootfs.join("bin/ls").exists());
    world.assert_nothing_left_behind();
}

#[test]
fn every_step_gets_a_fresh_overlay_root_filesystem() {
    fresh_root_filesystem(RootfsMode::Overlay);
}

#[test]
fn every_step_gets_a_fresh_copied_root_filesystem() {
    fresh_root_filesystem(RootfsMode::Copy);
}

#[test]
fn the_automatic_mode_picks_what_the_host_supports() {
    let Some(world) = world(RootfsMode::Auto, YoukiNetwork::Host, Limits::default()) else {
        return;
    };
    assert_ne!(world.runtime.rootfs_mode(), RootfsMode::Auto);
    let run = world.script("echo works");
    assert_eq!(run.step(0).status, BuildStatus::Success, "{:?}", run.log);
    assert!(world.runtime.describe().contains("youki"));
}

#[test]
fn the_image_is_pulled_once_and_then_served_from_the_cache() {
    let world = overlay_world!();
    let first = world.script("true");
    assert_eq!(first.step(0).status, BuildStatus::Success);
    assert!(first.has("pulling image"));
    world.registry.clear_requests();

    let second = world.script("true");
    assert_eq!(second.step(0).status, BuildStatus::Success);
    assert!(second.has("found in the cache"), "{:?}", second.log);
    assert!(!second.has("pulling image"));
    let blob_requests: Vec<_> = world
        .registry
        .requests()
        .into_iter()
        .filter(|request| request.path.contains("/blobs/"))
        .collect();
    assert!(blob_requests.is_empty(), "{blob_requests:?}");
}

// -----------------------------------------------------------------------------
// stopping steps
// -----------------------------------------------------------------------------

#[test]
fn cancelling_stops_the_container_and_leaves_nothing_behind() {
    let world = overlay_world!();
    let (run, took) = run_with_cancel_after_start(&world, "sleep 4711", "started");
    assert_eq!(run.step(0).status, BuildStatus::Cancelled, "{:?}", run.log);
    assert!(run.has("step cancelled"));
    assert!(took < Duration::from_secs(30), "{took:?}");
    world.assert_nothing_left_behind();
    assert!(!host_process_running("sleep 4711"), "the step's process survived");
}

#[test]
fn a_step_past_the_deadline_is_stopped() {
    let world = overlay_world!();
    // the first pull and the container start must fit into the budget
    let deadline = Instant::now() + Duration::from_secs(4);
    let begin = Instant::now();
    let run = world.exec(
        &job(vec![command_step("slow", "touch started\nsleep 4712")]),
        &CancelToken::new(),
        Some(deadline),
        Some(world.image.clone()),
    );
    assert_eq!(run.step(0).status, BuildStatus::Failed, "{:?}", run.log);
    assert!(run.outcome.timed_out);
    assert!(run.has("time limit exceeded"), "{:?}", run.log);
    assert!(begin.elapsed() < Duration::from_secs(30));
    world.assert_nothing_left_behind();
    assert!(!host_process_running("sleep 4712"));
}

#[test]
fn a_step_that_catches_sigterm_but_keeps_running_is_killed_after_the_grace_period() {
    let world = overlay_world!();
    // PID 1 of the container is the shell. With a handler installed SIGTERM
    // reaches it (and is answered), but the loop carries on like a badly
    // behaved service: the runtime has to follow up with SIGKILL.
    let (run, took) = run_with_cancel_after_start(
        &world,
        "trap 'echo got-sigterm' TERM\nwhile true; do sleep 1; done # 4713",
        "started",
    );
    assert_eq!(run.step(0).status, BuildStatus::Cancelled, "{:?}", run.log);
    assert!(run.stdout().contains(&"got-sigterm"), "{:?}", run.log);
    assert!(took < Duration::from_secs(30), "{took:?}");
    world.assert_nothing_left_behind();
    assert!(!host_process_running("# 4713"));
}

#[test]
fn background_processes_do_not_outlive_their_step() {
    let world = overlay_world!();
    let run = world.script("sleep 4715 &\necho started");
    assert_eq!(run.step(0).status, BuildStatus::Success, "{:?}", run.log);
    // the pid namespace dies with its init process, taking the background job along
    for _ in 0..50 {
        if !host_process_running("sleep 4715") {
            break;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    assert!(!host_process_running("sleep 4715"));
    world.assert_nothing_left_behind();
}

#[test]
fn pulling_an_image_can_be_cancelled() {
    let world = overlay_world!();
    // an image that is not cached and whose layer download hangs
    let busybox = static_busybox().unwrap();
    let image = TestImage::with_layers(vec![image_layer(&busybox)]).with_marker("stalls");
    let pushed = image.push(&world.registry, "e2e/stalled", "1");
    world.registry.stall_blob(&pushed.layer_digests[0]);
    let reference = format!("{}/e2e/stalled:1", world.registry.host());

    let cancel = CancelToken::new();
    let begin = Instant::now();
    let result = std::thread::scope(|scope| {
        scope.spawn(|| {
            std::thread::sleep(Duration::from_millis(800));
            cancel.cancel();
        });
        world.runtime.pull(&reference, &|| cancel.reason(), &|_| {})
    });
    assert!(
        matches!(result, Err(PullFailure::Stopped(StopReason::Cancelled))),
        "{:?}",
        result.err()
    );
    assert!(begin.elapsed() < Duration::from_secs(20));

    // through the executor the step ends as cancelled, too
    let cancel = CancelToken::new();
    let mut step = command_step("pull", "true");
    step.image = Some(reference);
    let job = job(vec![step]);
    let run = std::thread::scope(|scope| {
        scope.spawn(|| {
            std::thread::sleep(Duration::from_millis(800));
            cancel.cancel();
        });
        world.exec(&job, &cancel, None, None)
    });
    assert_eq!(run.step(0).status, BuildStatus::Cancelled, "{:?}", run.log);
    world.assert_nothing_left_behind();
}

// -----------------------------------------------------------------------------
// failures are explained
// -----------------------------------------------------------------------------

#[test]
fn a_shell_the_image_does_not_have_is_a_runtime_failure_with_the_reason() {
    let world = overlay_world!();
    let mut step = command_step("noshell", "true");
    step.interpreter.as_mut().unwrap().shell = "nonexistent".into();
    let run = world.run(&job(vec![step]));
    assert_eq!(run.step(0).status, BuildStatus::Failed, "{:?}", run.log);
    assert!(run.has("cannot run step: container runtime failed:"), "{:?}", run.log);
    assert!(run.has("nonexistent"), "{:?}", run.log);
    world.assert_nothing_left_behind();
}

#[test]
fn images_that_cannot_be_pulled_fail_the_step_with_the_reason() {
    let world = overlay_world!();
    let mut missing = command_step("missing", "true");
    missing.image = Some(format!("{}/e2e/missing:1", world.registry.host()));
    let run = world.exec(&job(vec![missing]), &CancelToken::new(), None, None);
    assert_eq!(run.step(0).status, BuildStatus::Failed);
    assert!(run.has("cannot pull image"), "{:?}", run.log);

    let mut invalid = command_step("invalid", "true");
    invalid.image = Some("not a valid reference".into());
    let run = world.exec(&job(vec![invalid]), &CancelToken::new(), None, None);
    assert_eq!(run.step(0).status, BuildStatus::Failed);
    assert!(run.has("invalid image reference"), "{:?}", run.log);
    world.assert_nothing_left_behind();
}

#[test]
fn what_single_user_containers_cannot_do_is_refused_up_front() {
    let world = overlay_world!();

    let mut other_user = command_step("other", "true");
    other_user.run_as = Some("1000".into());
    let run = world.run(&job(vec![other_user]));
    assert_eq!(run.step(0).status, BuildStatus::Failed);
    assert!(
        run.has("runAs `1000` is not supported by the youki executor"),
        "{:?}",
        run.log
    );

    for root in ["root", "0", "0:0", "root:root"] {
        let mut as_root = command_step("root", "id -u");
        as_root.run_as = Some(root.into());
        let run = world.run(&job(vec![as_root]));
        assert_eq!(run.step(0).status, BuildStatus::Success, "{root}: {:?}", run.log);
    }

    let mut on_host = command_step("host", "true");
    on_host.run_in_container = Some(false);
    let run = world.run(&job(vec![on_host]));
    assert_eq!(run.step(0).status, BuildStatus::Failed);
    assert!(run.has("runInContainer: false"), "{:?}", run.log);

    let mut with_services = job(vec![command_step("svc", "true")]);
    with_services.required_services = vec!["db".into()];
    let run = world.run(&with_services);
    assert_eq!(run.step(0).status, BuildStatus::Failed);
    assert!(
        run.has("job services (db) are not supported by the youki executor"),
        "{:?}",
        run.log
    );
    world.assert_nothing_left_behind();
}

#[test]
fn an_image_user_is_ignored_with_a_note() {
    let world = overlay_world!();
    let mut image = TestImage::with_layers(vec![image_layer(&static_busybox().unwrap())]);
    image.user = "1000".into();
    image.push(&world.registry, "e2e/user", "1");
    let mut step = command_step("user", "id -u");
    step.image = Some(format!("{}/e2e/user:1", world.registry.host()));
    let run = world.exec(&job(vec![step]), &CancelToken::new(), None, None);
    assert_eq!(run.step(0).status, BuildStatus::Success, "{:?}", run.log);
    assert!(run.stdout().contains(&"0"));
    assert!(
        run.stderr()
            .iter()
            .any(|line| line.contains("the image's USER `1000` is ignored")),
        "{:?}",
        run.log
    );
}

// -----------------------------------------------------------------------------
// the network, resource limits, the runtime itself
// -----------------------------------------------------------------------------

#[test]
fn the_host_network_is_shared_by_default() {
    let world = overlay_world!();
    let run = world.script("echo \"interfaces: $(ls /sys/class/net | tr '\\n' ' ')\"");
    assert_eq!(run.step(0).status, BuildStatus::Success, "{:?}", run.log);
    let host: Vec<String> = fs::read_dir("/sys/class/net")
        .unwrap()
        .flatten()
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .collect();
    let seen = run.value("interfaces").unwrap();
    for interface in host {
        assert!(seen.split(' ').any(|name| name == interface), "{interface} in {seen}");
    }
}

#[test]
fn without_a_network_the_step_sees_no_interface_but_loopback() {
    let Some(world) = world(RootfsMode::Overlay, YoukiNetwork::None, Limits::default()) else {
        return;
    };
    let run = world.script(
        "echo \"interfaces: $(ls /sys/class/net | tr '\\n' ' ')\"\n\
         echo \"lo: $(cat /sys/class/net/lo/operstate)\"",
    );
    assert_eq!(run.step(0).status, BuildStatus::Success, "{:?}", run.log);
    assert_eq!(run.value("interfaces"), Some("lo "));
    // documented limitation: nobody brings the loopback interface up
    assert_eq!(run.value("lo"), Some("down"));
}

#[test]
fn resource_limits_are_programmed_into_the_containers_cgroup() {
    let limits = Limits {
        memory_bytes: Some(64 << 20),
        cpus: None,
        pids: Some(100),
    };
    let Some(world) = world(RootfsMode::Overlay, YoukiNetwork::Host, limits) else {
        return;
    };
    let run = world.script(
        "echo \"memory: $(cat /sys/fs/cgroup/memory.max)\"\n\
         echo \"pids: $(cat /sys/fs/cgroup/pids.max)\"",
    );
    assert_eq!(run.step(0).status, BuildStatus::Success, "{:?}", run.log);
    assert_eq!(run.value("memory"), Some("67108864"));
    assert_eq!(run.value("pids"), Some("100"));
    world.assert_nothing_left_behind();
}

#[test]
fn limits_that_the_host_cannot_enforce_stop_the_start_up() {
    // cpu is not delegated on typical user sessions; whatever the host
    // offers, a limit that cannot be enforced must not be silently ignored
    let Some(binary) = super::tests::youki_binary() else {
        return;
    };
    let state = tempfile::tempdir().unwrap();
    let probe = YoukiRuntime::new(YoukiConfig {
        binary: binary.clone(),
        state_dir: state.path().join("youki"),
        limits: Limits {
            memory_bytes: Some(1 << 30),
            cpus: Some(1.0),
            pids: Some(100),
        },
        cgroup_parent: std::env::var("ONEDEV_TEST_YOUKI_CGROUP_PARENT").ok(),
        ..YoukiConfig::default()
    });
    match probe {
        Ok(runtime) => {
            // the host delegates everything: the limits really apply
            assert!(runtime.cgroup.controllers.contains("cpu"));
            assert!(runtime.cgroup.can_attach);
        }
        Err(reason) => assert!(
            reason.contains("cgroup") || reason.contains("user namespaces") || reason.contains("overlay"),
            "{reason}"
        ),
    }
}

#[test]
fn a_state_directory_belongs_to_one_server_at_a_time() {
    let world = overlay_world!();
    let second = YoukiRuntime::new(YoukiConfig {
        binary: world.runtime.binary.clone(),
        state_dir: world.runtime.config().state_dir.clone(),
        insecure_registries: vec![world.registry.host()],
        ..YoukiConfig::default()
    });
    let error = second.unwrap_err();
    assert!(error.contains("in use by another process"), "{error}");
}

#[test]
fn leftovers_of_a_crashed_server_are_removed() {
    let world = overlay_world!();
    let pulled = world.runtime.pull(&world.image, &|| None, &|_| {}).ok().unwrap();
    let workspace = world.work.path().canonicalize().unwrap();
    let prepared = world
        .runtime
        .prepare(
            pulled,
            &StepRequest {
                workspace: &workspace,
                shell: "sh",
                script: "touch started; sleep 4716",
                env: &[],
            },
        )
        .unwrap();
    // a container that nobody supervises any more
    let mut orphan = Command::new(prepared.program())
        .args(prepared.arguments())
        .current_dir(prepared.directory())
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    wait_for(&workspace.join("started"), Duration::from_secs(60));
    assert_eq!(world.containers().len(), 1);
    assert_eq!(world.bundles(), 1);

    let report = world.runtime.reap_leftovers(vec!["kept".to_owned()]);
    assert_eq!(report.stale_containers, 1);
    assert_eq!(report.stale_bundles, 1);
    assert_eq!(report.warnings, ["kept"]);
    world.assert_nothing_left_behind();
    let status = orphan.wait().unwrap();
    assert!(!status.success());
    assert!(!host_process_running("sleep 4716"));
    drop(prepared);
}

#[test]
fn concurrent_steps_do_not_get_in_each_others_way() {
    let world = overlay_world!();
    // warm the cache so that the steps below race on containers, not on the pull
    assert_eq!(world.script("true").step(0).status, BuildStatus::Success);
    std::thread::scope(|scope| {
        let handles: Vec<_> = (0..4)
            .map(|index| {
                let world = &world;
                scope.spawn(move || {
                    let run = world.run(&job(vec![command_step(
                        "c",
                        &format!("echo {index} > /tmp/mine; sleep 1; test \"$(cat /tmp/mine)\" = {index}; hostname"),
                    )]));
                    (run.step(0).status, run.stdout().last().map(|s| (*s).to_owned()))
                })
            })
            .collect();
        let mut hostnames = Vec::new();
        for handle in handles {
            let (status, hostname) = handle.join().unwrap();
            assert_eq!(status, BuildStatus::Success);
            hostnames.push(hostname.unwrap());
        }
        hostnames.sort();
        hostnames.dedup();
        assert_eq!(hostnames.len(), 4, "every container has a hostname of its own");
    });
    world.assert_nothing_left_behind();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn steps_run_from_blocking_tasks_of_an_async_server() {
    let Some(world) = world(RootfsMode::Overlay, YoukiNetwork::Host, Limits::default()) else {
        return;
    };
    let world = Arc::new(world);
    let inner = world.clone();
    let run = tokio::task::spawn_blocking(move || inner.script("echo from-blocking-task"))
        .await
        .unwrap();
    assert_eq!(run.step(0).status, BuildStatus::Success, "{:?}", run.log);
    assert!(run.stdout().contains(&"from-blocking-task"));
    // dropping the runtime inside async code must not panic
    drop(world);
}
