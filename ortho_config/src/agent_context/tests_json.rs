//! JSON serialization tests for the compact agent-context schema.
use super::{field, first_array_item, sample_agent_context};
use crate::agent_context::{AGENT_CONTEXT_KIND_SUFFIX, AgentContext};
use crate::{serialize_agent_context, serialize_agent_context_pretty};
use anyhow::{Result, ensure};
use rstest::rstest;
use serde_json::{Value, json};

#[rstest]
fn to_json_is_valid_parseable_json() {
    let context = sample_agent_context();
    let json = serialize_agent_context(&context).expect("serialize compact agent context");
    let value: Value = serde_json::from_str(&json).expect("parse compact agent context JSON");

    assert!(value.is_object());
}

#[rstest]
fn to_json_round_trips_via_serde() {
    let context = sample_agent_context();
    let json = serialize_agent_context(&context).expect("serialize compact agent context");
    let parsed: AgentContext = serde_json::from_str(&json).expect("parse compact agent context");

    assert_eq!(parsed, context);
}

#[rstest]
fn to_json_is_deterministic() {
    let context = sample_agent_context();

    assert_eq!(
        serialize_agent_context(&context).expect("serialize compact agent context"),
        serialize_agent_context(&context).expect("serialize compact agent context")
    );
}

#[rstest]
fn to_json_includes_kind_and_schema_version() {
    let context = sample_agent_context();
    let json = serialize_agent_context(&context).expect("serialize compact agent context");
    let value: Value = serde_json::from_str(&json).expect("parse compact agent context JSON");

    let schema_version =
        field(&value, "schema_version").expect("serialized context should carry a schema version");
    let kind = field(&value, "kind").expect("serialized context should carry a kind");
    assert_eq!(schema_version, crate::ORTHO_AGENT_CONTEXT_SCHEMA_VERSION);
    assert!(
        kind.as_str()
            .is_some_and(|kind_text| kind_text.ends_with(AGENT_CONTEXT_KIND_SUFFIX))
    );
}

#[rstest]
fn to_json_has_trailing_newline() {
    let context = sample_agent_context();
    let json = serialize_agent_context(&context).expect("serialize compact agent context");

    assert!(json.ends_with('\n'));
    assert!(!json.trim_end().contains('\n'));
}

#[rstest]
fn pretty_json_is_indented_without_a_trailing_newline() {
    let context = sample_agent_context();
    let json = serialize_agent_context_pretty(&context).expect("serialize pretty agent context");

    assert!(!json.ends_with('\n'));
    assert!(json.contains("\n  \"schema_version\": \"1\","));
}

#[test]
fn compact_context_serialization_excludes_localization_fields() -> Result<()> {
    let context = sample_agent_context();

    let value = serde_json::to_value(context).expect("serialize agent context");
    assert_context_identity_fields(&value)?;
    let command = first_array_item(field(&value, "commands")?)?;
    assert_command_policy_fields(command)?;
    assert_localization_fields_are_absent(&value, command)
}

/// Ensures the named field serializes to the expected wire string.
fn ensure_field_eq(value: &Value, name: &str, expected: &str) -> Result<()> {
    let actual = field(value, name)?;
    ensure!(
        actual == expected,
        "`{name}` should serialize as {expected:?}, got {actual}"
    );
    Ok(())
}

fn assert_context_identity_fields(value: &Value) -> Result<()> {
    ensure_field_eq(value, "schema_version", "1")?;
    ensure_field_eq(value, "kind", "example-cli.agent_context")
}

fn assert_command_policy_fields(command: &Value) -> Result<()> {
    ensure_field_eq(command, "interaction_mode", "non_interactive")?;
    ensure_field_eq(command, "mutation_effect", "read_only")?;
    ensure_field_eq(field(command, "async_submission")?, "mode", "submit")?;
    ensure_field_eq(field(command, "delivery_route")?, "target", "file")
}

fn assert_localization_fields_are_absent(value: &Value, command: &Value) -> Result<()> {
    ensure!(
        value.get("about_id").is_none(),
        "compact context must not carry `about_id`"
    );
    ensure!(
        value.get("headings_ids").is_none(),
        "compact context must not carry `headings_ids`"
    );
    ensure!(
        command.get("help_id").is_none(),
        "compact command must not carry `help_id`"
    );
    Ok(())
}

/// The unsupported profiles declaration serializes byte-identically to the
/// legacy `{ "supported": false }` (decision D7).
#[test]
fn unsupported_profiles_serialize_byte_identically_to_legacy() {
    let context = sample_agent_context();
    let json = serialize_agent_context(&context).expect("serialize compact agent context");
    let value: Value = serde_json::from_str(&json).expect("parse compact agent context JSON");
    let profiles = value.get("profiles").expect("profiles key present");
    assert_eq!(
        profiles,
        &json!({ "supported": false }),
        "unsupported profiles must serialize byte-identically to the legacy shape"
    );
    assert!(
        profiles.get("selection").is_none(),
        "the selection field must be omitted when absent"
    );
    assert!(
        profiles.get("list_command").is_none(),
        "the list_command field must be omitted when absent"
    );
}
