//! Tests for the compact agent-context schema.

use super::*;
use crate::docs::ORTHO_DOCS_IR_VERSION;
use camino::Utf8PathBuf;
use insta::assert_snapshot;
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
    assert!(context.skill_manifests.is_empty());
}

#[rstest]
fn compact_context_serialization_excludes_localization_fields() {
    let context = sample_agent_context();

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
    assert_snapshot!(json);
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
    assert!(command.summary.is_none());
    assert!(command.async_submission.is_none());
    assert!(command.delivery_route.is_none());
    assert!(command.inputs.is_empty());
    assert!(!context.profiles.supported);
    assert!(!context.feedback.supported);
    assert_eq!(context.policy.agent_native, PolicyMode::Warn);
    assert!(context.skill_manifests.is_empty());
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
    let Some(field) = value.get(name) else {
        panic!("JSON object should contain `{name}`");
    };
    field
}

fn first_array_item(value: &Value) -> &Value {
    let Some(item) = value.as_array().and_then(|items| items.first()) else {
        panic!("JSON value should be a non-empty array");
    };
    item
}

fn sample_agent_context() -> AgentContext {
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
