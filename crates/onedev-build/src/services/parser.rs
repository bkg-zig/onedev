//! Build-spec parsing and validation (YAML → `BuildSpec` AST).

use std::collections::{HashMap, HashSet};
use std::path::Path;

use crate::errors::{BuildError, BuildResult};
use crate::models::{BuildSpec, Trigger, TriggerEvent};

/// Parse and validate a build spec from YAML text.
pub fn parse_buildspec(yaml: &str) -> BuildResult<BuildSpec> {
    let spec: BuildSpec = serde_yaml::from_str(yaml)?;
    validate(&spec)?;
    Ok(spec)
}

/// Load, parse and validate `.onedev-buildspec.yml` (or any given path).
pub fn parse_buildspec_file(path: &Path) -> BuildResult<BuildSpec> {
    if !path.is_file() {
        return Err(BuildError::NotFound(path.display().to_string()));
    }
    parse_buildspec(&std::fs::read_to_string(path)?)
}

/// Validate a parsed spec: names, steps, dependencies, cycles, executors.
pub fn validate(spec: &BuildSpec) -> BuildResult<()> {
    if spec.jobs.is_empty() {
        return Err(BuildError::Spec("spec has no jobs".to_owned()));
    }

    let mut names = HashSet::new();
    for job in &spec.jobs {
        if job.name.trim().is_empty() {
            return Err(BuildError::Spec("job with empty name".to_owned()));
        }
        if !names.insert(job.name.as_str()) {
            return Err(BuildError::Spec(format!(
                "duplicate job name: {}",
                job.name
            )));
        }
        if job.steps.is_empty() {
            return Err(BuildError::Spec(format!("job {} has no steps", job.name)));
        }
        for step in &job.steps {
            if step.step_type == "CommandStep"
                && step
                    .commands()
                    .is_none_or(|commands| commands.trim().is_empty())
            {
                return Err(BuildError::Spec(format!(
                    "job {} has a CommandStep without commands",
                    job.name
                )));
            }
        }
    }

    for job in &spec.jobs {
        for need in &job.needs() {
            if need == &job.name {
                return Err(BuildError::Spec(format!(
                    "job {} depends on itself",
                    job.name
                )));
            }
            if !names.contains(need.as_str()) {
                return Err(BuildError::Spec(format!(
                    "job {} needs unknown job {}",
                    job.name, need
                )));
            }
        }
    }

    // Cycle check happens implicitly in the topological sort.
    execution_order(spec).map(|_| ())
}

/// Job names in dependency order (dependencies first).
pub fn execution_order(spec: &BuildSpec) -> BuildResult<Vec<String>> {
    let needs_owned: HashMap<String, Vec<String>> = spec
        .jobs
        .iter()
        .map(|job| (job.name.clone(), job.needs()))
        .collect();
    let needs: HashMap<&str, &[String]> = needs_owned
        .iter()
        .map(|(name, deps)| (name.as_str(), deps.as_slice()))
        .collect();

    let mut ordered = Vec::with_capacity(spec.jobs.len());
    let mut done: HashSet<&str> = HashSet::new();
    let mut visiting: Vec<&str> = Vec::new();

    fn visit<'a>(
        name: &'a str,
        needs: &HashMap<&'a str, &'a [String]>,
        done: &mut HashSet<&'a str>,
        visiting: &mut Vec<&'a str>,
        ordered: &mut Vec<String>,
    ) -> BuildResult<()> {
        if done.contains(name) {
            return Ok(());
        }
        if visiting.contains(&name) {
            return Err(BuildError::Spec(format!(
                "dependency cycle involving job {name}"
            )));
        }
        visiting.push(name);
        for need in needs.get(name).copied().unwrap_or(&[]) {
            visit(need, needs, done, visiting, ordered)?;
        }
        visiting.pop();
        done.insert(name);
        ordered.push(name.to_owned());
        Ok(())
    }

    for job in &spec.jobs {
        visit(&job.name, &needs, &mut done, &mut visiting, &mut ordered)?;
    }
    Ok(ordered)
}

