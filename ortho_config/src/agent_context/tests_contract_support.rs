//! Assertion helpers and expected JSON for agent-context schema tests.

use crate::agent_context::{
    AGENT_CONTEXT_KIND_SUFFIX, AgentCommand, AgentContext, AgentExample, AgentInput, AgentPolicy,
    AsyncSubmission, AsyncSubmissionMode, DeliveryRoute, InteractionMode, MutationEffect,
    ORTHO_AGENT_CONTEXT_SCHEMA_VERSION, PaginationContract, PolicyException, PolicyMode,
    SkillCommandRef, SkillManifest, SupportDeclaration,
};
use crate::docs::ORTHO_DOCS_IR_VERSION;
use camino::Utf8PathBuf;
use serde_json::Value;

/// Asserts the schema identity constants and their independence from the
/// documentation IR version.
pub(super) fn assert_agent_context_version_metadata() {
    assert_agent_context_schema_identity();
    assert_agent_context_schema_independence();
}

fn assert_agent_context_schema_identity() {
    assert_eq!(ORTHO_AGENT_CONTEXT_SCHEMA_VERSION, "1");
    assert_eq!(AGENT_CONTEXT_KIND_SUFFIX, "agent_context");
}

fn assert_agent_context_schema_independence() {
    assert!(
        AgentContext::new("example-cli")
            .kind
            .ends_with(AGENT_CONTEXT_KIND_SUFFIX)
    );
    assert_ne!(
        ORTHO_AGENT_CONTEXT_SCHEMA_VERSION, ORTHO_DOCS_IR_VERSION,
        "agent context must not share the documentation IR version"
    );
}

/// Asserts the complete legacy-compatible state produced by
/// [`AgentContext::new`].
pub(super) fn assert_legacy_default_context(context: &AgentContext) {
    assert_legacy_default_identity(context);
    assert_legacy_default_support_declarations(context);
    assert_legacy_default_policy_and_skills(context);
}

fn assert_legacy_default_identity(context: &AgentContext) {
    assert_eq!(context.schema_version, ORTHO_AGENT_CONTEXT_SCHEMA_VERSION);
    assert_eq!(context.kind, "example-cli.agent_context");
    assert_eq!(context.package, "example-cli");
}

fn assert_legacy_default_support_declarations(context: &AgentContext) {
    assert!(context.commands.is_empty());
    assert!(!context.profiles.supported);
    assert!(!context.feedback.supported);
}

fn assert_legacy_default_policy_and_skills(context: &AgentContext) {
    assert_eq!(context.policy.agent_native, PolicyMode::Warn);
    assert!(context.policy.exceptions.is_empty());
    assert!(context.skill_manifests.is_empty());
}

/// Asserts the schema-v1 serialization contract for absent optional command,
/// input, and example fields.
pub(super) fn assert_optional_command_fields_are_null(value: &Value) {
    let serialized_command = first_array_item(field(value, "commands"));
    let input = first_array_item(field(serialized_command, "inputs"));
    let example = first_array_item(field(serialized_command, "examples"));

    assert_optional_command_presence_fields_are_null(serialized_command);
    assert_optional_command_route_fields_are_null(serialized_command);
    assert_optional_command_nested_fields_are_null(input, example);
}

fn assert_optional_command_presence_fields_are_null(serialized_command: &Value) {
    assert!(serialized_command.get("summary").is_none());
    assert!(field(serialized_command, "canonical_verb").is_null());
    assert!(field(serialized_command, "async_submission").is_null());
}

fn assert_optional_command_route_fields_are_null(serialized_command: &Value) {
    assert!(field(serialized_command, "delivery_route").is_null());
    assert!(field(serialized_command, "pagination").is_null());
}

fn assert_optional_command_nested_fields_are_null(input: &Value, example: &Value) {
    assert!(field(input, "default").is_null());
    assert!(field(example, "output_mode").is_null());
}

/// Asserts the schema-v1 defaults applied when legacy JSON omits optional
/// command and context metadata.
pub(super) fn assert_legacy_omission_defaults(context: &AgentContext, command: &AgentCommand) {
    assert_legacy_command_modes(command);
    assert_legacy_command_optional_metadata(command);
    assert_legacy_command_collections(command);
    assert_legacy_context_support_defaults(context);
    assert_legacy_context_policy_defaults(context);
}

fn assert_legacy_command_modes(command: &AgentCommand) {
    assert_eq!(command.interaction_mode, InteractionMode::Unknown);
    assert_eq!(command.mutation_effect, MutationEffect::Unknown);
}

fn assert_legacy_command_optional_metadata(command: &AgentCommand) {
    assert!(command.summary.is_none());
    assert!(command.async_submission.is_none());
    assert!(command.delivery_route.is_none());
}

fn assert_legacy_command_collections(command: &AgentCommand) {
    assert!(command.inputs.is_empty());
}

fn assert_legacy_context_support_defaults(context: &AgentContext) {
    assert!(!context.profiles.supported);
    assert!(!context.feedback.supported);
}

fn assert_legacy_context_policy_defaults(context: &AgentContext) {
    assert_eq!(context.policy.agent_native, PolicyMode::Warn);
    assert!(context.policy.exceptions.is_empty());
    assert!(context.skill_manifests.is_empty());
}

/// Canonical pretty-printed schema-v1 JSON used by the wire-contract test.
pub(super) const AGENT_CONTEXT_WIRE_CONTRACT_JSON: &str =
    include_str!("fixtures/agent_context_wire_contract.json").trim_ascii_end();

/// Returns a required object field for schema assertions, failing with the
/// field name when the fixture is malformed.
pub(super) fn field<'a>(value: &'a Value, name: &str) -> &'a Value {
    let Some(field) = value.get(name) else {
        panic!("JSON object should contain `{name}`");
    };
    field
}

/// Returns the first array item required by a schema assertion.
pub(super) fn first_array_item(value: &Value) -> &Value {
    let Some(item) = value.as_array().and_then(|items| items.first()) else {
        panic!("JSON value should be a non-empty array");
    };
    item
}

/// Builds a fully populated context used by serialization and round-trip
/// contract tests.
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
            exceptions: vec![PolicyException {
                kind: "flag".to_owned(),
                name: "--json".to_owned(),
                command_path: None,
            }],
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
