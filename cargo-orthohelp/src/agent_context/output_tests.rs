//! Bridge tests for declared structured-output metadata (roadmap 7.2.3).
//!
//! The bridge is the only place the IR's strict stdout enum meets the wire's
//! tolerant one, so every accepted variant gets its own case: a dropped or
//! swapped `match` arm must fail here rather than degrading to `unknown`.
//!
//! Two cases keep the others honest. An absent block must stay absent rather
//! than becoming `supports_json: false`, and a declaration on one command must
//! not leak to its children — the contract is per command, never inherited.

use ortho_config::agent_context::{
    ExitClass as AgentExitClass, OutputContract, StdoutPurpose as AgentStdoutPurpose,
};
use rstest::rstest;

use super::support::*;
use crate::agent_context::bridge_ir_to_agent_context;
use crate::schema::{
    ExitClass as IrExitClass, OutputMetadata as IrOutput, StdoutPurpose as IrStdoutPurpose,
};

/// Returns the output contract of the single command in a one-command tree.
fn bridge_single(output: Option<IrOutput>) -> Option<OutputContract> {
    let mut metadata = doc(DocSpec::root_without_fields());
    metadata.output = output;
    let context = bridge_ir_to_agent_context(&metadata, "demo_pkg", None).expect("bridge succeeds");
    context
        .commands
        .into_iter()
        .next()
        .expect("a root command is always emitted")
        .output
}

/// Builds one documented class.
fn class(name: &str, meaning: &str) -> IrExitClass {
    IrExitClass {
        name: name.to_owned(),
        meaning: meaning.to_owned(),
    }
}

/// An absent declaration stays absent: the bridge never invents a refusal.
#[test]
fn absent_declaration_yields_none() {
    assert_eq!(
        bridge_single(None),
        None,
        "an undeclared command must not acquire a default contract"
    );
}

/// An explicit empty declaration projects to an empty contract.
#[test]
fn empty_declaration_yields_empty_contract() {
    assert_eq!(
        bridge_single(Some(IrOutput::default())),
        Some(OutputContract::default()),
        "an empty block still proves the author declared one"
    );
}

/// Every declared member reaches the wire unchanged.
#[test]
fn fully_declared_block_maps_every_member() {
    let declared = IrOutput {
        supports_json: Some(true),
        stdout: Some(IrStdoutPurpose::Data),
        stderr_diagnostics: Some(false),
        exit_classes: vec![class("success", "The command completed")],
    };

    assert_eq!(
        bridge_single(Some(declared)),
        Some(OutputContract {
            supports_json: Some(true),
            stdout: Some(AgentStdoutPurpose::Data),
            stderr_diagnostics: Some(false),
            exit_classes: vec![AgentExitClass {
                name: "success".to_owned(),
                meaning: "The command completed".to_owned(),
            }],
        }),
        "every declared member should survive the bridge"
    );
}

/// Each IR stdout variant maps to its own wire variant.
#[rstest]
#[case::data(IrStdoutPurpose::Data, AgentStdoutPurpose::Data)]
#[case::path(IrStdoutPurpose::Path, AgentStdoutPurpose::Path)]
#[case::summary(IrStdoutPurpose::Summary, AgentStdoutPurpose::Summary)]
#[case::none(IrStdoutPurpose::None, AgentStdoutPurpose::None)]
fn stdout_purposes_map_explicitly(
    #[case] declared: IrStdoutPurpose,
    #[case] expected: AgentStdoutPurpose,
) {
    let output = bridge_single(Some(IrOutput {
        stdout: Some(declared),
        ..IrOutput::default()
    }));

    assert_eq!(
        output.and_then(|contract| contract.stdout),
        Some(expected),
        "{declared:?} should map to {expected:?}, not to a fallback"
    );
}

/// A declared refusal survives as `Some(false)`.
#[test]
fn declared_refusal_is_not_flattened_to_undeclared() {
    let output = bridge_single(Some(IrOutput {
        supports_json: Some(false),
        stderr_diagnostics: Some(false),
        ..IrOutput::default()
    }))
    .expect("the block was declared");

    assert_eq!(
        output.supports_json,
        Some(false),
        "a refusal must not be dropped on the way to the wire"
    );
    assert_ne!(
        output.supports_json, None,
        "refusal and silence are different declarations"
    );
}

