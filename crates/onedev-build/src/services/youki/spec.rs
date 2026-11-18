//! The OCI runtime-spec `config.json` of a step container.
//!
//! Built as plain JSON so the result can be asserted on in unit tests
//! without a runtime. The shape follows `youki spec --rootless` with the
//! changes a build step needs:
//!
//! * a writable root (the root filesystem is a throw-away copy/overlay),
//! * the workspace bind-mounted at [`CONTAINER_WORKSPACE`],
//! * Docker's default capability set and `no-new-privileges`,
//! * an absolute `cgroupsPath` below the delegated cgroup parent and
//!   `linux.resources` only when limits are configured (a rootless runtime
//!   cannot program the device controller, so `resources.devices` must not
//!   appear),
//! * a single uid/gid mapping: the server's own user is container root.

use serde_json::{Value, json};

use super::YoukiNetwork;
use super::rootfs::OverlayMount;
use crate::services::executor::CONTAINER_WORKSPACE;

/// `PATH` of images that do not set one (what Docker uses).
pub const DEFAULT_PATH: &str = "/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin";

/// Capabilities `docker run` grants by default.
pub const DEFAULT_CAPABILITIES: [&str; 14] = [
    "CAP_CHOWN",
    "CAP_DAC_OVERRIDE",
    "CAP_FSETID",
    "CAP_FOWNER",
    "CAP_MKNOD",
    "CAP_NET_RAW",
    "CAP_SETGID",
    "CAP_SETUID",
    "CAP_SETFCAP",
    "CAP_SETPCAP",
    "CAP_NET_BIND_SERVICE",
    "CAP_SYS_CHROOT",
    "CAP_KILL",
    "CAP_AUDIT_WRITE",
];

const MASKED_PATHS: [&str; 10] = [
    "/proc/acpi",
    "/proc/asound",
    "/proc/kcore",
    "/proc/keys",
    "/proc/latency_stats",
    "/proc/timer_list",
    "/proc/timer_stats",
    "/proc/sched_debug",
    "/sys/firmware",
    "/proc/scsi",
];

const READONLY_PATHS: [&str; 5] = [
    "/proc/bus",
    "/proc/fs",
    "/proc/irq",
    "/proc/sys",
    "/proc/sysrq-trigger",
];

/// CPU scheduling period of `linux.resources.cpu` (microseconds).
const CPU_PERIOD: u64 = 100_000;

/// Resource limits of a container.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Limits {
    pub memory_bytes: Option<u64>,
    /// Number of CPUs (`1.5` = one and a half).
    pub cpus: Option<f64>,
    pub pids: Option<u64>,
}

impl Limits {
    pub fn needed_controllers(&self) -> super::cgroup::Needed {
        super::cgroup::Needed {
            memory: self.memory_bytes.is_some(),
            cpu: self.cpus.is_some(),
            pids: self.pids.is_some(),
        }
    }

    /// Limits written the way `docker run --memory/--cpus/--pids-limit` takes
    /// them (that is how the server is configured).
    pub fn from_docker_style(
        memory: Option<&str>,
        cpus: Option<&str>,
        pids: Option<u32>,
    ) -> Result<Self, String> {
        Ok(Self {
            memory_bytes: memory.map(parse_memory).transpose()?,
            cpus: cpus.map(parse_cpus).transpose()?,
            pids: pids.map(u64::from).filter(|pids| *pids > 0),
        })
    }
}

