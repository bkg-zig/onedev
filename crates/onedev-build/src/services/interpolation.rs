//! Variable interpolation - port of `io.onedev.server.util.interpolative`
//! (`Interpolative.g4`, `VariableInterpolator`, `JobVariableInterpolator`).
//!
//! Grammar (identical to vanilla):
//!
//! * `@name@` is a variable (the name is one or more characters other than `@`),
//! * `@@` is a literal `@`,
//! * any other single `@` is a syntax error.
//!
//! Variables resolve to the job variables (`build_number`, `branch`, ...),
//! `param:x`, `property:x`, `secret:x` (and the plural spellings `params:`,
//! `properties:`, `secrets:`), `file:path` (content of a workspace file) and
//! `attribute:name`. An unknown name is an error, never silently kept.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

/// A piece of an interpolated string.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Segment {
    Literal(String),
    Variable(String),
}

/// Why a string could not be interpolated.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum InterpolationError {
    #[error("malformed interpolation: a lone `@` must be written as `@@` (near `{0}`)")]
    Syntax(String),
    #[error("Unrecognized interpolation variable: {0}")]
    UnknownVariable(String),
    #[error("Undefined param: {0}")]
    UndefinedParam(String),
    #[error("Undefined property: {0}")]
    UndefinedProperty(String),
    #[error("Undefined secret: {0}")]
    UndefinedSecret(String),
    /// The secret exists but its `authorization` does not admit this build.
    #[error("No authorized job secret found (project: {project}, job secret: {name})")]
    UnauthorizedSecret { project: String, name: String },
    #[error("cannot read `{path}` for @file:{path}@: {reason}")]
    File { path: String, reason: String },
    #[error("{0}")]
    Other(String),
}

/// Split text into literal and variable segments (`Interpolative.parse`).
pub fn parse(text: &str) -> Result<Vec<Segment>, InterpolationError> {
    let mut segments = Vec::new();
    let mut literal = String::new();
    let mut chars = text.char_indices().peekable();
    while let Some((start, ch)) = chars.next() {
        if ch != '@' {
            literal.push(ch);
            continue;
        }
        match chars.peek().copied() {
            Some((_, '@')) => {
                chars.next();
                literal.push('@');
            }
            Some(_) => {
                // `@` VariableChar+ `@`
                let mut name = String::new();
                let mut closed = false;
                for (_, c) in chars.by_ref() {
                    if c == '@' {
                        closed = true;
                        break;
                    }
                    name.push(c);
                }
                if !closed {
                    let context: String = text[start..].chars().take(24).collect();
                    return Err(InterpolationError::Syntax(context));
                }
                if !literal.is_empty() {
                    segments.push(Segment::Literal(std::mem::take(&mut literal)));
                }
                segments.push(Segment::Variable(name));
            }
            None => {
                return Err(InterpolationError::Syntax("@".to_owned()));
            }
        }
    }
    if !literal.is_empty() {
        segments.push(Segment::Literal(literal));
    }
    Ok(segments)
}

/// Interpolate with a resolver. `Ok(None)` from the resolver contributes
/// nothing (vanilla skips `null` values).
pub fn interpolate_with(
    text: &str,
    resolver: &dyn Fn(&str) -> Result<Option<String>, InterpolationError>,
) -> Result<String, InterpolationError> {
    if !text.contains('@') {
        return Ok(text.to_owned());
    }
    let mut out = String::with_capacity(text.len());
    for segment in parse(text)? {
        match segment {
            Segment::Literal(literal) => out.push_str(&literal),
            Segment::Variable(name) => {
                if let Some(value) = resolver(&name)? {
                    out.push_str(&value);
                }
            }
        }
    }
    Ok(out)
}

/// Names of the fixed job variables (`JobVariable`), lower-cased.
pub const JOB_VARIABLES: &[&str] = &[
    "project_name",
    "project_path",
    "job_name",
    "job_token",
    "ref",
    "branch",
    "tag",
    "commit_hash",
    "build_number",
    "build_version",
    "pull_request_number",
    "issue_number",
    "server",
    "server_host",
    "server_url",
];

