//! Step templates - port of `UseTemplateStep` / `CompositeStep`.
//!
//! `UseTemplateStep { templateName, paramMatrix, excludeParamMaps }` runs the
//! steps of a named template once per parameter combination: the cartesian
//! product of the matrix minus the excluded combinations. Template steps see
//! only the template's own parameters (`@param:x@`); the second and later
//! repetitions get a ` (n)` suffix in their step names (as in vanilla).

use std::collections::BTreeMap;

use serde_json::Value;

use crate::models::{BuildSpec, ExecuteCondition, Step, StepTemplate};
use crate::services::interpolation::{Variables, param_combinations, substitute_params};

pub const USE_TEMPLATE_STEP: &str = "UseTemplateStep";
const MAX_DEPTH: usize = 8;

/// Inline every `UseTemplateStep` (recursively). Other steps are copied.
pub fn expand_steps(
    spec: &BuildSpec,
    steps: &[Step],
    vars: &Variables,
) -> Result<Vec<Step>, String> {
    let mut expanded = Vec::new();
    expand_into(spec, steps, vars, 0, &mut expanded)?;
    Ok(expanded)
}

fn expand_into(
    spec: &BuildSpec,
    steps: &[Step],
    vars: &Variables,
    depth: usize,
    out: &mut Vec<Step>,
) -> Result<(), String> {
    for step in steps {
        if step.step_type != USE_TEMPLATE_STEP {
            out.push(step.clone());
            continue;
        }
        let template_name = step
            .template_name
            .as_deref()
            .filter(|name| !name.is_empty())
            .ok_or("UseTemplateStep without templateName")?;
        if depth >= MAX_DEPTH {
            return Err(format!(
                "step templates are nested more than {MAX_DEPTH} levels deep (recursive use of `{template_name}`?)"
            ));
        }
        let template = spec
            .step_templates
            .iter()
            .find(|template| template.name == template_name)
            .ok_or_else(|| format!("Step template not found: {template_name}"))?;

        let (matrix, excluded) = matrix_of(step, vars)?;
        for (index, combination) in param_combinations(&matrix, &excluded).iter().enumerate() {
            let params = bind_params(template, combination)?;
            let scope = vars.with_params(&params);
            let mut instantiated = Vec::with_capacity(template.steps.len());
            for child in &template.steps {
                let mut child = bind_step(child, &params)?;
                if index > 0 {
                    child.name = Some(format!("{} ({})", child.display_name(), index + 1));
                }
                // The use-step's own condition/optional flag govern its children too.
                if child.condition == ExecuteCondition::Successful {
                    child.condition = step.condition;
                }
                child.optional |= step.optional;
                instantiated.push(child);
            }
            expand_into(spec, &instantiated, &scope, depth + 1, out)?;
        }
    }
    Ok(())
}

/// Parameter values for one instantiation: template defaults overlaid with the
/// combination. Parameters the template does not declare are rejected.
fn bind_params(
    template: &StepTemplate,
    combination: &BTreeMap<String, String>,
) -> Result<BTreeMap<String, String>, String> {
    let mut params = BTreeMap::new();
    for spec in &template.param_specs {
        if let Some(default) = default_value(&spec.extra) {
            params.insert(spec.name.clone(), default);
        }
    }
    for (name, value) in combination {
        if !template.param_specs.iter().any(|spec| &spec.name == name) {
            return Err(format!(
                "step template `{}` has no parameter `{name}`",
                template.name
            ));
        }
        params.insert(name.clone(), value.clone());
    }
    Ok(params)
}

fn default_value(extra: &serde_json::Map<String, Value>) -> Option<String> {
    let provider = extra.get("defaultValueProvider")?;
    match provider.get("value")? {
        Value::String(value) => Some(value.clone()),
        Value::Array(values) => Some(
            values
                .iter()
                .filter_map(Value::as_str)
                .collect::<Vec<_>>()
                .join(","),
        ),
        _ => None,
    }
}

/// Copy of `step` with `@param:x@` bound in every string field.
fn bind_step(step: &Step, params: &BTreeMap<String, String>) -> Result<Step, String> {
    fn walk(value: &mut Value, params: &BTreeMap<String, String>) -> Result<(), String> {
        match value {
            Value::String(text) => {
                *text = substitute_params(text, params).map_err(|e| e.to_string())?;
            }
            Value::Array(items) => {
                for item in items {
                    walk(item, params)?;
                }
            }
            Value::Object(map) => {
                for item in map.values_mut() {
                    walk(item, params)?;
                }
            }
            _ => {}
        }
        Ok(())
    }
    let mut value = serde_json::to_value(step).map_err(|e| e.to_string())?;
    walk(&mut value, params)?;
    serde_json::from_value(value).map_err(|e| e.to_string())
}