/// A memory size as `docker run --memory` accepts it: a number, optionally
/// with a decimal point, and an optional unit `b`, `k`, `m`, `g` or `t`
/// (binary multiples; a trailing `b` as in `4gb` is allowed; any case).
pub fn parse_memory(text: &str) -> Result<u64, String> {
    let invalid = || format!("invalid memory size `{text}` (expected a number with an optional unit b, k, m, g or t, e.g. 4g)");
    let lower = text.trim().to_ascii_lowercase();
    let split = lower
        .find(|c: char| !c.is_ascii_digit() && c != '.')
        .unwrap_or(lower.len());
    let (number, unit) = lower.split_at(split);
    let factor: u64 = match unit.trim() {
        "" | "b" => 1,
        "k" | "kb" => 1 << 10,
        "m" | "mb" => 1 << 20,
        "g" | "gb" => 1 << 30,
        "t" | "tb" => 1 << 40,
        _ => return Err(invalid()),
    };
    let bytes = if number.contains('.') {
        let value: f64 = number.parse().map_err(|_| invalid())?;
        let bytes = value * factor as f64;
        if !bytes.is_finite() || bytes >= u64::MAX as f64 {
            return Err(invalid());
        }
        bytes as u64
    } else {
        number
            .parse::<u64>()
            .map_err(|_| invalid())?
            .checked_mul(factor)
            .ok_or_else(invalid)?
    };
    if bytes == 0 {
        return Err(format!("the memory limit `{text}` must be greater than zero"));
    }
    Ok(bytes)
}

/// A CPU count as `docker run --cpus` takes it (`2`, `0.5`).
pub fn parse_cpus(text: &str) -> Result<f64, String> {
    match text.trim().parse::<f64>() {
        Ok(cpus) if cpus.is_finite() && cpus > 0.0 => Ok(cpus),
        _ => Err(format!(
            "invalid cpu count `{text}` (expected a positive number such as 2 or 0.5)"
        )),
    }
}

/// Everything a container's `config.json` depends on.
pub struct SpecInput<'a> {
    pub hostname: &'a str,
    /// Host ids that become container root.
    pub uid: u32,
    pub gid: u32,
    /// `process.args`.
    pub args: &'a [String],
    /// `NAME=value` entries.
    pub env: &'a [String],
    /// Working directory inside the container.
    pub cwd: &'a str,
    /// Host directory mounted at [`CONTAINER_WORKSPACE`].
    pub workspace: &'a str,
    /// Overlay mounts that make up the root filesystem (empty when the root
    /// filesystem is a plain copy).
    pub overlays: &'a [OverlayMount],
    pub network: YoukiNetwork,
    /// Absolute `linux.cgroupsPath`.
    pub cgroups_path: &'a str,
    pub limits: Limits,
    /// Open-file limit of the container process (soft = hard).
    pub nofile: Option<u64>,
}

/// Merge environments: the image's, then the defaults a container engine adds
/// (`PATH`, `HOME` unless the image has them, `HOSTNAME` always), then the
/// step's own, which wins. Later values replace earlier ones in place.
pub fn merge_env(
    image: &[String],
    hostname: &str,
    step: &[(String, String)],
) -> Result<Vec<String>, String> {
    let mut entries: Vec<(String, String)> = Vec::new();
    fn set(entries: &mut Vec<(String, String)>, key: &str, value: &str) {
        match entries.iter_mut().find(|(existing, _)| existing == key) {
            Some(entry) => entry.1 = value.to_owned(),
            None => entries.push((key.to_owned(), value.to_owned())),
        }
    }
    for entry in image {
        if let Some((key, value)) = entry.split_once('=')
            && !key.is_empty()
            && !entry.contains('\0')
        {
            set(&mut entries, key, value);
        }
    }
    if !entries.iter().any(|(key, _)| key == "PATH") {
        set(&mut entries, "PATH", DEFAULT_PATH);
    }
    if !entries.iter().any(|(key, _)| key == "HOME") {
        set(&mut entries, "HOME", "/root");
    }
    set(&mut entries, "HOSTNAME", hostname);
    for (key, value) in step {
        if key.is_empty() || key.contains('=') || key.contains('\0') || value.contains('\0') {
            return Err(format!("invalid environment variable `{key}`"));
        }
        set(&mut entries, key, value);
    }
    Ok(entries
        .into_iter()
        .map(|(key, value)| format!("{key}={value}"))
        .collect())
}