/// Everything a job's strings can refer to.
#[derive(Clone, Default)]
pub struct Variables {
    /// `JobVariable` values (lower-case names). Absent entries resolve to "".
    pub job: BTreeMap<String, String>,
    pub params: BTreeMap<String, String>,
    pub properties: BTreeMap<String, String>,
    /// Secret values. They are masked in the build log.
    pub secrets: BTreeMap<String, String>,
    /// Secrets that exist but may not be used by this build; referencing one
    /// fails with [`InterpolationError::UnauthorizedSecret`].
    pub denied_secrets: BTreeSet<String>,
    /// Values masked in the log although they are not usable (the values of
    /// `denied_secrets`: vanilla masks every secret of the project hierarchy).
    pub extra_masked: Vec<String>,
    /// `build_version`, settable at run time by `SetBuildVersionStep`.
    pub build_version: Arc<Mutex<Option<String>>>,
    /// Build attributes (`@attribute:name@`).
    pub attributes: BTreeMap<String, String>,
    /// Workspace for `@file:path@`.
    pub workspace: Option<PathBuf>,
}

/// Secret values must never reach a log by accident: only names are shown.
impl std::fmt::Debug for Variables {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Variables")
            .field("job", &self.job)
            .field("params", &self.params)
            .field("properties", &self.properties)
            .field("secrets", &self.secrets.keys().collect::<Vec<_>>())
            .field("denied_secrets", &self.denied_secrets)
            .field("build_version", &self.build_version())
            .field("attributes", &self.attributes)
            .field("workspace", &self.workspace)
            .finish()
    }
}

impl Variables {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_job(mut self, name: &str, value: impl Into<String>) -> Self {
        self.job.insert(name.to_owned(), value.into());
        self
    }

    pub fn with_param(mut self, name: &str, value: impl Into<String>) -> Self {
        self.params.insert(name.to_owned(), value.into());
        self
    }

    pub fn with_property(mut self, name: &str, value: impl Into<String>) -> Self {
        self.properties.insert(name.to_owned(), value.into());
        self
    }

    pub fn with_secret(mut self, name: &str, value: impl Into<String>) -> Self {
        self.secrets.insert(name.to_owned(), value.into());
        self
    }

    /// Same variables with another workspace (jobs have their own).
    pub fn for_workspace(&self, workspace: &Path) -> Self {
        let mut copy = self.clone();
        copy.workspace = Some(workspace.to_path_buf());
        copy
    }

    /// Same variables for another job of the same build.
    pub fn for_job(&self, job_name: &str, job_token: &str) -> Self {
        let mut copy = self.clone();
        copy.job.insert("job_name".to_owned(), job_name.to_owned());
        copy.job
            .insert("job_token".to_owned(), job_token.to_owned());
        copy
    }

    /// Variables with additional params (used for template parameters).
    pub fn with_params(&self, params: &BTreeMap<String, String>) -> Self {
        let mut copy = self.clone();
        copy.params = params.clone();
        copy
    }

    pub fn build_version(&self) -> Option<String> {
        self.build_version
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone()
    }

    pub fn set_build_version(&self, version: String) {
        *self
            .build_version
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(version);
    }

    fn resolve(&self, name: &str) -> Result<Option<String>, InterpolationError> {
        if name == "build_version" {
            return Ok(Some(
                self.build_version()
                    .or_else(|| self.job.get("build_version").cloned())
                    .unwrap_or_default(),
            ));
        }
        if JOB_VARIABLES.contains(&name) {
            return Ok(Some(self.job.get(name).cloned().unwrap_or_default()));
        }
        let strip = |singular: &str, plural: &str| -> Option<String> {
            name.strip_prefix(singular)
                .or_else(|| name.strip_prefix(plural))
                .map(str::to_owned)
        };
        if let Some(param) = strip("param:", "params:") {
            return self
                .params
                .get(&param)
                .cloned()
                .map(Some)
                .ok_or(InterpolationError::UndefinedParam(param));
        }
        if let Some(property) = strip("property:", "properties:") {
            return self
                .properties
                .get(&property)
                .cloned()
                .map(Some)
                .ok_or(InterpolationError::UndefinedProperty(property));
        }
        if let Some(secret) = strip("secret:", "secrets:") {
            return match self.secrets.get(&secret) {
                Some(value) => Ok(Some(value.clone())),
                None if self.denied_secrets.contains(&secret) => {
                    Err(InterpolationError::UnauthorizedSecret {
                        project: self.job.get("project_path").cloned().unwrap_or_default(),
                        name: secret,
                    })
                }
                None => Err(InterpolationError::UndefinedSecret(secret)),
            };
        }
        if let Some(path) = name.strip_prefix("file:") {
            return self.read_workspace_file(path).map(Some);
        }
        if let Some(attribute) = name.strip_prefix("attribute:") {
            return Ok(self
                .attributes
                .get(attribute)
                .cloned()
                .or(Some(String::new())));
        }
        Err(InterpolationError::UnknownVariable(name.to_owned()))
    }

