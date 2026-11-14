//! Contract tests for the agent-context to effective-surface adapter.
//!
//! The adapter is the only place a real command tree becomes a validation
//! surface, so these tests pin what it must not invent: a root command's
//! inputs never reach a child, and only the options the consumer lists as
//! global are classified global. Each test starts from a real `AgentContext`
//! rather than a hand-built surface, so a regression in the adapter is caught
//! here rather than only in a consumer.

use ortho_config::{AgentCommand, AgentContext, AgentInput, RendererContract};
use pretty_assertions::assert_eq;

use super::input::{CommandContract, EffectiveCommandSurface};
use super::report::codes;
use super::support::{
    codes_of, example, global, input, manifest, option, option_use, policy, reference,
};
use super::surface::GlobalOption;
use super::validate;
use crate::policy::PolicyMode;

/// Builds one declared input that is available as a long option.
///
/// `AgentInput` has no `Default`, so every field is stated: the adapter reads
/// only `name`, `long`, and `value_type`, and the rest stay at their documented
/// empty values so a test changes exactly what it exercises.
fn agent_input(long: &str) -> AgentInput {
    AgentInput {
        name: long.to_owned(),
        long: Some(long.to_owned()),
        value_type: None,
        required: false,
        default: None,
        enum_values: Vec::new(),
    }
}

/// Builds one command declaring the supplied long options in order.
///
/// The defaults mirror the schema-v1 legacy compatibility values, which the
/// adapter never reads. Only `path`, `canonical_verb`, and `inputs` reach the
/// surface.
fn agent_command(segments: &[&str], longs: &[&str]) -> AgentCommand {
    AgentCommand {
        path: segments
            .iter()
            .map(|segment| (*segment).to_owned())
            .collect(),
        summary: None,
        canonical_verb: None,
        inputs: longs.iter().map(|long| agent_input(long)).collect(),
        output_modes: Vec::new(),
        interaction_mode: ortho_config::InteractionMode::default(),
        mutation_effect: ortho_config::MutationEffect::default(),
        bypass_flag: None,
        dry_run_flag: None,
        async_submission: None,
        delivery_route: None,
        pagination: None,
        examples: Vec::new(),
        renderer: RendererContract::default(),
    }
}

/// Builds a context exposing `tool` and `tool build`.
///
/// The root declares a root-only option; the child declares `json`. Neither
/// declares the global `color`, so the adapter must add it to both.
fn context() -> AgentContext {
    AgentContext {
        commands: vec![
            agent_command(&["tool"], &["root-only"]),
            agent_command(&["tool", "build"], &["json"]),
        ],
        ..AgentContext::new("tool")
    }
}

/// Returns one command from a surface, failing the caller when it is absent.
fn command_at(surface: &EffectiveCommandSurface, index: usize) -> &CommandContract {
    surface
        .commands
        .get(index)
        .expect("the adapter builds one contract per tree command")
}

/// Builds the surface every test in this module validates against.
fn adapter_surface() -> EffectiveCommandSurface {
    EffectiveCommandSurface::from_agent_context(&context(), &[GlobalOption::new("color")])
}

#[test]
fn a_root_only_option_never_reaches_a_child() {
    let surface = adapter_surface();

    let child = command_at(&surface, 1);
    assert_eq!(
        child.options,
        vec![option("json"), global("color")],
        "a child lists its own inputs first, then the shared globals"
    );
    assert!(
        !child
            .options
            .iter()
            .any(|option| option.long == "root-only"),
        "the root's inputs must not be unioned into a child"
    );

    let claims = input(
        vec![manifest(
            "build-workflow",
            vec![reference(&["tool", "build"], &["root-only"])],
        )],
        Vec::new(),
    );

    let report = validate(&claims, &surface, &policy(PolicyMode::Off))
        .expect("the adapter produces an unambiguous surface");

    assert_eq!(codes_of(&report), vec![codes::UNKNOWN_FLAG]);
}

#[test]
fn the_global_classification_reaches_the_policy_gate() {
    // `color` was added to the child by the adapter rather than declared by
    // the tree, so this finding only appears if the classification survived
    // the trip. An unclassified `color` would be judged as a domain option and
    // the glossary value check would stay silent.
    let claims = input(
        vec![manifest(
            "build-workflow",
            vec![reference(&["tool", "build"], &["color"])],
        )],
        vec![example(
            "build-workflow",
            1,
            &["tool", "build"],
            vec![option_use("color", Some("sometimes"), 1)],
        )],
    );

    let report = validate(&claims, &adapter_surface(), &policy(PolicyMode::Deny))
        .expect("the adapter produces an unambiguous surface");

    assert_eq!(codes_of(&report), vec![codes::POLICY_GLOBAL_OPTION_VALUE]);
}

#[test]
fn a_declared_global_is_listed_once_with_its_own_value_type() {
    let context = AgentContext {
        commands: vec![AgentCommand {
            inputs: vec![AgentInput {
                value_type: Some("string".to_owned()),
                ..agent_input("color")
            }],
            ..agent_command(&["tool"], &[])
        }],
        ..AgentContext::new("tool")
    };
    let globals = [GlobalOption::new("color")];

    let surface = EffectiveCommandSurface::from_agent_context(&context, &globals);

    let colours: Vec<_> = command_at(&surface, 0)
        .options
        .iter()
        .filter(|option| option.long == "color")
        .collect();
    assert_eq!(colours.len(), 1, "one CLI option must not become two");
    let colour = colours.first().expect("the length was asserted to be one");
    assert_eq!(colour.value_type.as_deref(), Some("string"));
    assert!(colour.is_global);
}

#[test]
fn an_input_without_a_long_name_contributes_no_option() {
    let context = AgentContext {
        commands: vec![AgentCommand {
            inputs: vec![AgentInput {
                long: None,
                ..agent_input("target")
            }],
            ..agent_command(&["tool"], &[])
        }],
        ..AgentContext::new("tool")
    };

    let surface = EffectiveCommandSurface::from_agent_context(&context, &[]);

    assert_eq!(
        command_at(&surface, 0).options,
        Vec::new(),
        "a positional input is not an option"
    );
}
