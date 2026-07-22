//! Assertion helpers and expected JSON for agent-context schema tests.

use super::{field, first_array_item};
use crate::agent_context::{
    AGENT_CONTEXT_KIND_SUFFIX, AgentCommand, AgentContext, InteractionMode, MutationEffect,
    ORTHO_AGENT_CONTEXT_SCHEMA_VERSION, PolicyMode,
};
use crate::docs::ORTHO_DOCS_IR_VERSION;
use serde_json::Value;

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
    assert!(context.skill_manifests.is_empty());
}

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
    assert!(context.skill_manifests.is_empty());
}

pub(super) const AGENT_CONTEXT_WIRE_CONTRACT_JSON: &str = r#"{
  "schema_version": "1",
  "kind": "example-cli.agent_context",
  "package": "example-cli",
  "commands": [
    {
      "path": [
        "example-cli",
        "list"
      ],
      "summary": "List configured resources.",
      "canonical_verb": "list",
      "inputs": [
        {
          "name": "format",
          "long": "format",
          "value_type": "string",
          "required": false,
          "default": "json",
          "enum_values": [
            "json"
          ]
        }
      ],
      "output_modes": [
        "json"
      ],
      "interaction_mode": "non_interactive",
      "mutation_effect": "read_only",
      "async_submission": {
        "mode": "submit",
        "noun": "job"
      },
      "delivery_route": {
        "supported": true,
        "target": "file"
      },
      "pagination": {
        "limit_input": "limit",
        "cursor_input": "cursor"
      },
      "examples": [
        {
          "command": "example-cli list --format json",
          "output_mode": "json"
        }
      ]
    }
  ],
  "profiles": {
    "supported": false
  },
  "feedback": {
    "supported": false
  },
  "policy": {
    "agent_native": "warn"
  },
  "skill_manifests": [
    {
      "id": "example-list",
      "path": "skills/example-list.md",
      "manifest_schema_version": "v1",
      "commands": [
        {
          "path": [
            "example-cli",
            "list"
          ],
          "flags": [
            "format"
          ]
        }
      ]
    }
  ]
}"#;
