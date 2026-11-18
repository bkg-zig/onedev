//! Job secrets and properties of a project hierarchy.
//!
//! They live in `projects.build_setting` (same names as vanilla's
//! `ProjectBuildSetting`):
//!
//! ```json
//! {
//!   "jobSecrets":    [{ "name": "deploy-key", "value": "…", "authorization": "on branch \"main\"" }],
//!   "jobProperties": [{ "name": "java-version", "value": "17" }]
//! }
//! ```
//!
//! A project sees its own entries and those of all ancestors; on a name
//! clash the entry closest to the project wins. Secrets are only handed to
//! builds their `authorization` (a [job match](super::job_match), default
//! `on branch "**"`) admits, but the values of *all* secrets of the hierarchy
//! are masked in the build log, like vanilla does.

use std::collections::{BTreeMap, BTreeSet};

use onedev_build::utils::match_path;
use onedev_db::DbPool;
use serde_json::Value;
use sqlx::Row;
use tracing::warn;

use super::job_match::{Allowed, JobMatch, MatchContext};

/// Authorization of a secret that does not say otherwise.
pub const DEFAULT_AUTHORIZATION: &str = r#"on branch "**""#;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JobSecret {
    pub name: String,
    pub value: String,
    pub authorization: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JobProperty {
    pub name: String,
    pub value: String,
}

/// The build related part of one project's settings.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BuildSetting {
    pub job_secrets: Vec<JobSecret>,
    pub job_properties: Vec<JobProperty>,
}

impl BuildSetting {
    /// Read `projects.build_setting`. Entries that cannot be understood are
    /// skipped (and logged) instead of failing every build of the project.
    pub fn from_json(value: Option<&Value>) -> Self {
        let mut setting = BuildSetting::default();
        let Some(object) = value.and_then(Value::as_object) else {
            return setting;
        };
        let text = |entry: &Value, key: &str| entry.get(key).and_then(Value::as_str).map(str::to_owned);

        for entry in object
            .get("jobSecrets")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            match (text(entry, "name"), text(entry, "value")) {
                (Some(name), Some(value)) if !name.is_empty() => setting.job_secrets.push(JobSecret {
                    name,
                    value,
                    authorization: text(entry, "authorization")
                        .filter(|a| !a.trim().is_empty())
                        .unwrap_or_else(|| DEFAULT_AUTHORIZATION.to_owned()),
                }),
                _ => warn!("ignoring a malformed entry of build_setting.jobSecrets"),
            }
        }
        for entry in object
            .get("jobProperties")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            match (text(entry, "name"), text(entry, "value")) {
                (Some(name), Some(value)) if !name.is_empty() => {
                    setting.job_properties.push(JobProperty { name, value })
                }
                _ => warn!("ignoring a malformed entry of build_setting.jobProperties"),
            }
        }
        setting
    }
}

/// Settings of the project and all its ancestors, the project itself first.
pub async fn load_hierarchy(pool: &DbPool, project_id: i64) -> Result<Vec<BuildSetting>, sqlx::Error> {
    let rows = sqlx::query(
        r#"
        WITH RECURSIVE chain AS (
            SELECT id, parent_id, build_setting, 1 AS depth FROM projects WHERE id = $1
            UNION ALL
            SELECT p.id, p.parent_id, p.build_setting, c.depth + 1
            FROM projects p JOIN chain c ON p.id = c.parent_id
            WHERE c.depth < 32
        )
        SELECT build_setting FROM chain ORDER BY depth
        "#,
    )
    .bind(project_id)
    .fetch_all(pool)
    .await?;
    rows.iter()
        .map(|row| {
            let value: Option<Value> = row.try_get("build_setting")?;
            Ok(BuildSetting::from_json(value.as_ref()))
        })
        .collect()
}

/// Where a build comes from; decides what `on branch` means for secrets.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BuildOrigin {
    /// Push, tag, schedule, manual run, ...: the commit has to be on a
    /// branch that matches (so commits that are on no branch of the project,
    /// like the head of a fork's pull request, get no secrets).
    Commit { branches: Vec<String> },
    /// Pull request inside the same project: source *and* target branch have
    /// to match, otherwise anyone able to open a pull request could read the
    /// secrets of the target branch.
    PullRequest {
        source_branch: String,
        target_branch: String,
    },
}