    fn read_workspace_file(&self, relative: &str) -> Result<String, InterpolationError> {
        let fail = |reason: String| InterpolationError::File {
            path: relative.to_owned(),
            reason,
        };
        let workspace = self
            .workspace
            .as_deref()
            .ok_or_else(|| fail("no workspace is available here".to_owned()))?;
        let path = crate::services::executor::safe_join(workspace, relative)
            .ok_or_else(|| fail("path leaves the workspace".to_owned()))?;
        // A symlink must not be able to read files outside the workspace.
        let real = path.canonicalize().map_err(|e| fail(e.to_string()))?;
        let real_workspace = workspace.canonicalize().map_err(|e| fail(e.to_string()))?;
        if !real.starts_with(&real_workspace) {
            return Err(fail("path leaves the workspace".to_owned()));
        }
        let content = std::fs::read(&real).map_err(|e| fail(e.to_string()))?;
        if content.len() > 1024 * 1024 {
            return Err(fail("file is larger than 1 MiB".to_owned()));
        }
        // vanilla trims the trailing line break of the file content
        let text = String::from_utf8_lossy(&content);
        Ok(text.trim_end_matches(['\n', '\r']).to_owned())
    }

    /// Interpolate `text` (error on unknown variables).
    pub fn interpolate(&self, text: &str) -> Result<String, InterpolationError> {
        interpolate_with(text, &|name| self.resolve(name))
    }

    /// Environment form of the fixed variables: `ONEDEV_BUILD_NUMBER`, ...
    /// (and `ONEDEV_PARAM_<NAME>`). Secrets are deliberately not exported.
    pub fn environment(&self) -> Vec<(String, String)> {
        let mut env: Vec<(String, String)> = Vec::new();
        for name in JOB_VARIABLES {
            if *name == "job_token" {
                continue;
            }
            let value = if *name == "build_version" {
                self.build_version()
                    .or_else(|| self.job.get(*name).cloned())
            } else {
                self.job.get(*name).cloned()
            };
            if let Some(value) = value {
                env.push((env_name(name), value));
            }
        }
        for (name, value) in &self.params {
            env.push((env_name(&format!("param:{name}")), value.clone()));
        }
        env
    }

    /// Secret values to mask in logs. Like vanilla (`Build.getMaskSecrets`)
    /// secrets shorter than the mask itself are left alone: masking `abc`
    /// would garble every log line containing it.
    pub fn secret_values(&self) -> Vec<String> {
        self.secrets
            .values()
            .chain(self.extra_masked.iter())
            .filter(|value| value.len() >= MASK.len())
            .cloned()
            .collect()
    }
}

/// `build_number` -> `ONEDEV_BUILD_NUMBER`, `param:x-y` -> `ONEDEV_PARAM_X_Y`.
pub fn env_name(variable: &str) -> String {
    let mut name = String::from("ONEDEV_");
    name.extend(variable.chars().map(|c| {
        if c.is_ascii_alphanumeric() {
            c.to_ascii_uppercase()
        } else {
            '_'
        }
    }));
    name
}

/// Replaces secret values by `*******` (vanilla masks secrets in build logs).
#[derive(Debug, Clone, Default)]
pub struct SecretMasker {
    /// Longest first so that overlapping secrets are masked completely.
    secrets: Vec<String>,
}

pub const MASK: &str = "*******";

impl SecretMasker {
    pub fn new(mut secrets: Vec<String>) -> Self {
        secrets.retain(|s| !s.is_empty());
        secrets.sort_by_key(|s| std::cmp::Reverse(s.len()));
        secrets.dedup();
        Self { secrets }
    }

    pub fn is_empty(&self) -> bool {
        self.secrets.is_empty()
    }

    pub fn mask(&self, line: &str) -> String {
        let mut masked = line.to_owned();
        for secret in &self.secrets {
            if masked.contains(secret.as_str()) {
                masked = masked.replace(secret.as_str(), MASK);
            }
        }
        masked
    }
}

