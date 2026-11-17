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
            return Err(BuildError::Spec(format!("duplicate job name: {}", job.name)));
        }
        if job.executor != "host" {
            return Err(BuildError::Spec(format!(
                "job {}: unsupported executor '{}' (only 'host' is implemented)",
                job.name, job.executor
            )));
        }
        if job.steps.is_empty() {
            return Err(BuildError::Spec(format!("job {} has no steps", job.name)));
        }
        for step in &job.steps {
            if step.run.trim().is_empty() {
                return Err(BuildError::Spec(format!(
                    "job {} has a step with an empty run command",
                    job.name
                )));
            }
        }
    }

    for job in &spec.jobs {
        for need in &job.needs {
            if need == &job.name {
                return Err(BuildError::Spec(format!("job {} depends on itself", job.name)));
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
    let needs: HashMap<&str, &[String]> = spec
        .jobs
        .iter()
        .map(|job| (job.name.as_str(), job.needs.as_slice()))
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

/// Names of jobs started by `event`, in dependency order: jobs whose
/// triggers match, plus jobs that (transitively) depend on them.
/// Jobs without any trigger only run manually or as a dependency.
pub fn jobs_for_event(spec: &BuildSpec, event: &TriggerEvent) -> BuildResult<Vec<String>> {
    let order = execution_order(spec)?;
    let by_name: HashMap<&str, &crate::models::JobSpec> =
        spec.jobs.iter().map(|job| (job.name.as_str(), job)).collect();

    let mut selected: HashSet<&str> = HashSet::new();
    if matches!(event, TriggerEvent::Manual) {
        selected = by_name.keys().copied().collect();
    } else {
        for job in &spec.jobs {
            if job.triggers.iter().any(|t: &Trigger| t.matches(event)) {
                selected.insert(job.name.as_str());
            }
        }
        // Pull in dependents whose dependencies are all selected.
        loop {
            let mut added = false;
            for job in &spec.jobs {
                if !selected.contains(job.name.as_str())
                    && !job.needs.is_empty()
                    && job.needs.iter().all(|need| selected.contains(need.as_str()))
                {
                    selected.insert(job.name.as_str());
                    added = true;
                }
            }
            if !added {
                break;
            }
        }
    }

    Ok(order
        .into_iter()
        .filter(|name| selected.contains(name.as_str()))
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    const SPEC: &str = r#"
version: 1
jobs:
  - name: build
    steps:
      - run: cargo build
    triggers:
      - type: push
        branches: [main]
  - name: test
    needs: [build]
    steps:
      - run: cargo test
"#;

    #[test]
    fn parses_and_orders_jobs() {
        let spec = parse_buildspec(SPEC).unwrap();
        assert_eq!(execution_order(&spec).unwrap(), vec!["build", "test"]);
    }

    #[test]
    fn rejects_cycles_and_unknown_needs() {
        let cyclic = SPEC.replace("needs: [build]", "needs: [test]");
        assert!(parse_buildspec(&cyclic).is_err());
        let unknown = SPEC.replace("needs: [build]", "needs: [nope]");
        assert!(parse_buildspec(&unknown).is_err());
    }

    #[test]
    fn trigger_matching_respects_branches() {
        let spec = parse_buildspec(SPEC).unwrap();
        let on_main = jobs_for_event(&spec, &TriggerEvent::Push { branch: "main".into() }).unwrap();
        assert_eq!(on_main, vec!["build", "test"]); // dependent job is pulled in
        let on_dev = jobs_for_event(&spec, &TriggerEvent::Push { branch: "dev".into() }).unwrap();
        assert!(on_dev.is_empty());
        let manual = jobs_for_event(&spec, &TriggerEvent::Manual).unwrap();
        assert_eq!(manual, vec!["build", "test"]);
    }
}