/// What secret authorizations are evaluated against.
#[derive(Debug, Clone)]
pub struct AuthorizationContext {
    pub project_path: String,
    pub origin: BuildOrigin,
}

impl MatchContext for AuthorizationContext {
    fn project_path(&self) -> &str {
        &self.project_path
    }

    fn job_name(&self) -> Result<&str, String> {
        Err("the job name is not known when job secrets are authorized".to_owned())
    }

    fn on_branch(&self, pattern: &str) -> Result<bool, String> {
        Ok(match &self.origin {
            BuildOrigin::Commit { branches } => branches.iter().any(|branch| match_path(pattern, branch)),
            BuildOrigin::PullRequest {
                source_branch,
                target_branch,
            } => match_path(pattern, source_branch) && match_path(pattern, target_branch),
        })
    }
}

/// Secrets and properties a build may use.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ResolvedSettings {
    /// Usable secrets (authorized for the build).
    pub secrets: BTreeMap<String, String>,
    /// Names of secrets the build is not authorized for.
    pub denied: BTreeSet<String>,
    /// Values of the denied secrets; masked in logs although unusable.
    pub denied_values: Vec<String>,
    pub properties: BTreeMap<String, String>,
}

/// Whether `authorization` admits the build described by `context`.
/// A malformed expression admits nothing.
pub fn is_authorized(authorization: &str, context: &dyn MatchContext) -> bool {
    match JobMatch::parse(
        authorization,
        Allowed {
            project: true,
            job: false,
        },
    )
    .and_then(|expression| expression.matches(context))
    {
        Ok(admitted) => admitted,
        Err(error) => {
            warn!(%error, authorization, "invalid job secret authorization, denying access");
            false
        }
    }
}