/// Replace only `@param:name@` references with the given values and keep every
/// other variable (and literal `@@`) exactly as written. Used when a step
/// template is instantiated: its own parameters are bound at expansion time,
/// everything else (`@build_version@`, `@file:..@`, secrets) at run time.
pub fn substitute_params(
    text: &str,
    params: &BTreeMap<String, String>,
) -> Result<String, InterpolationError> {
    if !text.contains('@') {
        return Ok(text.to_owned());
    }
    let mut out = String::with_capacity(text.len());
    for segment in parse(text)? {
        match segment {
            Segment::Literal(literal) => out.push_str(&literal.replace('@', "@@")),
            Segment::Variable(name) => {
                let param = name
                    .strip_prefix("param:")
                    .or_else(|| name.strip_prefix("params:"));
                match param {
                    Some(param) => match params.get(param) {
                        // the value is data: escape `@` so it is not re-interpreted later
                        Some(value) => out.push_str(&value.replace('@', "@@")),
                        None => return Err(InterpolationError::UndefinedParam(param.to_owned())),
                    },
                    None => {
                        out.push('@');
                        out.push_str(&name);
                        out.push('@');
                    }
                }
            }
        }
    }
    Ok(out)
}

/// Collect parameters of a `UseTemplateStep` into combinations: the cartesian
/// product of `matrix` (name -> values) without the combinations matching an
/// entry of `exclude` (all given names equal). Order is deterministic.
pub fn param_combinations(
    matrix: &BTreeMap<String, Vec<String>>,
    exclude: &[BTreeMap<String, String>],
) -> Vec<BTreeMap<String, String>> {
    let mut combinations: Vec<BTreeMap<String, String>> = vec![BTreeMap::new()];
    for (name, values) in matrix {
        if values.is_empty() {
            continue;
        }
        let mut next = Vec::with_capacity(combinations.len() * values.len());
        for combination in &combinations {
            for value in values {
                let mut extended = combination.clone();
                extended.insert(name.clone(), value.clone());
                next.push(extended);
            }
        }
        combinations = next;
    }
    combinations.retain(|combination| {
        !exclude.iter().any(|excluded| {
            !excluded.is_empty()
                && excluded
                    .iter()
                    .all(|(name, value)| combination.get(name) == Some(value))
        })
    });
    combinations
}