type Matrix = BTreeMap<String, Vec<String>>;

/// `paramMatrix` and `excludeParamMaps` of a use-step.
fn matrix_of(
    step: &Step,
    vars: &Variables,
) -> Result<(Matrix, Vec<BTreeMap<String, String>>), String> {
    let mut matrix = Matrix::new();
    if let Some(Value::Array(entries)) = step.extra.get("paramMatrix") {
        for entry in entries {
            let name = entry
                .get("name")
                .and_then(Value::as_str)
                .ok_or("paramMatrix entry without name")?;
            let secret = entry
                .get("secret")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            let provider = entry.get("valuesProvider").unwrap_or(&Value::Null);
            if let Some(alternatives) = values_of(provider, secret, name, vars)? {
                matrix.insert(name.to_owned(), alternatives);
            }
        }
    }

    let mut excluded = Vec::new();
    if let Some(Value::Array(maps)) = step.extra.get("excludeParamMaps") {
        for map in maps {
            let mut one = BTreeMap::new();
            if let Some(Value::Array(params)) = map.get("params") {
                for param in params {
                    let name = param
                        .get("name")
                        .and_then(Value::as_str)
                        .ok_or("excludeParamMaps entry without name")?;
                    let secret = param
                        .get("secret")
                        .and_then(Value::as_bool)
                        .unwrap_or(false);
                    let provider = param.get("valueProvider").unwrap_or(&Value::Null);
                    if let Some(value) = value_of(provider, secret, name, vars)? {
                        one.insert(name.to_owned(), value);
                    }
                }
            }
            excluded.push(one);
        }
    }
    Ok((matrix, excluded))
}

fn resolve(raw: &str, secret: bool, vars: &Variables) -> Result<String, String> {
    if secret {
        vars.secrets
            .get(raw)
            .cloned()
            .ok_or_else(|| format!("Undefined secret: {raw}"))
    } else {
        vars.interpolate(raw).map_err(|e| e.to_string())
    }
}

fn strings_of(value: &Value) -> Vec<String> {
    match value {
        Value::Array(items) => items
            .iter()
            .filter_map(Value::as_str)
            .map(str::to_owned)
            .collect(),
        Value::String(text) => vec![text.clone()],
        _ => Vec::new(),
    }
}

/// All values a `ValuesProvider` yields (one entry per alternative);
/// `None` when the parameter is ignored.
fn values_of(
    provider: &Value,
    secret: bool,
    name: &str,
    vars: &Variables,
) -> Result<Option<Vec<String>>, String> {
    let kind = provider
        .get("type")
        .and_then(Value::as_str)
        .unwrap_or("SpecifiedValues");
    match kind {
        "SpecifiedValues" => {
            let mut alternatives = Vec::new();
            if let Some(Value::Array(values)) = provider.get("values") {
                for entry in values {
                    let resolved = strings_of(entry)
                        .iter()
                        .map(|raw| resolve(raw, secret, vars))
                        .collect::<Result<Vec<_>, _>>()?;
                    alternatives.push(resolved.join(","));
                }
            }
            Ok(Some(alternatives))
        }
        "IgnoreValues" => Ok(None),
        "PassthroughValues" => passthrough(provider, vars).map(|value| Some(vec![value])),
        "ScriptingValues" => Err(format!(
            "parameter `{name}` uses a Groovy script, which this server does not support"
        )),
        other => Err(format!(
            "parameter `{name}` uses the unsupported values provider {other}"
        )),
    }
}

/// The value a `ValueProvider` yields; `None` when the parameter is ignored.
fn value_of(
    provider: &Value,
    secret: bool,
    name: &str,
    vars: &Variables,
) -> Result<Option<String>, String> {
    let kind = provider
        .get("type")
        .and_then(Value::as_str)
        .unwrap_or("SpecifiedValue");
    match kind {
        "SpecifiedValue" => {
            let raw = provider.get("value").map(strings_of).unwrap_or_default();
            let resolved = raw
                .iter()
                .map(|raw| resolve(raw, secret, vars))
                .collect::<Result<Vec<_>, _>>()?;
            Ok(Some(resolved.join(",")))
        }
        "IgnoreValue" => Ok(None),
        "PassthroughValue" => passthrough(provider, vars).map(Some),
        "ScriptingValue" => Err(format!(
            "parameter `{name}` uses a Groovy script, which this server does not support"
        )),
        other => Err(format!(
            "parameter `{name}` uses the unsupported value provider {other}"
        )),
    }
}