/// Class order and text are preserved, including non-ASCII meanings.
#[test]
fn exit_classes_keep_author_order_and_text() {
    let output = bridge_single(Some(IrOutput {
        exit_classes: vec![
            class("usage", "The invocation was invalid"),
            class("success", "The command completed — nothing to do"),
        ],
        ..IrOutput::default()
    }))
    .expect("the block was declared");

    let names = output
        .exit_classes
        .iter()
        .map(|entry| entry.name.as_str())
        .collect::<Vec<_>>();
    assert_eq!(names, ["usage", "success"], "order should be preserved");
    assert_eq!(
        output
            .exit_classes
            .get(1)
            .map(|entry| entry.meaning.as_str()),
        Some("The command completed — nothing to do"),
        "meanings are transported verbatim, never localized"
    );
}

/// Shapes the derive rejects are still transported verbatim.
///
/// A repeated class name and a whitespace-only meaning are hard errors in the
/// derive, so a well-formed document never carries them. The bridge is not a
/// validator, though: it must not drop, merge, or rewrite what the IR holds,
/// because a lossy bridge would hide a malformed block instead of exposing it.
#[test]
fn rejected_class_shapes_are_transported_verbatim() {
    let output = bridge_single(Some(IrOutput {
        exit_classes: vec![
            class("usage", "The invocation was invalid"),
            class("usage", "The invocation was invalid"),
            class("success", "   "),
        ],
        ..IrOutput::default()
    }))
    .expect("the block was declared");

    let transported = output
        .exit_classes
        .iter()
        .map(|entry| (entry.name.as_str(), entry.meaning.as_str()))
        .collect::<Vec<_>>();
    assert_eq!(
        transported,
        [
            ("usage", "The invocation was invalid"),
            ("usage", "The invocation was invalid"),
            ("success", "   "),
        ],
        "the bridge must neither deduplicate nor rewrite a rejected shape"
    );
}

/// A localizer resolves text elsewhere but never touches output metadata.
#[test]
fn localizer_does_not_alter_output_metadata() {
    let mut metadata = doc(DocSpec::root_without_fields());
    metadata.output = Some(IrOutput {
        supports_json: Some(true),
        stdout: Some(IrStdoutPurpose::Summary),
        stderr_diagnostics: None,
        exit_classes: vec![class("success", "The command completed")],
    });
    let localizer = StaticLocalizer::new([("root.about", "Root command.")]);

    let context = bridge_ir_to_agent_context(&metadata, "demo_pkg", Some(&localizer))
        .expect("bridge succeeds");
    let output = context
        .commands
        .first()
        .and_then(|command| command.output.clone())
        .expect("the declared block should survive localization");

    assert_eq!(
        output
            .exit_classes
            .first()
            .map(|entry| entry.meaning.as_str()),
        Some("The command completed"),
        "a class meaning is an identifier's explanation, not a Fluent key"
    );
    assert_eq!(
        output.stdout,
        Some(AgentStdoutPurpose::Summary),
        "the declared stdout purpose must survive the bridge"
    );
}

/// A parent's declaration does not reach its child, and vice versa.
#[test]
fn declarations_are_per_command_not_inherited() {
    let mut child = doc(DocSpec::child("inspect", "cmd.inspect"));
    child.output = Some(IrOutput {
        supports_json: Some(true),
        ..IrOutput::default()
    });
    let mut root = metadata_with_subcommands(vec![child]);
    root.output = None;

    let context = bridge_ir_to_agent_context(&root, "demo_pkg", None).expect("bridge succeeds");
    let mut commands = context.commands.into_iter();

    assert_eq!(
        commands.next().and_then(|command| command.output),
        None,
        "the declaring child must not lend its block to the parent"
    );
    assert_eq!(
        commands
            .next()
            .and_then(|command| command.output)
            .and_then(|contract| contract.supports_json),
        Some(true),
        "the child's own declaration should survive"
    );
}