/// Merge several maps (later wins) - handy for building variable scopes.
pub fn merged<'a>(
    maps: impl IntoIterator<Item = &'a HashMap<String, String>>,
) -> HashMap<String, String> {
    let mut out = HashMap::new();
    for map in maps {
        out.extend(map.iter().map(|(k, v)| (k.clone(), v.clone())));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vars() -> Variables {
        Variables::new()
            .with_job("build_number", "42")
            .with_job("branch", "main")
            .with_job("project_path", "group/app")
            .with_param("env", "prod")
            .with_property("buildenv", "maven:3-jdk-17")
            .with_secret("token", "s3cr3t-value")
    }

    #[test]
    fn grammar_matches_the_antlr_definition() {
        assert_eq!(
            parse("plain").unwrap(),
            vec![Segment::Literal("plain".into())]
        );
        assert_eq!(
            parse("a@x@b@@c").unwrap(),
            vec![
                Segment::Literal("a".into()),
                Segment::Variable("x".into()),
                Segment::Literal("b@c".into()),
            ]
        );
        assert_eq!(parse("@@").unwrap(), vec![Segment::Literal("@".into())]);
        assert!(
            parse("mail@example.com").is_err(),
            "a lone @ is a syntax error like in vanilla"
        );
        assert!(parse("trailing@").is_err());
        assert_eq!(
            parse("@a b@").unwrap(),
            vec![Segment::Variable("a b".into())],
            "names are any non-@ characters"
        );
        // `@` directly followed by `@` is always a literal, never an empty variable
        assert_eq!(
            parse("@@x@@").unwrap(),
            vec![Segment::Literal("@x@".into())]
        );
    }

    #[test]
    fn resolves_job_variables_params_properties_and_secrets() {
        let v = vars();
        assert_eq!(
            v.interpolate("v@build_number@-@branch@ @project_path@")
                .unwrap(),
            "v42-main group/app"
        );
        assert_eq!(
            v.interpolate("@param:env@ @params:env@").unwrap(),
            "prod prod"
        );
        assert_eq!(
            v.interpolate("@property:buildenv@ @properties:buildenv@")
                .unwrap(),
            "maven:3-jdk-17 maven:3-jdk-17"
        );
        assert_eq!(
            v.interpolate("@secret:token@ @secrets:token@").unwrap(),
            "s3cr3t-value s3cr3t-value"
        );
        // job variables without a value are empty (vanilla: `value != null ? value : ""`)
        assert_eq!(
            v.interpolate("[@tag@][@pull_request_number@]").unwrap(),
            "[][]"
        );
        assert_eq!(v.interpolate("100@@").unwrap(), "100@");
    }

    #[test]
    fn unknown_names_are_errors_not_silently_kept() {
        let v = vars();
        assert_eq!(
            v.interpolate("@nope@").unwrap_err(),
            InterpolationError::UnknownVariable("nope".into())
        );
        assert_eq!(
            v.interpolate("@param:x@").unwrap_err(),
            InterpolationError::UndefinedParam("x".into())
        );
        assert_eq!(
            v.interpolate("@property:x@").unwrap_err(),
            InterpolationError::UndefinedProperty("x".into())
        );
        assert_eq!(
            v.interpolate("@secret:x@").unwrap_err(),
            InterpolationError::UndefinedSecret("x".into())
        );
        assert!(v.interpolate("@file:a.txt@").is_err(), "no workspace");
        // error messages never contain secret values
        assert!(
            !v.interpolate("@secret:other@")
                .unwrap_err()
                .to_string()
                .contains("s3cr3t")
        );
    }

    #[test]
    fn build_version_reflects_later_updates() {
        let v = vars();
        assert_eq!(v.interpolate("@build_version@").unwrap(), "");
        v.set_build_version("1.2.3".into());
        assert_eq!(v.interpolate("@build_version@").unwrap(), "1.2.3");
        let clone = v.for_job("other", "tok");
        assert_eq!(
            clone
                .interpolate("@build_version@ @job_name@ @job_token@")
                .unwrap(),
            "1.2.3 other tok"
        );
        clone.set_build_version("2.0.0".into());
        assert_eq!(
            v.interpolate("@build_version@").unwrap(),
            "2.0.0",
            "the version cell is shared between jobs"
        );
    }

    #[test]
    fn file_variables_read_workspace_files_safely() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("version.txt"), "7.1\n").unwrap();
        std::fs::create_dir(dir.path().join("sub")).unwrap();
        std::fs::write(dir.path().join("sub/x"), "nested").unwrap();
        let outside = tempfile::tempdir().unwrap();
        std::fs::write(outside.path().join("secret"), "top secret").unwrap();
        std::os::unix::fs::symlink(outside.path().join("secret"), dir.path().join("link")).unwrap();

        let v = vars().for_workspace(dir.path());
        assert_eq!(v.interpolate("v@file:version.txt@").unwrap(), "v7.1");
        assert_eq!(v.interpolate("@file:sub/x@").unwrap(), "nested");
        assert!(v.interpolate("@file:../escape@").is_err());
        assert!(v.interpolate("@file:/etc/passwd@").is_err());
        assert!(
            v.interpolate("@file:link@").is_err(),
            "symlinks must not leak outside files"
        );
        assert!(v.interpolate("@file:missing@").is_err());
    }

    #[test]
    fn environment_exports_job_variables_but_never_secrets() {
        let env: HashMap<String, String> = vars().environment().into_iter().collect();
        assert_eq!(
            env.get("ONEDEV_BUILD_NUMBER").map(String::as_str),
            Some("42")
        );
        assert_eq!(
            env.get("ONEDEV_PARAM_ENV").map(String::as_str),
            Some("prod")
        );
        assert!(!env.values().any(|v| v.contains("s3cr3t")));
        assert!(!env.contains_key("ONEDEV_JOB_TOKEN"));
        assert_eq!(env_name("param:my-var"), "ONEDEV_PARAM_MY_VAR");
    }

    #[test]
    fn masking_hides_secrets_longest_first() {
        let masker = SecretMasker::new(vec!["abc".into(), "abcdef".into(), "".into()]);
        assert_eq!(masker.mask("x abcdef y abc"), format!("x {MASK} y {MASK}"));
        assert_eq!(masker.mask("nothing here"), "nothing here");
        assert!(SecretMasker::new(vec![]).is_empty());
        assert_eq!(vars().secret_values(), vec!["s3cr3t-value".to_owned()]);
        // secrets shorter than the mask are not masked (vanilla: `removeIf(length < MASK.length())`)
        let short = vars()
            .with_secret("pin", "123456")
            .with_secret("exact", "1234567");
        assert_eq!(
            short.secret_values().len(),
            2,
            "{:?}",
            short.secret_values()
        );
        assert!(!short.secret_values().contains(&"123456".to_owned()));
    }

    #[test]
    fn unauthorized_secrets_fail_loudly_but_are_still_masked() {
        let mut variables = vars();
        variables.denied_secrets.insert("deploy-key".to_owned());
        variables.extra_masked.push("denied-secret-value".to_owned());

        assert_eq!(
            variables.interpolate("@secret:deploy-key@").unwrap_err(),
            InterpolationError::UnauthorizedSecret {
                project: "group/app".to_owned(),
                name: "deploy-key".to_owned(),
            }
        );
        assert_eq!(
            variables.interpolate("@secret:deploy-key@").unwrap_err().to_string(),
            "No authorized job secret found (project: group/app, job secret: deploy-key)"
        );
        // unknown names stay a different error
        assert_eq!(
            variables.interpolate("@secret:nope@").unwrap_err(),
            InterpolationError::UndefinedSecret("nope".to_owned())
        );
        // the denied value must never show up in a log
        let masker = SecretMasker::new(variables.secret_values());
        assert_eq!(masker.mask("leak: denied-secret-value"), format!("leak: {MASK}"));
    }

    #[test]
    fn debug_output_never_contains_secret_values() {
        let mut variables = vars();
        variables.extra_masked.push("denied-secret-value".to_owned());
        let shown = format!("{variables:?}");
        assert!(shown.contains("token"), "names are shown: {shown}");
        assert!(!shown.contains("s3cr3t-value"), "{shown}");
        assert!(!shown.contains("denied-secret-value"), "{shown}");
    }

    #[test]
    fn template_parameters_are_bound_early_everything_else_stays_for_run_time() {
        let params: BTreeMap<String, String> = [("jdk".to_owned(), "17".to_owned())].into();
        assert_eq!(
            substitute_params("java @param:jdk@ v@build_version@ @secret:x@ a@@b", &params)
                .unwrap(),
            "java 17 v@build_version@ @secret:x@ a@@b"
        );
        // the substituted value is data: a literal @ in it stays a literal @ at run time
        let tricky: BTreeMap<String, String> = [("v".to_owned(), "x@y".to_owned())].into();
        let bound = substitute_params("@param:v@", &tricky).unwrap();
        assert_eq!(Variables::new().interpolate(&bound).unwrap(), "x@y");
        assert_eq!(
            substitute_params("@param:missing@", &params).unwrap_err(),
            InterpolationError::UndefinedParam("missing".into())
        );
        assert_eq!(
            substitute_params("no variables", &params).unwrap(),
            "no variables"
        );
    }

    #[test]
    fn parameter_combinations_support_matrix_and_exclusions() {
        let matrix: BTreeMap<String, Vec<String>> = [
            ("jdk".to_owned(), vec!["11".to_owned(), "17".to_owned()]),
            ("os".to_owned(), vec!["linux".to_owned(), "mac".to_owned()]),
        ]
        .into();
        let all = param_combinations(&matrix, &[]);
        assert_eq!(all.len(), 4);
        let excluded: Vec<BTreeMap<String, String>> = vec![
            [
                ("jdk".to_owned(), "11".to_owned()),
                ("os".to_owned(), "mac".to_owned()),
            ]
            .into(),
        ];
        let some = param_combinations(&matrix, &excluded);
        assert_eq!(some.len(), 3);
        assert!(!some.iter().any(|c| c["jdk"] == "11" && c["os"] == "mac"));
        // a partial exclusion drops every matching combination
        let partial: Vec<BTreeMap<String, String>> =
            vec![[("os".to_owned(), "linux".to_owned())].into()];
        assert_eq!(param_combinations(&matrix, &partial).len(), 2);
        // no matrix: exactly one empty combination (the template runs once)
        assert_eq!(
            param_combinations(&BTreeMap::new(), &[]),
            vec![BTreeMap::new()]
        );
    }
}
