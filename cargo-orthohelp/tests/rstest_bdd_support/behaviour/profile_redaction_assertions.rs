//! Projection assertions shared by the profile-redaction scenarios.
//!
//! Each function inspects one generated document and returns the scenario
//! failure rather than panicking, so a broken projection names the locale and
//! field that disclosed it. The step modules read as Gherkin; the mechanics of
//! walking the agent-context and documentation schemas live here.

use serde_json::Value;

use super::profile_redaction_support::{command_nodes, context_input, ir_field, require};
use super::steps::StepResult;

pub(super) fn require_profiles_supported(context: &Value) -> StepResult<()> {
    require(
        context
            .get("profiles")
            .and_then(|profiles| profiles.get("supported"))
            == Some(&Value::Bool(true)),
        "profiles should be supported",
    )
}

pub(super) fn redaction_records(context: &Value) -> StepResult<&[Value]> {
    context
        .get("profiles")
        .and_then(|profiles| profiles.get("redaction"))
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .ok_or_else(|| "redaction list missing from profiles".into())
}

pub(super) fn find_redaction<'a>(
    records: &'a [Value],
    path: &[&str],
    field: &str,
) -> Option<&'a Value> {
    records.iter().find(|record| {
        record
            .get("field_name")
            .and_then(Value::as_str)
            .is_some_and(|name| name == field)
            && record
                .get("command_path")
                .and_then(Value::as_array)
                .is_some_and(|actual| {
                    actual
                        .iter()
                        .filter_map(Value::as_str)
                        .eq(path.iter().copied())
                })
    })
}

pub(super) fn require_records(records: &[Value], field: &str, exposure: &str) -> StepResult<()> {
    let record = find_redaction(records, &["redaction_fixture"], field)
        .ok_or_else(|| format!("missing redaction record for {field}"))?;
    require(
        record.get("exposure").and_then(Value::as_str) == Some(exposure),
        format!("{field} should be classified {exposure}"),
    )
}

pub(super) fn context_enum_values(input: &Value) -> Vec<&str> {
    input
        .get("enum_values")
        .and_then(Value::as_array)
        .map(|values| values.iter().filter_map(Value::as_str).collect())
        .unwrap_or_default()
}

pub(super) fn require_context_input_default(
    command: &Value,
    name: &str,
    expected: &str,
) -> StepResult<()> {
    let input = context_input(command, name).ok_or_else(|| format!("{name} input missing"))?;
    let default = input.get("default").and_then(Value::as_str);
    require(
        default.is_some_and(|value| value.contains(expected)),
        format!("{name} should keep a default containing {expected:?}, got {default:?}"),
    )
}

pub(super) fn require_context_enum_values(
    command: &Value,
    name: &str,
    expected: &[&str],
) -> StepResult<()> {
    let input = context_input(command, name).ok_or_else(|| format!("{name} input missing"))?;
    let actual = context_enum_values(input);
    require(
        actual == expected,
        format!("{name} enum values should be {expected:?}, got {actual:?}"),
    )
}

pub(super) fn assert_public_ir_fields(ir: &Value, locale: &str) -> StepResult<()> {
    for (name, fragment) in [("region", "eu-west"), ("mode", "fast")] {
        let field = ir_field(ir, name).ok_or_else(|| format!("{locale}: {name} field missing"))?;
        require(
            default_display(field).is_some_and(|display| display.contains(fragment)),
            format!("{locale}: {name} lost its public default"),
        )?;
    }
    let mode = ir_field(ir, "mode").ok_or("mode field missing")?;
    let values = cli_possible_values(mode).ok_or("mode possible values missing")?;
    require(
        values.iter().filter_map(Value::as_str).eq(["fast", "safe"]),
        format!("{locale}: mode possible values should stay public"),
    )?;
    for name in ["token", "locator"] {
        assert_protected_ir_field(ir, name, locale)?;
    }
    let rotate = command_nodes(ir)
        .into_iter()
        .find(|node| command_name(node) == "rotate")
        .ok_or_else(|| format!("{locale}: rotate command missing"))?;
    assert_protected_ir_field(rotate, "replacement", locale)
}

pub(super) fn assert_protected_ir_field(node: &Value, name: &str, locale: &str) -> StepResult<()> {
    let field = ir_field(node, name).ok_or_else(|| format!("{locale}: {name} field missing"))?;
    require(
        field.get("default").is_some_and(Value::is_null),
        format!("{locale}: {name} retained a protected default"),
    )?;
    require(
        field
            .get("examples")
            .and_then(Value::as_array)
            .is_some_and(Vec::is_empty),
        format!("{locale}: {name} retained protected examples"),
    )?;
    require(
        cli_possible_values(field).is_some_and(Vec::is_empty),
        format!("{locale}: {name} retained protected enum members"),
    )?;
    let help = field
        .get("help")
        .and_then(Value::as_str)
        .unwrap_or_default();
    require(
        !help.is_empty() && !help.starts_with("[missing:"),
        format!("{locale}: {name} should keep its public description"),
    )
}

pub(super) fn assert_ordinary_ir_examples(ir: &Value, locale: &str) -> StepResult<()> {
    let examples = section_examples(ir).ok_or("command examples are not an array")?;
    require(
        examples.iter().any(|example| {
            example.get("code").and_then(Value::as_str) == Some("ordinary-fixture --name demo")
        }),
        format!("{locale}: the command example was not retained"),
    )?;
    let name = ir_field(ir, "name").ok_or("name field missing")?;
    require(
        default_display(name) == Some("String :: from(\"plain-default\")"),
        format!("{locale}: the public default was not retained"),
    )?;
    let codes = field_example_codes(name);
    require(
        codes.contains(&"ordinary-fixture --name plain-example"),
        format!("{locale}: the field example was not retained, got {codes:?}"),
    )
}

pub(super) fn section_examples(command: &Value) -> Option<&Vec<Value>> {
    command.get("sections")?.get("examples")?.as_array()
}

pub(super) fn command_name(command: &Value) -> &str {
    command
        .get("app_name")
        .and_then(Value::as_str)
        .unwrap_or("<unnamed>")
}

pub(super) fn default_display(field: &Value) -> Option<&str> {
    field.get("default")?.get("display")?.as_str()
}

pub(super) fn cli_possible_values(field: &Value) -> Option<&Vec<Value>> {
    field.get("cli")?.get("possible_values")?.as_array()
}

pub(super) fn field_example_codes(field: &Value) -> Vec<&str> {
    field
        .get("examples")
        .and_then(Value::as_array)
        .map(|examples| {
            examples
                .iter()
                .filter_map(|example| example.get("code").and_then(Value::as_str))
                .collect()
        })
        .unwrap_or_default()
}
