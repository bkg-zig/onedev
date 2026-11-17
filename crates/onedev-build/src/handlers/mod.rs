//! Framework-independent entry points for running builds.

use std::path::Path;

use crate::errors::BuildResult;
use crate::models::{BuildReport, LogLine, TriggerEvent};
use crate::services::parser::parse_buildspec_file;
use crate::services::runner::BuildRunner;

/// Load `.onedev-buildspec.yml` in `work_dir` and run it for `event`.
pub fn run_buildspec(
    work_dir: &Path,
    artifact_root: &Path,
    event: &TriggerEvent,
    log: Option<flume::Sender<LogLine>>,
) -> BuildResult<BuildReport> {
    let spec = parse_buildspec_file(&work_dir.join(".onedev-buildspec.yml"))?;
    BuildRunner::new().run(&spec, work_dir, artifact_root, event, log)
}
