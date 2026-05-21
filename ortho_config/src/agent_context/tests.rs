//! Tests for the compact agent-context schema.

use super::*;
use crate::docs::ORTHO_DOCS_IR_VERSION;
use rstest::rstest;
use serde_json::{Value, json};

#[rstest]
fn agent_context_version_is_independent_from_docs_ir() {
    assert_eq!(ORTHO_AGENT_CONTEXT_SCHEMA_VERSION, "1");
    assert_ne!(
        ORTHO_AGENT_CONTEXT_SCHEMA_VERSION, ORTHO_DOCS_IR_VERSION,
        "agent context must not share the documentation IR version"
    );
}

#[rstest]
fn new_context_uses_legacy_defaults() {
    let context = AgentContext::new("example-cli");

    assert_eq!(context.schema_version, ORTHO_AGENT_CONTEXT_SCHEMA_VERSION);
    assert_eq!(context.kind, "example-cli.agent_context");
    assert_eq!(context.package, "example-cli");
    assert!(context.commands.is_empty());
    assert!(!context.profiles.supported);
    assert!(!context.feedback.supported);
    assert_eq!(context.policy.agent_native, PolicyMode::Warn);
}

#[rstest]
fn compact_context_serialization_excludes_localization_fields() {
    let context = AgentContext {
        schema_version: ORTHO_AGENT_CONTEXT_SCHEMA_VERSION.to_owned(),
        kind: "example-cli.agent_context".to_owned(),
        package: "example-cli".to_owned(),
        commands: vec![AgentCommand {
            path: vec!["example-cli".to_owned(), "list".to_owned()],
            canonical_verb: Some("list".to_owned()),
            inputs: vec![AgentInput {
                name: "format".to_owned(),
                long: Some("format".to_owned()),
                value_type: Some("string".to_owned()),
                required: false,
                default: Some("json".to_owned()),
                enum_values: vec!["json".to_owned()],
            }],
            output_modes: vec!["json".to_owned()],
            interaction_mode: InteractionMode::NonInteractive,
            mutation_effect: MutationEffect::ReadOnly,
            async_submission: Some(AsyncSubmission {
                mode: AsyncSubmissionMode::Submit,
                noun: Some("job".to_owned()),
            }),
            delivery_route: Some(DeliveryRoute {
                supported: true,
                target: Some("file".to_owned()),
            }),
            pagination: None,
            examples: vec![AgentExample {
                command: "example-cli list --format json".to_owned(),
                output_mode: Some("json".to_owned()),
            }],
        }],
        profiles: SupportDeclaration { supported: false },
        feedback: SupportDeclaration { supported: false },
        policy: AgentPolicy {
            agent_native: PolicyMode::Warn,
        },
    };

    let value = serde_json::to_value(context).expect("serialize agent context");
    assert_eq!(field(&value, "schema_version"), "1");
    assert_eq!(field(&value, "kind"), "example-cli.agent_context");
    let command = first_array_item(field(&value, "commands"));
    assert_eq!(field(command, "interaction_mode"), "non_interactive");
    assert_eq!(field(command, "mutation_effect"), "read-only");
    assert_eq!(field(field(command, "async_submission"), "mode"), "submit");
    assert_eq!(field(field(command, "delivery_route"), "target"), "file");
    assert!(value.get("about_id").is_none());
    assert!(value.get("headings_ids").is_none());
    assert!(command.get("help_id").is_none());
}

#[rstest]
fn absent_optional_metadata_deserializes_to_documented_defaults() {
    let context: AgentContext = serde_json::from_value(json!({
        "schema_version": "1",
        "kind": "legacy-cli.agent_context",
        "package": "legacy-cli",
        "commands": [
            {
                "path": ["legacy-cli"]
            }
        ]
    }))
    .expect("deserialize context with legacy omissions");

    let command = context
        .commands
        .first()
        .expect("legacy context should contain one command");
    assert_eq!(command.interaction_mode, InteractionMode::Unknown);
    assert_eq!(command.mutation_effect, MutationEffect::Unknown);
    assert!(command.async_submission.is_none());
    assert!(command.delivery_route.is_none());
    assert!(command.inputs.is_empty());
    assert!(!context.profiles.supported);
    assert!(!context.feedback.supported);
    assert_eq!(context.policy.agent_native, PolicyMode::Warn);
}

#[rstest]
#[case(json!({"package": "missing-version", "commands": []}))]
#[case(json!({"schema_version": "1", "commands": []}))]
#[case(json!({"schema_version": "1", "package": "missing-commands"}))]
#[case(json!({"schema_version": "1", "package": "missing-kind", "commands": []}))]
fn missing_required_top_level_fields_fail_deserialization(#[case] payload: Value) {
    let error = serde_json::from_value::<AgentContext>(payload)
        .expect_err("missing required schema fields should fail");

    assert!(
        error.is_data() || error.is_syntax(),
        "expected a data or syntax error, got {error}"
    );
}

#[rstest]
#[case(MutationEffect::Unknown, "unknown")]
#[case(MutationEffect::ReadOnly, "read-only")]
#[case(MutationEffect::Write, "write")]
#[case(MutationEffect::Delete, "delete")]
#[case(MutationEffect::Submit, "submit")]
fn mutation_effect_serializes_canonical_wire_values(
    #[case] effect: MutationEffect,
    #[case] expected: &str,
) {
    let value = serde_json::to_value(effect).expect("serialize mutation effect");
    assert_eq!(value, expected);
}

fn field<'a>(value: &'a Value, name: &str) -> &'a Value {
    value
        .get(name)
        .unwrap_or_else(|| panic!("JSON object should contain `{name}`"))
}

fn first_array_item(value: &Value) -> &Value {
    value
        .as_array()
        .and_then(|items| items.first())
        .expect("JSON value should be a non-empty array")
}
