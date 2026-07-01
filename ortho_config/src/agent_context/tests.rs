//! Tests for the compact agent-context schema.

use super::*;
use crate::docs::ORTHO_DOCS_IR_VERSION;
use camino::Utf8PathBuf;
use insta::assert_snapshot;
use proptest::{collection::vec, option, prelude::*};
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
fn agent_context_command_name_const_is_context() {
    assert_eq!(crate::AGENT_CONTEXT_COMMAND, "context");
}

#[rstest]
fn agent_context_json_flag_const_is_long_json() {
    assert_eq!(crate::AGENT_CONTEXT_JSON_FLAG, "json");
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
#[case::hyphenated("example-cli")]
#[case::underscored("hello_world")]
fn new_uses_agent_context_kind(#[case] package: &str) {
    let context = AgentContext::new(package);

    assert_eq!(context.kind, crate::agent_context_kind(package));
}

#[rstest]
fn to_json_is_valid_parseable_json() {
    let context = sample_agent_context();
    let json = context.to_json().expect("serialize compact agent context");
    let value: Value = serde_json::from_str(&json).expect("parse compact agent context JSON");

    assert!(value.is_object());
}

#[rstest]
fn to_json_round_trips_via_serde() {
    let context = sample_agent_context();
    let json = context.to_json().expect("serialize compact agent context");
    let parsed: AgentContext = serde_json::from_str(&json).expect("parse compact agent context");

    assert_eq!(parsed, context);
}

#[rstest]
fn to_json_is_deterministic() {
    let context = sample_agent_context();

    assert_eq!(
        context.to_json().expect("serialize compact agent context"),
        context.to_json().expect("serialize compact agent context")
    );
}

#[rstest]
fn to_json_includes_kind_and_schema_version() {
    let context = sample_agent_context();
    let json = context.to_json().expect("serialize compact agent context");
    let value: Value = serde_json::from_str(&json).expect("parse compact agent context JSON");

    assert_eq!(
        field(&value, "schema_version"),
        ORTHO_AGENT_CONTEXT_SCHEMA_VERSION
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
    let json = context.to_json().expect("serialize compact agent context");

    assert!(json.ends_with('\n'));
}

#[rstest]
fn to_json_pretty_has_no_trailing_newline() {
    let context = sample_agent_context();
    let json = context
        .to_json_pretty()
        .expect("serialize pretty agent context");

    assert!(!json.ends_with('\n'));
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

proptest! {
    #[test]
    fn to_json_always_round_trips(context in any_agent_context()) {
        let json = context.to_json().expect("serialize compact agent context");
        let parsed: AgentContext =
            serde_json::from_str(&json).expect("parse compact agent context");

        prop_assert_eq!(parsed, context);
    }
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

fn any_agent_context() -> impl Strategy<Value = AgentContext> {
    (package_name(), vec(any_agent_command(), 0..4)).prop_map(|(package, commands)| AgentContext {
        schema_version: ORTHO_AGENT_CONTEXT_SCHEMA_VERSION.to_owned(),
        kind: crate::agent_context_kind(&package),
        package,
        commands,
        profiles: SupportDeclaration { supported: false },
        feedback: SupportDeclaration { supported: false },
        policy: AgentPolicy {
            agent_native: PolicyMode::Warn,
        },
        skill_manifests: Vec::new(),
    })
}

fn any_agent_command() -> impl Strategy<Value = AgentCommand> {
    (
        vec(command_segment(), 1..4),
        option::of(summary()),
        interaction_mode(),
        mutation_effect(),
    )
        .prop_map(
            |(path, summary, interaction_mode, mutation_effect)| AgentCommand {
                path,
                summary,
                canonical_verb: None,
                inputs: Vec::new(),
                output_modes: vec!["json".to_owned()],
                interaction_mode,
                mutation_effect,
                async_submission: None,
                delivery_route: None,
                pagination: None,
                examples: Vec::new(),
            },
        )
}

fn interaction_mode() -> impl Strategy<Value = InteractionMode> {
    prop_oneof![
        Just(InteractionMode::Unknown),
        Just(InteractionMode::NonInteractive),
        Just(InteractionMode::Interactive),
    ]
}

fn mutation_effect() -> impl Strategy<Value = MutationEffect> {
    prop_oneof![
        Just(MutationEffect::Unknown),
        Just(MutationEffect::ReadOnly),
        Just(MutationEffect::Write),
        Just(MutationEffect::Delete),
        Just(MutationEffect::Submit),
    ]
}

fn package_name() -> impl Strategy<Value = String> {
    "[A-Za-z0-9_.-]{0,16}"
}

fn command_segment() -> impl Strategy<Value = String> {
    "[a-z][a-z0-9-]{0,12}"
}

fn summary() -> impl Strategy<Value = String> {
    "[A-Za-z0-9 .,;-]{0,48}"
}