fn base_mounts(network: YoukiNetwork) -> Vec<Value> {
    // With the host's network namespace the host's /sys is the only coherent
    // view; with a namespace of its own a fresh sysfs is needed (the host's
    // would show the host's interfaces).
    let sys = match network {
        YoukiNetwork::Host => json!({
            "destination": "/sys",
            "type": "none",
            "source": "/sys",
            "options": ["rbind", "nosuid", "noexec", "nodev", "ro"],
        }),
        YoukiNetwork::None => json!({
            "destination": "/sys",
            "type": "sysfs",
            "source": "sysfs",
            "options": ["nosuid", "noexec", "nodev", "ro"],
        }),
    };
    vec![
        json!({"destination": "/proc", "type": "proc", "source": "proc", "options": []}),
        json!({
            "destination": "/dev",
            "type": "tmpfs",
            "source": "tmpfs",
            "options": ["nosuid", "strictatime", "mode=755", "size=65536k"],
        }),
        json!({
            "destination": "/dev/pts",
            "type": "devpts",
            "source": "devpts",
            "options": ["nosuid", "noexec", "newinstance", "ptmxmode=0666", "mode=0620"],
        }),
        json!({
            "destination": "/dev/shm",
            "type": "tmpfs",
            "source": "shm",
            "options": ["nosuid", "noexec", "nodev", "mode=1777", "size=65536k"],
        }),
        json!({
            "destination": "/dev/mqueue",
            "type": "mqueue",
            "source": "mqueue",
            "options": ["nosuid", "noexec", "nodev"],
        }),
        sys,
        json!({
            "destination": "/sys/fs/cgroup",
            "type": "cgroup",
            "source": "cgroup",
            "options": ["nosuid", "noexec", "nodev", "relatime", "ro"],
        }),
    ]
}

fn resources(limits: &Limits) -> Option<Value> {
    let mut resources = serde_json::Map::new();
    if let Some(bytes) = limits.memory_bytes {
        resources.insert("memory".into(), json!({ "limit": bytes }));
    }
    if let Some(cpus) = limits.cpus {
        let quota = (cpus * CPU_PERIOD as f64).round().max(1000.0) as i64;
        resources.insert("cpu".into(), json!({ "quota": quota, "period": CPU_PERIOD }));
    }
    if let Some(pids) = limits.pids {
        resources.insert("pids".into(), json!({ "limit": pids }));
    }
    (!resources.is_empty()).then(|| Value::Object(resources))
}

