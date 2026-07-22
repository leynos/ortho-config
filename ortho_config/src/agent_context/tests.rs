//! Tests for the compact agent-context schema.

use super::*;
use camino::Utf8PathBuf;
use insta::assert_snapshot;
use rstest::rstest;
use serde_json::{Value, json};
#[path = "tests_json.rs"]
mod json;

#[path = "tests_contract_support.rs"]
mod contract_support;
use contract_support::*;

#[path = "tests_round_trip.rs"]
mod round_trip;

#[rstest]
fn agent_context_version_is_independent_from_docs_ir() {
    assert_agent_context_version_metadata();
}

#[rstest]
#[case::hyphenated("example-cli", "example-cli.agent_context")]
#[case::underscored("hello_world", "hello_world.agent_context")]
#[case::empty("", ".agent_context")]
#[case::dotted("ns.tool", "ns.tool.agent_context")]
#[case::mixed("Foo-Bar_9", "Foo-Bar_9.agent_context")]
fn agent_context_kind_appends_suffix(#[case] package: &str, #[case] expected: &str) {
    assert_eq!(crate::agent_context_kind(package), expected);
}

#[test]
fn new_context_uses_legacy_defaults() {
    let context = AgentContext::new("example-cli");

    assert_legacy_default_context(&context);
}

#[rstest]
#[case::hyphenated("example-cli")]
#[case::underscored("hello_world")]
fn new_uses_agent_context_kind(#[case] package: &str) {
    let context = AgentContext::new(package);

    assert_eq!(context.kind, crate::agent_context_kind(package));
}

#[rstest]
#[case(None, false)]
#[case(Some("List configured resources."), true)]
fn command_summary_serializes_only_when_present(
    #[case] summary: Option<&str>,
    #[case] should_include_summary: bool,
) {
    let mut context = sample_agent_context();
    context
        .commands
        .first_mut()
        .expect("sample context should contain one command")
        .summary = summary.map(str::to_owned);

    let value = serde_json::to_value(context).expect("serialize agent context");
    let command = first_array_item(field(&value, "commands"));

    assert_eq!(command.get("summary").is_some(), should_include_summary);
    if let Some(expected) = summary {
        assert_eq!(field(command, "summary"), expected);
    }
}

#[rstest]
fn command_summary_round_trips_when_present() {
    let context: AgentContext = serde_json::from_value(json!({
        "schema_version": "1",
        "kind": "summary-cli.agent_context",
        "package": "summary-cli",
        "commands": [
            {
                "path": ["summary-cli", "list"],
                "summary": "List configured resources."
            }
        ]
    }))
    .expect("deserialize context with command summary");

    let command = context
        .commands
        .first()
        .expect("context should contain one command");
    assert_eq!(
        command.summary.as_deref(),
        Some("List configured resources.")
    );
}

#[rstest]
fn skill_manifest_default_is_empty_list() {
    let context: AgentContext = serde_json::from_value(json!({
        "schema_version": "1",
        "kind": "legacy-cli.agent_context",
        "package": "legacy-cli",
        "commands": []
    }))
    .expect("deserialize context without skill manifests");

    assert!(context.skill_manifests.is_empty());
}

#[rstest]
fn skill_manifest_serialises_with_camino_path() {
    let manifest = SkillManifest {
        id: "rename".to_owned(),
        path: Utf8PathBuf::from("skills/rename.md"),
        manifest_schema_version: "v1".to_owned(),
        commands: vec![SkillCommandRef {
            path: vec!["weaver".to_owned(), "rename".to_owned()],
            flags: vec!["json".to_owned()],
        }],
    };

    let value = serde_json::to_value(&manifest).expect("serialize skill manifest");
    assert_eq!(field(&value, "path"), "skills/rename.md");
    let json = serde_json::to_string_pretty(&manifest).expect("serialize skill manifest snapshot");

    assert_snapshot!(json, @r###"
    {
      "id": "rename",
      "path": "skills/rename.md",
      "manifest_schema_version": "v1",
      "commands": [
        {
          "path": [
            "weaver",
            "rename"
          ],
          "flags": [
            "json"
          ]
        }
      ]
    }
    "###);
}

#[rstest]
fn skill_command_ref_defaults_flags_to_empty() {
    let command: SkillCommandRef = serde_json::from_value(json!({
        "path": ["weaver", "rename"]
    }))
    .expect("deserialize command ref without flags");

    assert!(command.flags.is_empty());
}

#[rstest]
#[case(json!({
    "path": "skills/rename.md",
    "manifest_schema_version": "v1",
    "commands": []
}))]
#[case(json!({
    "id": "rename",
    "manifest_schema_version": "v1",
    "commands": []
}))]
#[case(json!({
    "id": "rename",
    "path": "skills/rename.md",
    "commands": []
}))]
fn skill_manifest_required_fields_fail_deserialization(#[case] payload: Value) {
    let error = serde_json::from_value::<SkillManifest>(payload)
        .expect_err("missing required skill manifest fields should fail");

    assert!(
        error.is_data() || error.is_syntax(),
        "expected a data or syntax error, got {error}"
    );
}

