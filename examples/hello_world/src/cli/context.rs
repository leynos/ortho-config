//! Illustrative agent-context contract for the `hello_world` example.
//!
//! This module demonstrates the downstream `context --json` naming convention.
//! It is hand-authored example metadata, not an auto-generated command tree.

use clap::Parser;
use ortho_config::{
    AgentCommand, AgentContext, AgentExample, AgentInput, InteractionMode, MutationEffect,
    OrthoConfig, serde_json, serialize_agent_context,
};
use serde::{Deserialize, Serialize};

const CONTEXT_COMMAND: &str = "context";
const CONTEXT_JSON_FLAG: &str = "json";

/// Arguments for the `context` introspection command.
#[derive(Debug, Clone, Default, PartialEq, Eq, Parser, Deserialize, Serialize, OrthoConfig)]
#[ortho_config(prefix = "HELLO_WORLD_CONTEXT")]
pub struct ContextCommand {
    /// Emit the agent context as compact JSON.
    #[arg(long = CONTEXT_JSON_FLAG)]
    #[serde(default)]
    pub json: bool,
}

/// Builds the illustrative `hello_world` agent-context payload.
///
/// # Examples
///
/// ```
/// let context = hello_world::cli::context::hello_world_agent_context();
///
/// assert_eq!(context.kind, "hello_world.agent_context");
/// assert_eq!(context.package, "hello_world");
/// ```
#[must_use]
pub fn hello_world_agent_context() -> AgentContext {
    let mut context = AgentContext::new("hello_world");
    context.commands = vec![greet_command_context(), take_leave_command_context()];
    context
}

/// Renders the illustrative `hello_world` agent context as compact JSON.
///
/// # Errors
///
/// Returns a [`serde_json::Error`] if the context cannot be serialized.
///
/// # Examples
///
/// ```
/// let json = hello_world::cli::context::render_agent_context_json()?;
/// let payload: ortho_config::serde_json::Value =
///     ortho_config::serde_json::from_str(&json)?;
///
/// assert_eq!(
///     payload.get("kind").and_then(ortho_config::serde_json::Value::as_str),
///     Some("hello_world.agent_context")
/// );
/// # Ok::<(), ortho_config::serde_json::Error>(())
/// ```
pub fn render_agent_context_json() -> Result<String, serde_json::Error> {
    serialize_agent_context(&hello_world_agent_context())
}

/// Returns the human-readable pointer shown by bare `context`.
///
/// # Examples
///
/// ```
/// let pointer = hello_world::cli::context::context_json_pointer();
///
/// assert!(pointer.contains("hello-world context --json"));
/// ```
#[must_use]
pub fn context_json_pointer() -> String {
    format!("Run `hello-world {CONTEXT_COMMAND} --{CONTEXT_JSON_FLAG}` for JSON agent context.\n")
}

fn greet_command_context() -> AgentCommand {
    AgentCommand {
        path: vec!["hello-world".to_owned(), "greet".to_owned()],
        summary: Some("Print a greeting using the configured style.".to_owned()),
        canonical_verb: Some("get".to_owned()),
        inputs: vec![
            string_input("preamble", "preamble", None, Vec::new()),
            string_input("punctuation", "punctuation", Some("!"), Vec::new()),
        ],
        output_modes: vec!["text".to_owned()],
        interaction_mode: InteractionMode::NonInteractive,
        mutation_effect: MutationEffect::ReadOnly,
        async_submission: None,
        delivery_route: None,
        pagination: None,
        examples: vec![AgentExample {
            command: "hello-world greet --preamble \"Good morning\"".to_owned(),
            output_mode: Some("text".to_owned()),
        }],
    }
}

fn take_leave_command_context() -> AgentCommand {
    AgentCommand {
        path: vec!["hello-world".to_owned(), "take-leave".to_owned()],
        summary: Some(
            "Say goodbye while describing how the farewell will be delivered.".to_owned(),
        ),
        canonical_verb: Some("get".to_owned()),
        inputs: vec![
            string_input("parting", "parting", Some("Take care"), Vec::new()),
            string_input("greeting_preamble", "preamble", None, Vec::new()),
            string_input("greeting_punctuation", "punctuation", None, Vec::new()),
            string_input("channel", "channel", None, ["message", "call", "email"]),
            AgentInput {
                name: "remind_in".to_owned(),
                long: Some("remind-in".to_owned()),
                value_type: Some("u16".to_owned()),
                required: false,
                default: None,
                enum_values: Vec::new(),
            },
            string_input("gift", "gift", None, Vec::new()),
            AgentInput {
                name: "wave".to_owned(),
                long: Some("wave".to_owned()),
                value_type: Some("bool".to_owned()),
                required: false,
                default: Some("false".to_owned()),
                enum_values: Vec::new(),
            },
        ],
        output_modes: vec!["text".to_owned()],
        interaction_mode: InteractionMode::NonInteractive,
        mutation_effect: MutationEffect::ReadOnly,
        async_submission: None,
        delivery_route: None,
        pagination: None,
        examples: vec![AgentExample {
            command: "hello-world take-leave --channel email".to_owned(),
            output_mode: Some("text".to_owned()),
        }],
    }
}

fn string_input(
    name: &str,
    long: &str,
    default: Option<&str>,
    enum_values: impl IntoIterator<Item = &'static str>,
) -> AgentInput {
    AgentInput {
        name: name.to_owned(),
        long: Some(long.to_owned()),
        value_type: Some("string".to_owned()),
        required: false,
        default: default.map(str::to_owned),
        enum_values: enum_values.into_iter().map(str::to_owned).collect(),
    }
}
