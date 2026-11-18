//! Start-up checks of the youki runtime: is everything there that rootless
//! containers need, and which of the optional features does this host allow?
//!
//! Every probe returns a human-readable reason on failure so that a
//! misconfigured host is explained at start-up and not by an obscure error in
//! the first build.

use std::fs;
use std::path::Path;
use std::process::{Command, Stdio};

/// Version line of the runtime binary (`youki version: 0.7.0`).
pub fn binary_version(binary: &Path) -> Result<String, String> {
    let output = Command::new(binary)
        .arg("--version")
        .stdin(Stdio::null())
        .output()
        .map_err(|e| format!("cannot run the youki binary {}: {e}", binary.display()))?;
    if !output.status.success() {
        return Err(format!(
            "{} --version failed ({}): {}",
            binary.display(),
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    let text = String::from_utf8_lossy(&output.stdout);
    let line = text.lines().next().unwrap_or("").trim();
    if !line.to_ascii_lowercase().contains("youki") {
        return Err(format!(
            "{} does not look like youki (--version printed `{line}`)",
            binary.display()
        ));
    }
    Ok(line.to_owned())
}

fn read_sysctl(path: &str) -> Option<String> {
    fs::read_to_string(path).ok().map(|text| text.trim().to_owned())
}

/// Can this process create a user namespace and become root in it?
pub fn user_namespaces() -> Result<(), String> {
    if read_sysctl("/proc/sys/user/max_user_namespaces").as_deref() == Some("0") {
        return Err(
            "user namespaces are disabled (sysctl user.max_user_namespaces is 0); rootless containers need them"
                .to_owned(),
        );
    }
    if read_sysctl("/proc/sys/kernel/unprivileged_userns_clone").as_deref() == Some("0") {
        return Err(
            "unprivileged user namespaces are disabled (set sysctl kernel.unprivileged_userns_clone=1)"
                .to_owned(),
        );
    }
    let Ok(unshare) = which::which("unshare") else {
        // nothing to probe with; the runtime reports the problem itself
        return Ok(());
    };
    let output = Command::new(unshare)
        .args(["--user", "--map-root-user", "true"])
        .stdin(Stdio::null())
        .output()
        .map_err(|e| format!("cannot run unshare: {e}"))?;
    if output.status.success() {
        return Ok(());
    }
    let mut message = format!(
        "this user cannot create user namespaces: {}",
        String::from_utf8_lossy(&output.stderr).trim()
    );
    if read_sysctl("/proc/sys/kernel/apparmor_restrict_unprivileged_userns").as_deref() == Some("1") {
        message.push_str(
            "; AppArmor restricts unprivileged user namespaces (set sysctl \
             kernel.apparmor_restrict_unprivileged_userns=0 or give the runtime an AppArmor profile)",
        );
    }
    Err(message)
}

/// Does the kernel allow overlay mounts inside a user namespace, and can the
/// state directory's filesystem back them? The probe mounts an overlay,
/// modifies a lower file (copy-up), creates and deletes entries (whiteout).
pub fn overlay_mounts(scratch: &Path) -> Result<(), String> {
    let (Ok(unshare), Ok(_mount)) = (which::which("unshare"), which::which("mount")) else {
        return Err("`unshare` and `mount` (util-linux) are needed to test overlay support".to_owned());
    };
    fs::create_dir_all(scratch).map_err(|e| format!("cannot create {}: {e}", scratch.display()))?;
    let script = r#"set -e
d="$PROBE_DIR"
mkdir -p "$d/lower/sub" "$d/upper" "$d/work" "$d/merged"
echo lower > "$d/lower/sub/file"
mount -t overlay overlay -o "lowerdir=$d/lower,upperdir=$d/upper,workdir=$d/work,userxattr" "$d/merged"
echo more >> "$d/merged/sub/file"
echo new > "$d/merged/created"
mkdir "$d/merged/sub/dir"
rm "$d/merged/sub/file"
test ! -e "$d/merged/sub/file"
test -f "$d/merged/created"
"#;
    let output = Command::new(unshare)
        .args(["--user", "--map-root-user", "--mount", "--", "sh", "-c", script])
        .env("PROBE_DIR", scratch)
        .stdin(Stdio::null())
        .output();
    // the writable layer holds directories with unusual modes
    let _ = crate::services::oci::store::remove_tree(scratch);
    let output = output.map_err(|e| format!("cannot run unshare: {e}"))?;
    if output.status.success() {
        Ok(())
    } else {
        Err(format!(
            "the kernel or the filesystem of {} does not support unprivileged overlay mounts: {}",
            scratch.parent().unwrap_or(scratch).display(),
            String::from_utf8_lossy(&output.stderr).trim()
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn something_that_is_not_youki_is_recognised() {
        let error = binary_version(Path::new("/bin/echo")).unwrap_err();
        assert!(error.contains("does not look like youki") || error.contains("failed"), "{error}");
        let error = binary_version(Path::new("/definitely/not/here")).unwrap_err();
        assert!(error.contains("cannot run the youki binary"), "{error}");
    }

    #[test]
    fn the_real_runtime_reports_its_version_when_present() {
        let Some(binary) = crate::services::youki::tests::youki_binary() else {
            return;
        };
        let version = binary_version(&binary).unwrap();
        assert!(version.to_ascii_lowercase().contains("youki"), "{version}");
    }

    #[test]
    fn the_probes_never_panic_and_leave_nothing_behind() {
        let dir = tempfile::tempdir().unwrap();
        let scratch = dir.path().join("probe");
        // success or failure depends on the machine; both are fine
        let _ = user_namespaces();
        let _ = overlay_mounts(&scratch);
        assert!(!scratch.exists(), "the scratch directory is removed");
    }
}