fn passthrough(provider: &Value, vars: &Variables) -> Result<String, String> {
    let param = provider
        .get("paramName")
        .and_then(Value::as_str)
        .ok_or("passthrough provider without paramName")?;
    vars.params
        .get(param)
        .cloned()
        .ok_or_else(|| format!("Undefined param: {param}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn step(value: Value) -> Step {
        serde_json::from_value(value).expect("valid step")
    }

    fn command(name: &str, commands: &str) -> Value {
        json!({"type": "CommandStep", "name": name, "interpreter": {"type": "PosixInterpreter", "commands": commands}})
    }

    fn spec_with(template: Value) -> BuildSpec {
        BuildSpec {
            step_templates: vec![serde_json::from_value(template).expect("valid template")],
            ..Default::default()
        }
    }

    fn commands_of(steps: &[Step]) -> Vec<String> {
        steps
            .iter()
            .map(|s| s.commands().unwrap_or("").to_owned())
            .collect()
    }

    #[test]
    fn plain_steps_pass_through_and_templates_are_inlined() {
        let spec = spec_with(json!({
            "name": "set up cache",
            "steps": [command("restore", "echo restore"), command("warm", "echo warm")]
        }));
        let steps = vec![
            step(command("first", "echo first")),
            step(json!({"type": "UseTemplateStep", "name": "use", "templateName": "set up cache"})),
            step(command("last", "echo last")),
        ];
        let expanded = expand_steps(&spec, &steps, &Variables::new()).unwrap();
        let names: Vec<_> = expanded
            .iter()
            .map(|s| s.display_name().to_owned())
            .collect();
        assert_eq!(names, ["first", "restore", "warm", "last"]);
    }

    #[test]
    fn matrix_runs_the_template_per_combination_with_numbered_names() {
        let spec = spec_with(json!({
            "name": "build",
            "paramSpecs": [
                {"type": "TextParam", "name": "jdk"},
                {"type": "TextParam", "name": "flavor", "defaultValueProvider": {"type": "SpecifiedDefaultValue", "value": "plain"}}
            ],
            "steps": [command("compile", "echo jdk=@param:jdk@ flavor=@param:flavor@ v=@build_version@")]
        }));
        let use_step = step(json!({
            "type": "UseTemplateStep", "name": "matrix", "templateName": "build",
            "paramMatrix": [
                {"name": "jdk", "secret": false, "valuesProvider": {"type": "SpecifiedValues", "values": [["11"], ["17"]]}}
            ]
        }));
        let expanded = expand_steps(&spec, &[use_step], &Variables::new()).unwrap();
        assert_eq!(
            commands_of(&expanded),
            [
                "echo jdk=11 flavor=plain v=@build_version@",
                "echo jdk=17 flavor=plain v=@build_version@"
            ]
        );
        assert_eq!(expanded[0].display_name(), "compile");
        assert_eq!(expanded[1].display_name(), "compile (2)");
    }

    #[test]
    fn excluded_combinations_are_skipped_and_multi_values_join_with_commas() {
        let spec = spec_with(json!({
            "name": "t",
            "paramSpecs": [{"type": "TextParam", "name": "os"}, {"type": "TextParam", "name": "arch"}],
            "steps": [command("run", "echo @param:os@/@param:arch@")]
        }));
        let use_step = step(json!({
            "type": "UseTemplateStep", "templateName": "t",
            "paramMatrix": [
                {"name": "os", "valuesProvider": {"type": "SpecifiedValues", "values": [["linux"], ["mac"]]}},
                {"name": "arch", "valuesProvider": {"type": "SpecifiedValues", "values": [["x64", "arm"]]}}
            ],
            "excludeParamMaps": [
                {"params": [{"name": "os", "valueProvider": {"type": "SpecifiedValue", "value": ["mac"]}}]}
            ]
        }));
        let expanded = expand_steps(&spec, &[use_step], &Variables::new()).unwrap();
        assert_eq!(commands_of(&expanded), ["echo linux/x64,arm"]);
    }

    #[test]
    fn passthrough_secret_and_ignore_providers() {
        let spec = spec_with(json!({
            "name": "t",
            "paramSpecs": [{"type": "TextParam", "name": "a"}, {"type": "SecretParam", "name": "b"}, {"type": "TextParam", "name": "c", "defaultValueProvider": {"value": "dflt"}}],
            "steps": [command("run", "echo @param:a@ @param:b@ @param:c@")]
        }));
        let use_step = step(json!({
            "type": "UseTemplateStep", "templateName": "t",
            "paramMatrix": [
                {"name": "a", "valuesProvider": {"type": "PassthroughValues", "paramName": "outer"}},
                {"name": "b", "secret": true, "valuesProvider": {"type": "SpecifiedValues", "values": [["my-secret"]]}},
                {"name": "c", "valuesProvider": {"type": "IgnoreValues"}}
            ]
        }));
        let vars = Variables::new()
            .with_param("outer", "from-job")
            .with_secret("my-secret", "s3cr3t");
        let expanded = expand_steps(&spec, &[use_step], &vars).unwrap();
        assert_eq!(commands_of(&expanded), ["echo from-job s3cr3t dflt"]);
    }

    #[test]
    fn errors_are_explicit() {
        let spec =
            spec_with(json!({"name": "t", "paramSpecs": [], "steps": [command("run", "echo hi")]}));
        let missing = step(json!({"type": "UseTemplateStep", "templateName": "nope"}));
        assert_eq!(
            expand_steps(&spec, &[missing], &Variables::new()).unwrap_err(),
            "Step template not found: nope"
        );
        let unknown_param = step(json!({
            "type": "UseTemplateStep", "templateName": "t",
            "paramMatrix": [{"name": "x", "valuesProvider": {"type": "SpecifiedValues", "values": [["1"]]}}]
        }));
        assert!(
            expand_steps(&spec, &[unknown_param], &Variables::new())
                .unwrap_err()
                .contains("no parameter `x`")
        );
        let scripted = step(json!({
            "type": "UseTemplateStep", "templateName": "t",
            "paramMatrix": [{"name": "x", "valuesProvider": {"type": "ScriptingValues", "scriptName": "s"}}]
        }));
        assert!(
            expand_steps(&spec, &[scripted], &Variables::new())
                .unwrap_err()
                .contains("Groovy")
        );
        let nameless = step(json!({"type": "UseTemplateStep"}));
        assert!(expand_steps(&spec, &[nameless], &Variables::new()).is_err());
    }

    #[test]
    fn templates_can_use_templates_but_not_themselves() {
        let spec = BuildSpec {
            step_templates: vec![
                serde_json::from_value(json!({
                    "name": "outer", "steps": [
                        command("a", "echo a"),
                        {"type": "UseTemplateStep", "name": "inner-use", "templateName": "inner"}
                    ]
                }))
                .unwrap(),
                serde_json::from_value(json!({"name": "inner", "steps": [command("b", "echo b")]}))
                    .unwrap(),
                serde_json::from_value(json!({
                    "name": "loop", "steps": [{"type": "UseTemplateStep", "templateName": "loop"}]
                }))
                .unwrap(),
            ],
            ..Default::default()
        };
        let outer = step(json!({"type": "UseTemplateStep", "templateName": "outer"}));
        let expanded = expand_steps(&spec, &[outer], &Variables::new()).unwrap();
        assert_eq!(commands_of(&expanded), ["echo a", "echo b"]);

        let looping = step(json!({"type": "UseTemplateStep", "templateName": "loop"}));
        assert!(
            expand_steps(&spec, &[looping], &Variables::new())
                .unwrap_err()
                .contains("nested")
        );
    }

    #[test]
    fn the_use_steps_condition_and_optional_flag_apply_to_its_children() {
        let spec = spec_with(json!({
            "name": "t",
            "steps": [command("plain", "echo 1"), {"type": "CommandStep", "name": "own", "condition": "FAILED",
                "interpreter": {"type": "PosixInterpreter", "commands": "echo 2"}}]
        }));
        let always = step(
            json!({"type": "UseTemplateStep", "templateName": "t", "condition": "ALWAYS", "optional": true}),
        );
        let expanded = expand_steps(&spec, &[always], &Variables::new()).unwrap();
        assert_eq!(expanded[0].condition, ExecuteCondition::Always);
        assert_eq!(
            expanded[1].condition,
            ExecuteCondition::Failed,
            "an explicit child condition wins"
        );
        assert!(expanded.iter().all(|s| s.optional));
    }
}