/// Merge the settings of the hierarchy (project first) for one build.
pub fn resolve(hierarchy: &[BuildSetting], context: &dyn MatchContext) -> ResolvedSettings {
    let mut resolved = ResolvedSettings::default();
    let mut seen_secrets = BTreeSet::new();
    let mut seen_properties = BTreeSet::new();
    for setting in hierarchy {
        for secret in &setting.job_secrets {
            if !seen_secrets.insert(secret.name.clone()) {
                continue; // shadowed by a secret closer to the project
            }
            if is_authorized(&secret.authorization, context) {
                resolved.secrets.insert(secret.name.clone(), secret.value.clone());
            } else {
                resolved.denied.insert(secret.name.clone());
                resolved.denied_values.push(secret.value.clone());
            }
        }
        for property in &setting.job_properties {
            if seen_properties.insert(property.name.clone()) {
                resolved
                    .properties
                    .insert(property.name.clone(), property.value.clone());
            }
        }
    }
    resolved
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn ctx(origin: BuildOrigin) -> AuthorizationContext {
        AuthorizationContext {
            project_path: "group/app".to_owned(),
            origin,
        }
    }

    fn on(branches: &[&str]) -> AuthorizationContext {
        ctx(BuildOrigin::Commit {
            branches: branches.iter().map(|b| (*b).to_owned()).collect(),
        })
    }

    fn setting(value: serde_json::Value) -> BuildSetting {
        BuildSetting::from_json(Some(&value))
    }

    #[test]
    fn reads_the_json_leniently() {
        let parsed = setting(json!({
            "jobSecrets": [
                {"name": "a", "value": "1"},
                {"name": "b", "value": "2", "authorization": "on branch \"main\""},
                {"name": "blank-auth", "value": "3", "authorization": "  "},
                {"value": "no-name"},
                {"name": "", "value": "empty-name"},
                "garbage",
            ],
            "jobProperties": [{"name": "p", "value": "v"}, {"name": 3}],
        }));
        assert_eq!(parsed.job_secrets.len(), 3);
        assert_eq!(parsed.job_secrets[0].authorization, DEFAULT_AUTHORIZATION);
        assert_eq!(parsed.job_secrets[1].authorization, r#"on branch "main""#);
        assert_eq!(parsed.job_secrets[2].authorization, DEFAULT_AUTHORIZATION);
        assert_eq!(parsed.job_properties.len(), 1);
        assert_eq!(BuildSetting::from_json(None), BuildSetting::default());
        assert_eq!(BuildSetting::from_json(Some(&json!([1, 2]))), BuildSetting::default());
    }

    #[test]
    fn commit_builds_need_the_commit_on_a_matching_branch() {
        let hierarchy = [setting(json!({"jobSecrets": [
            {"name": "any", "value": "value-any"},
            {"name": "release", "value": "value-release", "authorization": "on branch \"release/**\""},
            {"name": "main", "value": "value-main", "authorization": "on branch \"main\" or on branch \"develop\""},
        ]}))];

        let on_main = resolve(&hierarchy, &on(&["main", "feature/x"]));
        assert_eq!(on_main.secrets.keys().collect::<Vec<_>>(), ["any", "main"]);
        assert_eq!(on_main.denied.iter().collect::<Vec<_>>(), ["release"]);
        assert_eq!(on_main.denied_values, ["value-release"]);

        let on_release = resolve(&hierarchy, &on(&["release/1.0"]));
        assert_eq!(on_release.secrets.keys().collect::<Vec<_>>(), ["any", "release"]);

        // a commit that is on no branch (fork pull request head) gets nothing
        let orphan = resolve(&hierarchy, &on(&[]));
        assert!(orphan.secrets.is_empty());
        assert_eq!(orphan.denied.len(), 3);
        assert_eq!(orphan.denied_values.len(), 3);
    }

    #[test]
    fn pull_requests_need_source_and_target_branch_to_match() {
        let hierarchy = [setting(json!({"jobSecrets": [
            {"name": "main-only", "value": "v1", "authorization": "on branch \"main\""},
            {"name": "anything", "value": "v2"},
        ]}))];
        let pr = |source: &str, target: &str| {
            ctx(BuildOrigin::PullRequest {
                source_branch: source.to_owned(),
                target_branch: target.to_owned(),
            })
        };
        let from_feature = resolve(&hierarchy, &pr("feature", "main"));
        assert_eq!(from_feature.secrets.keys().collect::<Vec<_>>(), ["anything"]);
        let main_to_main = resolve(&hierarchy, &pr("main", "main"));
        assert_eq!(main_to_main.secrets.len(), 2);
    }

    #[test]
    fn the_closest_project_wins_on_name_clashes_and_invalid_expressions_deny() {
        let child = setting(json!({
            "jobSecrets": [
                {"name": "token", "value": "child-token"},
                {"name": "broken", "value": "broken-value", "authorization": "this is not valid"},
            ],
            "jobProperties": [{"name": "jdk", "value": "21"}],
        }));
        let parent = setting(json!({
            "jobSecrets": [
                {"name": "token", "value": "parent-token"},
                {"name": "inherited", "value": "inherited-value"},
            ],
            "jobProperties": [{"name": "jdk", "value": "11"}, {"name": "node", "value": "20"}],
        }));
        let resolved = resolve(&[child, parent], &on(&["main"]));
        assert_eq!(resolved.secrets["token"], "child-token");
        assert_eq!(resolved.secrets["inherited"], "inherited-value");
        assert!(resolved.denied.contains("broken"));
        assert_eq!(resolved.properties["jdk"], "21");
        assert_eq!(resolved.properties["node"], "20");
        // shadowed parent values are not part of the build at all
        assert!(!resolved.denied_values.contains(&"parent-token".to_owned()));
    }

    #[test]
    fn project_criteria_can_restrict_secrets_to_projects() {
        let hierarchy = [setting(json!({"jobSecrets": [
            {"name": "inside", "value": "x1", "authorization": "\"Project\" is \"group/**\" and on branch \"**\""},
            {"name": "outside", "value": "x2", "authorization": "\"Project\" is \"other/**\""},
            {"name": "job-based", "value": "x3", "authorization": "\"Job\" is \"deploy\""},
        ]}))];
        let resolved = resolve(&hierarchy, &on(&["main"]));
        assert_eq!(resolved.secrets.keys().collect::<Vec<_>>(), ["inside"]);
        // job criteria cannot be evaluated while authorizing secrets: denied
        assert!(resolved.denied.contains("job-based"));
    }
}
