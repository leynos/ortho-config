//! JSON serialization tests for the compact agent-context schema.

use super::{field, first_array_item, sample_agent_context};
use crate::agent_context::{AGENT_CONTEXT_KIND_SUFFIX, AgentContext};
use crate::{serialize_agent_context, serialize_agent_context_pretty};
use rstest::rstest;
use serde_json::Value;

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

    assert_eq!(
        field(&value, "schema_version"),
        crate::ORTHO_AGENT_CONTEXT_SCHEMA_VERSION
    );
    assert!(
        field(&value, "kind")
            .as_str()
            .is_some_and(|kind| kind.ends_with(AGENT_CONTEXT_KIND_SUFFIX))
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
fn compact_context_serialization_excludes_localization_fields() {
    let context = sample_agent_context();

    let value = serde_json::to_value(context).expect("serialize agent context");
    assert_context_identity_fields(&value);
    let command = first_array_item(field(&value, "commands"));
    assert_command_policy_fields(command);
    assert_localization_fields_are_absent(&value, command);
}

fn assert_context_identity_fields(value: &Value) {
    assert_eq!(field(value, "schema_version"), "1");
    assert_eq!(field(value, "kind"), "example-cli.agent_context");
}

fn assert_command_policy_fields(command: &Value) {
    assert_eq!(field(command, "interaction_mode"), "non_interactive");
    assert_eq!(field(command, "mutation_effect"), "read-only");
    assert_eq!(field(field(command, "async_submission"), "mode"), "submit");
    assert_eq!(field(field(command, "delivery_route"), "target"), "file");
}

fn assert_localization_fields_are_absent(value: &Value, command: &Value) {
    assert!(value.get("about_id").is_none());
    assert!(value.get("headings_ids").is_none());
    assert!(command.get("help_id").is_none());
}
