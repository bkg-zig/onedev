use onedev_build::handlers::run_buildspec;
use onedev_build::models::{BuildStatus, TriggerEvent};

const SPEC: &str = r#"
version: 59
jobs:
  - name: build
    steps:
      - type: CheckoutStep
        name: checkout
      - type: CommandStep
        name: compile
        interpreter:
          type: PosixInterpreter
          commands: mkdir -p target && printf 'binary' > target/app && echo built
      - type: PublishArtifactStep
        name: publish
        artifacts: target/app
    triggers:
      - type: BranchUpdateTrigger
        branches: main
  - name: test
    jobDependencies:
      - jobName: build
    steps:
      - type: CommandStep
        interpreter:
          type: PosixInterpreter
          commands: test -f target/app && echo tested
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
        &TriggerEvent::Push {
            branch: "main".into(),
        },
        Some(tx),
    )
    .unwrap();

    assert_eq!(report.status, BuildStatus::Success);
    assert_eq!(report.jobs.len(), 2);
    assert!(report.jobs.iter().all(|j| j.status == BuildStatus::Success));
    assert!(
        artifacts.join("build/target/app").exists(),
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
        &TriggerEvent::Push {
            branch: "dev".into(),
        },
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
        &TriggerEvent::Push {
            branch: "main".into(),
        },
        None,
    )
    .unwrap();

    assert_eq!(report.status, BuildStatus::Failed);
    let build = &report.jobs[0];
    let test = &report.jobs[1];
    assert_eq!(build.status, BuildStatus::Failed);
    assert_eq!(test.status, BuildStatus::Skipped);
}

#[test]
fn parses_the_vanilla_repository_buildspec() {
    // The real spec shipped with OneDev itself must parse.
    let manifest = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let vanilla = manifest.join("../../vendor/vanilla-onedev/.onedev-buildspec.yml");
    let spec =
        onedev_build::services::parser::parse_buildspec(&std::fs::read_to_string(vanilla).unwrap())
            .unwrap();
    assert!(!spec.jobs.is_empty());
    assert!(spec.jobs.iter().any(|job| job.name == "Release"));
}
