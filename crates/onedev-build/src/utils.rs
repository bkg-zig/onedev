//! Small helpers for the build layer.

use std::path::Path;

/// First existing build-spec file in a checkout: `.onedev-buildspec.yml`
/// (vanilla name) with `.yaml` accepted as well.
pub fn find_buildspec(work_dir: &Path) -> Option<std::path::PathBuf> {
    for name in [".onedev-buildspec.yml", ".onedev-buildspec.yaml"] {
        let candidate = work_dir.join(name);
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    None
}