/// Assemble the `config.json` document.
pub fn build(input: &SpecInput<'_>) -> Value {
    let mut mounts: Vec<Value> = input.overlays.iter().map(OverlayMount::to_json).collect();
    mounts.extend(base_mounts(input.network));
    mounts.push(json!({
        "destination": CONTAINER_WORKSPACE,
        "type": "bind",
        "source": input.workspace,
        "options": ["rbind", "rw"],
    }));

    let mut namespaces = vec![
        json!({"type": "pid"}),
        json!({"type": "ipc"}),
        json!({"type": "uts"}),
        json!({"type": "mount"}),
        json!({"type": "cgroup"}),
        json!({"type": "user"}),
    ];
    if input.network == YoukiNetwork::None {
        namespaces.push(json!({"type": "network"}));
    }

    let capabilities: Vec<&str> = DEFAULT_CAPABILITIES.to_vec();
    let mut process = json!({
        "terminal": false,
        "user": {"uid": 0, "gid": 0},
        "args": input.args,
        "env": input.env,
        "cwd": input.cwd,
        "capabilities": {
            "bounding": capabilities,
            "effective": capabilities,
            "permitted": capabilities,
        },
        "noNewPrivileges": true,
    });
    if let Some(limit) = input.nofile {
        process["rlimits"] = json!([{"type": "RLIMIT_NOFILE", "hard": limit, "soft": limit}]);
    }

    let mut linux = json!({
        "uidMappings": [{"hostID": input.uid, "containerID": 0, "size": 1}],
        "gidMappings": [{"hostID": input.gid, "containerID": 0, "size": 1}],
        "namespaces": namespaces,
        "cgroupsPath": input.cgroups_path,
        "maskedPaths": MASKED_PATHS,
        "readonlyPaths": READONLY_PATHS,
    });
    if let Some(resources) = resources(&input.limits) {
        linux["resources"] = resources;
    }

    json!({
        "ociVersion": "1.0.2",
        "root": {"path": "rootfs", "readonly": false},
        "hostname": input.hostname,
        "mounts": mounts,
        "process": process,
        "linux": linux,
    })
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    fn overlay(name: &str) -> OverlayMount {
        OverlayMount {
            destination: format!("/{name}"),
            lower: PathBuf::from(format!("/images/abc/rootfs/{name}")),
            upper: PathBuf::from(format!("/bundles/x/upper/{name}")),
            work: PathBuf::from(format!("/bundles/x/work/{name}")),
        }
    }

    fn input<'a>(
        args: &'a [String],
        env: &'a [String],
        overlays: &'a [OverlayMount],
    ) -> SpecInput<'a> {
        SpecInput {
            hostname: "0123456789ab",
            uid: 1000,
            gid: 1001,
            args,
            env,
            cwd: CONTAINER_WORKSPACE,
            workspace: "/srv/work/space",
            overlays,
            network: YoukiNetwork::Host,
            cgroups_path: "/system.slice/onedev.service/onedev-x",
            limits: Limits::default(),
            nofile: None,
        }
    }

    #[test]
    fn the_basic_document_matches_what_the_runtime_expects() {
        let args = vec!["sh".to_owned(), "-c".to_owned(), "echo hi".to_owned()];
        let env = vec!["PATH=/bin".to_owned()];
        let overlays = [overlay("usr"), overlay("etc")];
        let spec = build(&input(&args, &env, &overlays));

        assert_eq!(spec["root"], json!({"path": "rootfs", "readonly": false}));
        assert_eq!(spec["hostname"], "0123456789ab");
        assert_eq!(spec["process"]["args"], json!(["sh", "-c", "echo hi"]));
        assert_eq!(spec["process"]["env"], json!(["PATH=/bin"]));
        assert_eq!(spec["process"]["cwd"], CONTAINER_WORKSPACE);
        assert_eq!(spec["process"]["terminal"], false);
        assert_eq!(spec["process"]["noNewPrivileges"], true);
        assert_eq!(spec["process"]["user"], json!({"uid": 0, "gid": 0}));
        assert_eq!(spec["process"]["capabilities"]["bounding"].as_array().unwrap().len(), 14);
        assert!(
            !spec["process"]["capabilities"]["effective"]
                .to_string()
                .contains("CAP_SYS_ADMIN"),
            "no administrative capabilities"
        );
        assert!(spec["process"].get("rlimits").is_none());

        assert_eq!(spec["linux"]["uidMappings"], json!([{"hostID": 1000, "containerID": 0, "size": 1}]));
        assert_eq!(spec["linux"]["gidMappings"], json!([{"hostID": 1001, "containerID": 0, "size": 1}]));
        assert_eq!(spec["linux"]["cgroupsPath"], "/system.slice/onedev.service/onedev-x");
        let types: Vec<&str> = spec["linux"]["namespaces"]
            .as_array()
            .unwrap()
            .iter()
            .map(|ns| ns["type"].as_str().unwrap())
            .collect();
        assert_eq!(types, ["pid", "ipc", "uts", "mount", "cgroup", "user"]);
        assert!(spec["linux"].get("resources").is_none(), "no limits, no resources (and never devices)");
    }

    #[test]
    fn mounts_are_ordered_overlays_first_and_the_workspace_last() {
        let overlays = [overlay("usr"), overlay("etc")];
        let spec = build(&input(&[], &[], &overlays));
        let destinations: Vec<&str> = spec["mounts"]
            .as_array()
            .unwrap()
            .iter()
            .map(|m| m["destination"].as_str().unwrap())
            .collect();
        assert_eq!(
            destinations,
            [
                "/usr",
                "/etc",
                "/proc",
                "/dev",
                "/dev/pts",
                "/dev/shm",
                "/dev/mqueue",
                "/sys",
                "/sys/fs/cgroup",
                CONTAINER_WORKSPACE
            ]
        );
        let workspace = spec["mounts"].as_array().unwrap().last().unwrap();
        assert_eq!(workspace["type"], "bind");
        assert_eq!(workspace["source"], "/srv/work/space");
        assert_eq!(workspace["options"], json!(["rbind", "rw"]));

        let usr = &spec["mounts"][0];
        assert_eq!(usr["type"], "overlay");
        assert_eq!(usr["source"], "overlay");
        assert_eq!(
            usr["options"],
            json!([
                "lowerdir=/images/abc/rootfs/usr",
                "upperdir=/bundles/x/upper/usr",
                "workdir=/bundles/x/work/usr",
                "userxattr"
            ])
        );

        let copy_mode = build(&input(&[], &[], &[]));
        assert_eq!(copy_mode["mounts"][0]["destination"], "/proc");
    }

    #[test]
    fn the_host_network_shares_sys_and_none_isolates_it() {
        let host = build(&input(&[], &[], &[]));
        let sys = host["mounts"]
            .as_array()
            .unwrap()
            .iter()
            .find(|m| m["destination"] == "/sys")
            .unwrap();
        assert_eq!(sys["type"], "none");
        assert!(sys["options"].as_array().unwrap().contains(&json!("rbind")));

        let mut isolated = input(&[], &[], &[]);
        isolated.network = YoukiNetwork::None;
        let spec = build(&isolated);
        let sys = spec["mounts"]
            .as_array()
            .unwrap()
            .iter()
            .find(|m| m["destination"] == "/sys")
            .unwrap();
        assert_eq!(sys["type"], "sysfs", "a fresh sysfs shows the container's interfaces");
        assert!(
            spec["linux"]["namespaces"]
                .as_array()
                .unwrap()
                .contains(&json!({"type": "network"}))
        );
    }

    #[test]
    fn limits_become_cgroup_resources_without_a_device_policy() {
        let mut with_limits = input(&[], &[], &[]);
        with_limits.limits = Limits {
            memory_bytes: Some(512 * 1024 * 1024),
            cpus: Some(1.5),
            pids: Some(2048),
        };
        with_limits.nofile = Some(65536);
        let spec = build(&with_limits);
        assert_eq!(
            spec["linux"]["resources"],
            json!({
                "memory": {"limit": 536870912u64},
                "cpu": {"quota": 150000, "period": 100000},
                "pids": {"limit": 2048}
            })
        );
        assert!(spec["linux"]["resources"].get("devices").is_none());
        assert_eq!(
            spec["process"]["rlimits"],
            json!([{"type": "RLIMIT_NOFILE", "hard": 65536, "soft": 65536}])
        );

        let only_pids = Limits {
            pids: Some(10),
            ..Limits::default()
        };
        assert_eq!(resources(&only_pids).unwrap(), json!({"pids": {"limit": 10}}));
        assert!(resources(&Limits::default()).is_none());
        // absurdly small CPU shares stay schedulable
        let tiny = Limits {
            cpus: Some(0.0001),
            ..Limits::default()
        };
        assert_eq!(resources(&tiny).unwrap()["cpu"]["quota"], 1000);
    }

    #[test]
    fn needed_controllers_follow_the_limits() {
        let needed = Limits {
            memory_bytes: Some(1),
            cpus: None,
            pids: Some(1),
        }
        .needed_controllers();
        assert!(needed.memory && needed.pids && !needed.cpu);
        assert!(!Limits::default().needed_controllers().any());
    }

    #[test]
    fn environments_merge_with_the_step_winning() {
        let image = vec![
            "PATH=/opt/bin:/bin".to_owned(),
            "JAVA_HOME=/opt/jdk".to_owned(),
            "HOSTNAME=from-image".to_owned(),
            "BROKEN".to_owned(),
            "=novalue".to_owned(),
        ];
        let step = vec![
            ("JAVA_HOME".to_owned(), "/custom/jdk".to_owned()),
            ("CI".to_owned(), "true".to_owned()),
        ];
        let merged = merge_env(&image, "hostname1", &step).unwrap();
        assert_eq!(
            merged,
            [
                "PATH=/opt/bin:/bin",
                "JAVA_HOME=/custom/jdk",
                "HOSTNAME=hostname1",
                "HOME=/root",
                "CI=true"
            ]
        );

        // images without PATH get the usual default; an image HOME is kept
        let merged = merge_env(&["HOME=/home/app".to_owned()], "h", &[]).unwrap();
        assert_eq!(
            merged,
            [
                "HOME=/home/app".to_owned(),
                format!("PATH={DEFAULT_PATH}"),
                "HOSTNAME=h".to_owned()
            ]
        );
    }

    #[test]
    fn memory_sizes_follow_docker_notation() {
        assert_eq!(parse_memory("4g").unwrap(), 4 << 30);
        assert_eq!(parse_memory("4G").unwrap(), 4 << 30);
        assert_eq!(parse_memory("512m").unwrap(), 512 << 20);
        assert_eq!(parse_memory("512MB").unwrap(), 512 << 20);
        assert_eq!(parse_memory("1024k").unwrap(), 1024 << 10);
        assert_eq!(parse_memory(" 2gb ").unwrap(), 2 << 30);
        assert_eq!(parse_memory("1t").unwrap(), 1 << 40);
        assert_eq!(parse_memory("123456").unwrap(), 123456);
        assert_eq!(parse_memory("100b").unwrap(), 100);
        assert_eq!(parse_memory("1.5g").unwrap(), 3 << 29);
        assert_eq!(parse_memory("0.5m").unwrap(), 1 << 19);
        for bad in ["", "g", "-1g", "1x", "1 0g", "0", "0g", "1..5g", "99999999999999999999", "18446744073709551615g", "1e3"] {
            assert!(parse_memory(bad).is_err(), "`{bad}` must be rejected");
        }
    }

    #[test]
    fn cpu_counts_must_be_positive_numbers() {
        assert_eq!(parse_cpus("2").unwrap(), 2.0);
        assert_eq!(parse_cpus(" 0.5 ").unwrap(), 0.5);
        for bad in ["", "0", "-1", "abc", "NaN", "inf", "1,5"] {
            assert!(parse_cpus(bad).is_err(), "`{bad}` must be rejected");
        }
    }

    #[test]
    fn docker_style_limits_are_translated_as_a_whole() {
        let limits = Limits::from_docker_style(Some("2g"), Some("1.5"), Some(512)).unwrap();
        assert_eq!(
            limits,
            Limits {
                memory_bytes: Some(2 << 30),
                cpus: Some(1.5),
                pids: Some(512)
            }
        );
        assert_eq!(Limits::from_docker_style(None, None, None).unwrap(), Limits::default());
        // zero pids means "no limit" rather than "no process may start"
        assert_eq!(Limits::from_docker_style(None, None, Some(0)).unwrap().pids, None);
        let error = Limits::from_docker_style(Some("lots"), None, None).unwrap_err();
        assert!(error.contains("invalid memory size"), "{error}");
        assert!(Limits::from_docker_style(None, Some("x"), None).is_err());
    }

    #[test]
    fn malformed_step_variables_are_rejected() {
        for (key, value) in [("", "x"), ("A=B", "x"), ("A\0B", "x"), ("A", "x\0y")] {
            let error = merge_env(&[], "h", &[(key.to_owned(), value.to_owned())]).unwrap_err();
            assert!(error.contains("invalid environment variable"), "{error}");
        }
    }
}