#[rstest]
fn agent_context_json_snapshot_covers_wire_contract() {
    let json = serde_json::to_string_pretty(&sample_agent_context())
        .expect("serialize agent context snapshot");

    assert_eq!(json, AGENT_CONTEXT_WIRE_CONTRACT_JSON);
}

#[rstest]
fn absent_optional_fields_serialize_with_documented_nulls() {
    let mut context = sample_agent_context();
    let mutable_command = context
        .commands
        .first_mut()
        .expect("sample context should contain one command");
    mutable_command.summary = None;
    mutable_command.canonical_verb = None;
    mutable_command.async_submission = None;
    mutable_command.delivery_route = None;
    mutable_command.pagination = None;
    mutable_command
        .inputs
        .first_mut()
        .expect("sample command should contain one input")
        .default = None;
    mutable_command
        .examples
        .first_mut()
        .expect("sample command should contain one example")
        .output_mode = None;

    let value = serde_json::to_value(context).expect("serialize agent context");
    assert_optional_command_fields_are_null(&value);
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
    assert_legacy_omission_defaults(&context, command);
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
fn unknown_top_level_fields_are_ignored_for_forward_compatibility() {
    let payload = json!({
        "schema_version": "1",
        "kind": "future-cli.agent_context",
        "package": "future-cli",
        "commands": [],
        "future_optional_field": {
            "producer": "newer-ortho-config"
        }
    });

    let context = serde_json::from_value::<AgentContext>(payload)
        .expect("unknown fields should be ignored within the same major version");

    assert_eq!(context.package, "future-cli");
    assert!(context.commands.is_empty());
}

#[rstest]
#[case(AsyncSubmissionMode::Inline, "inline")]
#[case(AsyncSubmissionMode::Submit, "submit")]
fn async_submission_mode_serializes_canonical_wire_values(
    #[case] mode: AsyncSubmissionMode,
    #[case] expected: &str,
) {
    let value = serde_json::to_value(mode).expect("serialize async submission mode");
    assert_eq!(value, expected);
}

#[rstest]
#[case(InteractionMode::Unknown, "unknown")]
#[case(InteractionMode::NonInteractive, "non_interactive")]
#[case(InteractionMode::Interactive, "interactive")]
fn interaction_mode_serializes_canonical_wire_values(
    #[case] mode: InteractionMode,
    #[case] expected: &str,
) {
    let value = serde_json::to_value(mode).expect("serialize interaction mode");
    assert_eq!(value, expected);
}

#[rstest]
#[case(PolicyMode::Off, "off")]
#[case(PolicyMode::Warn, "warn")]
#[case(PolicyMode::Deny, "deny")]
fn policy_mode_serializes_canonical_wire_values(#[case] mode: PolicyMode, #[case] expected: &str) {
    let value = serde_json::to_value(mode).expect("serialize policy mode");
    assert_eq!(value, expected);
}

#[rstest]
#[case(MutationEffect::Unknown, "unknown")]
#[case(MutationEffect::ReadOnly, "read_only")]
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

pub(super) fn field<'a>(value: &'a Value, name: &str) -> &'a Value {
    let Some(field) = value.get(name) else {
        panic!("JSON object should contain `{name}`");
    };
    field
}

pub(super) fn first_array_item(value: &Value) -> &Value {
    let Some(item) = value.as_array().and_then(|items| items.first()) else {
        panic!("JSON value should be a non-empty array");
    };
    item
}

pub(super) fn sample_agent_context() -> AgentContext {
    AgentContext {
        schema_version: ORTHO_AGENT_CONTEXT_SCHEMA_VERSION.to_owned(),
        kind: "example-cli.agent_context".to_owned(),
        package: "example-cli".to_owned(),
        commands: vec![AgentCommand {
            path: vec!["example-cli".to_owned(), "list".to_owned()],
            summary: Some("List configured resources.".to_owned()),
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
            pagination: Some(PaginationContract {
                limit_input: Some("limit".to_owned()),
                cursor_input: Some("cursor".to_owned()),
            }),
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
        skill_manifests: vec![SkillManifest {
            id: "example-list".to_owned(),
            path: Utf8PathBuf::from("skills/example-list.md"),
            manifest_schema_version: "v1".to_owned(),
            commands: vec![SkillCommandRef {
                path: vec!["example-cli".to_owned(), "list".to_owned()],
                flags: vec!["format".to_owned()],
            }],
        }],
    }
}