/// Names of the jobs `event` starts, in dependency order: exactly the jobs
/// with a trigger that fires (a manual run starts every job). Like in vanilla
/// OneDev a job without a trigger of its own never runs automatically, not
/// even when a job it depends on does (think of a `deploy` job that needs
/// `build`); the *dependencies* of started jobs are run by the runner.
pub fn jobs_for_event(spec: &BuildSpec, event: &TriggerEvent) -> BuildResult<Vec<String>> {
    let order = execution_order(spec)?;
    let selected: HashSet<&str> = if matches!(event, TriggerEvent::Manual) {
        spec.jobs.iter().map(|job| job.name.as_str()).collect()
    } else {
        spec.jobs
            .iter()
            .filter(|job| job.triggers.iter().any(|t: &Trigger| t.matches(event)))
            .map(|job| job.name.as_str())
            .collect()
    };
    Ok(order
        .into_iter()
        .filter(|name| selected.contains(name.as_str()))
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    const SPEC: &str = r#"
version: 59
jobs:
  - name: build
    steps:
      - type: CommandStep
        name: compile
        interpreter:
          type: PosixInterpreter
          commands: cargo build
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
          commands: cargo test
"#;

    #[test]
    fn parses_and_orders_jobs() {
        let spec = parse_buildspec(SPEC).unwrap();
        assert_eq!(execution_order(&spec).unwrap(), vec!["build", "test"]);
    }

    #[test]
    fn rejects_cycles_and_unknown_needs() {
        let cyclic = SPEC.replace("jobName: build", "jobName: test");
        assert!(parse_buildspec(&cyclic).is_err());
        let unknown = SPEC.replace("jobName: build", "jobName: nope");
        assert!(parse_buildspec(&unknown).is_err());
    }

    #[test]
    fn trigger_matching_respects_branches() {
        let spec = parse_buildspec(SPEC).unwrap();
        let on_main = jobs_for_event(
            &spec,
            &TriggerEvent::push("main"),
        )
        .unwrap();
        // `test` depends on `build` but has no trigger: it does not run on its own
        assert_eq!(on_main, vec!["build"]);
        let on_dev = jobs_for_event(
            &spec,
            &TriggerEvent::push("dev"),
        )
        .unwrap();
        assert!(on_dev.is_empty());
        let manual = jobs_for_event(&spec, &TriggerEvent::Manual).unwrap();
        assert_eq!(manual, vec!["build", "test"]);
    }

    #[test]
    fn dependents_only_run_with_a_trigger_of_their_own() {
        let step = "steps:\n      - type: CommandStep\n        interpreter:\n          type: PosixInterpreter\n          commands: \"true\"";
        let yaml = format!(
            r#"
version: 59
jobs:
  - name: build
    {step}
    triggers:
      - type: BranchUpdateTrigger
  - name: test
    jobDependencies: [ {{ jobName: build }} ]
    {step}
    triggers:
      - type: BranchUpdateTrigger
        branches: main
  - name: deploy
    jobDependencies: [ {{ jobName: test }} ]
    {step}
  - name: nightly
    {step}
    triggers:
      - type: ScheduleTrigger
        cronExpression: "0 0 * * * ?"
"#
        );
        let spec = parse_buildspec(&yaml).unwrap();
        // `deploy` has no trigger: it never starts automatically
        assert_eq!(
            jobs_for_event(&spec, &TriggerEvent::push("main")).unwrap(),
            vec!["build", "test"]
        );
        assert_eq!(
            jobs_for_event(&spec, &TriggerEvent::push("dev")).unwrap(),
            vec!["build"]
        );
        assert_eq!(
            jobs_for_event(&spec, &TriggerEvent::Schedule).unwrap(),
            vec!["nightly"]
        );
        assert!(jobs_for_event(&spec, &TriggerEvent::tag("v1")).unwrap().is_empty());
    }
}
