//! JSON serialization tests for the compact agent-context schema.

use super::{field, first_array_item, sample_agent_context};
use crate::agent_context::{AGENT_CONTEXT_KIND_SUFFIX, AgentCommand, AgentContext};
use crate::{serialize_agent_context, serialize_agent_context_pretty};
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
    assert_eq!(field(command, "mutation_effect"), "read_only");
    assert_eq!(field(field(command, "async_submission"), "mode"), "submit");
    assert_eq!(field(field(command, "delivery_route"), "target"), "file");
}

fn assert_localization_fields_are_absent(value: &Value, command: &Value) {
    assert!(value.get("about_id").is_none());
    assert!(value.get("headings_ids").is_none());
    assert!(command.get("help_id").is_none());
}

#[rstest]
fn absent_bypass_and_dry_run_flags_serialize_as_explicit_nulls() {
    let mut context = sample_agent_context();
    let command = context
        .commands
        .first_mut()
        .expect("sample context should contain one command");
    command.bypass_flag = None;
    command.dry_run_flag = None;

    let value = serde_json::to_value(context).expect("serialize agent context");
    let serialized_command = first_array_item(field(&value, "commands"));

    assert!(field(serialized_command, "bypass_flag").is_null());
    assert!(field(serialized_command, "dry_run_flag").is_null());
}

#[rstest]
fn declared_bypass_and_dry_run_flags_round_trip() {
    let mut command = sample_agent_context()
        .commands
        .pop()
        .expect("sample context should contain one command");
    command.interaction_mode = super::InteractionMode::Interactive;
    command.mutation_effect = super::MutationEffect::Delete;
    command.bypass_flag = Some("--force".to_owned());
    command.dry_run_flag = Some("--dry-run".to_owned());

    let serialized = serde_json::to_value(&command).expect("serialize agent command");
    assert_eq!(field(&serialized, "bypass_flag"), "--force");
    assert_eq!(field(&serialized, "dry_run_flag"), "--dry-run");

    let parsed: AgentCommand =
        serde_json::from_value(serialized).expect("parse agent command with declared flags");
    assert_eq!(parsed, command);
}

#[rstest]
fn legacy_commands_without_flag_fields_deserialize_with_nulls() {
    let context: AgentContext = serde_json::from_value(json!({
        "schema_version": "1",
        "kind": "legacy-cli.agent_context",
        "package": "legacy-cli",
        "commands": [
            {
                "path": ["legacy-cli", "purge"]
            }
        ]
    }))
    .expect("deserialize context without bypass or dry-run flags");

    let command = context
        .commands
        .first()
        .expect("legacy context should contain one command");
    assert!(command.bypass_flag.is_none());
    assert!(command.dry_run_flag.is_none());
}
