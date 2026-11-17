use onedev_build::handlers::run_buildspec;
use onedev_build::models::{BuildStatus, TriggerEvent};

const SPEC: &str = r#"
version: 1
jobs:
  - name: build
    steps:
      - name: compile
        run: mkdir -p target && printf 'binary' > target/app && echo built
    artifacts: [target/app]
    triggers:
      - type: push
        branches: [main]
  - name: test
    needs: [build]
    steps:
      - run: test -f target/app && echo tested
"#;

fn seeded(work: &std::path::Path) -> std::path::PathBuf {
    std::fs::write(work.join(".onedev-buildspec.yml"), SPEC).unwrap();
    work.join("artifacts")
}

#[test]
fn example_job_runs_green_and_collects_artifacts() {
    let root = tempfile::tempdir().unwrap();
    let artifacts = seeded(root.path());
    let (tx, rx) = flume::unbounded();

    let report = run_buildspec(
        root.path(),
        &artifacts,
        &TriggerEvent::Push { branch: "main".into() },
        Some(tx),
    )
    .unwrap();

    assert_eq!(report.status, BuildStatus::Success);
    assert_eq!(report.jobs.len(), 2);
    assert!(report.jobs.iter().all(|j| j.status == BuildStatus::Success));
    assert_eq!(
        artifacts.join("build/target/app").exists(),
        true,
        "artifact must be collected"
    );

    let logs: Vec<String> = rx.try_iter().map(|l| l.line).collect();
    assert!(logs.iter().any(|l| l == "built"), "logs: {logs:?}");
    assert!(logs.iter().any(|l| l == "tested"), "logs: {logs:?}");
}

#[test]
fn non_matching_trigger_selects_no_jobs() {
    let root = tempfile::tempdir().unwrap();
    let artifacts = seeded(root.path());

    let report = run_buildspec(
        root.path(),
        &artifacts,
        &TriggerEvent::Push { branch: "dev".into() },
        None,
    )
    .unwrap();

    assert!(report.jobs.iter().all(|j| j.status == BuildStatus::Skipped));
}

#[test]
fn failing_job_skips_dependents() {
    let root = tempfile::tempdir().unwrap();
    std::fs::write(
        root.path().join(".onedev-buildspec.yml"),
        SPEC.replace("echo built", "exit 3"),
    )
    .unwrap();

    let report = run_buildspec(
        root.path(),
        &root.path().join("artifacts"),
        &TriggerEvent::Push { branch: "main".into() },
        None,
    )
    .unwrap();

    assert_eq!(report.status, BuildStatus::Failed);
    let build = &report.jobs[0];
    let test = &report.jobs[1];
    assert_eq!(build.status, BuildStatus::Failed);
    assert_eq!(test.status, BuildStatus::Skipped);
}
