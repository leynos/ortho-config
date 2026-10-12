//! Bridge tests for declared and undeclared behaviour metadata.
//!
//! The behaviour block is the only part of the agent-context bridge that maps
//! IR enumerations rather than copying strings, so every accepted variant gets
//! an explicit case. A dropped or mis-wired `match` arm must fail here instead
//! of silently reporting `unknown` to a downstream consumer.

use ortho_config::{InteractionMode, MutationEffect};
use rstest::rstest;

use super::support::*;
use crate::agent_context::bridge_ir_to_agent_context;
use crate::schema::{InteractionKind, MutationKind};

/// Carries one declared-behaviour scenario for
/// [`transform_maps_declared_behaviour`].
struct DeclaredBehaviourCase {
    app_name: &'static str,
    about_id: &'static str,
    interaction: InteractionKind,
    mutation: MutationKind,
    declared_bypass: Option<&'static str>,
    declared_dry_run: Option<&'static str>,
    expected_interaction: InteractionMode,
    expected_mutation: MutationEffect,
    expected_bypass: Option<&'static str>,
    expected_dry_run: Option<&'static str>,
}

/// Each declared behaviour kind reaches its matching agent-context field
/// verbatim, with no normalisation between the IR and the wire.
///
/// Every [`MutationKind`] the IR accepts has a row, so a `map_mutation` arm
/// that returned `None` or the wrong effect would fail rather than fall
/// through to the undeclared default.
#[rstest]
#[case::interactive_delete(DeclaredBehaviourCase {
    app_name: "purge",
    about_id: "cmd.purge",
    interaction: InteractionKind::Interactive,
    mutation: MutationKind::Delete,
    declared_bypass: Some("--force"),
    declared_dry_run: None,
    expected_interaction: InteractionMode::Interactive,
    expected_mutation: MutationEffect::Delete,
    expected_bypass: Some("--force"),
    expected_dry_run: None,
})]
#[case::non_interactive_read_only(DeclaredBehaviourCase {
    app_name: "inspect",
    about_id: "cmd.inspect",
    interaction: InteractionKind::NonInteractive,
    mutation: MutationKind::ReadOnly,
    declared_bypass: None,
    declared_dry_run: Some("--dry-run"),
    expected_interaction: InteractionMode::NonInteractive,
    expected_mutation: MutationEffect::ReadOnly,
    expected_bypass: None,
    expected_dry_run: Some("--dry-run"),
})]
#[case::non_interactive_write(DeclaredBehaviourCase {
    app_name: "apply",
    about_id: "cmd.apply",
    interaction: InteractionKind::NonInteractive,
    mutation: MutationKind::Write,
    declared_bypass: None,
    declared_dry_run: Some("--dry-run"),
    expected_interaction: InteractionMode::NonInteractive,
    expected_mutation: MutationEffect::Write,
    expected_bypass: None,
    expected_dry_run: Some("--dry-run"),
})]
#[case::non_interactive_submit(DeclaredBehaviourCase {
    app_name: "submit",
    about_id: "cmd.submit",
    interaction: InteractionKind::NonInteractive,
    mutation: MutationKind::Submit,
    declared_bypass: None,
    declared_dry_run: None,
    expected_interaction: InteractionMode::NonInteractive,
    expected_mutation: MutationEffect::Submit,
    expected_bypass: None,
    expected_dry_run: None,
})]
fn transform_maps_declared_behaviour(#[case] case: DeclaredBehaviourCase) {
    let metadata = doc(DocSpec {
        app_name: case.app_name,
        bin_name: None,
        about_id: case.about_id,
        fields: Vec::new(),
        subcommands: Vec::new(),
        behaviour: Some(declared_behaviour(
            case.interaction,
            case.mutation,
            case.declared_bypass,
            case.declared_dry_run,
        )),
    });

    let context = bridge_ir_to_agent_context(&metadata, "demo_pkg", None);
    let command = context
        .commands
        .first()
        .expect("command should be generated");

    assert_eq!(command.interaction_mode, case.expected_interaction);
    assert_eq!(command.mutation_effect, case.expected_mutation);
    assert_eq!(command.bypass_flag.as_deref(), case.expected_bypass);
    assert_eq!(command.dry_run_flag.as_deref(), case.expected_dry_run);
}

/// An IR command with no behaviour block reports `Unknown`, never a guess:
/// ADR-010 forbids inferring behaviour from a command's name.
#[test]
fn transform_keeps_behaviour_fields_unknown_when_undeclared() {
    let metadata = doc(DocSpec::child("version", "cmd.version"));

    let context = bridge_ir_to_agent_context(&metadata, "demo_pkg", None);
    let command = context
        .commands
        .first()
        .expect("version command should be generated");

    assert_eq!(command.interaction_mode, InteractionMode::Unknown);
    assert_eq!(command.mutation_effect, MutationEffect::Unknown);
    assert_eq!(command.bypass_flag, None);
    assert_eq!(command.dry_run_flag, None);
}

/// Nested subcommands carry their own behaviour, read per command rather
/// than inherited from the parent.
#[test]
fn transform_maps_behaviour_on_nested_subcommands() {
    let metadata = metadata_with_subcommands(vec![
        doc(DocSpec {
            app_name: "prune",
            bin_name: None,
            about_id: "cmd.prune",
            fields: Vec::new(),
            subcommands: Vec::new(),
            behaviour: Some(declared_behaviour(
                InteractionKind::Interactive,
                MutationKind::Delete,
                None,
                None,
            )),
        }),
        doc(DocSpec::child("list", "cmd.list")),
    ]);

    let context = bridge_ir_to_agent_context(&metadata, "demo_pkg", None);
    let prune = context
        .commands
        .iter()
        .find(|command| command.path == ["demo-bin", "prune"])
        .expect("prune command should be generated");

    assert_eq!(prune.interaction_mode, InteractionMode::Interactive);
    assert_eq!(prune.mutation_effect, MutationEffect::Delete);
    assert_eq!(prune.bypass_flag, None);

    let list = context
        .commands
        .iter()
        .find(|command| command.path == ["demo-bin", "list"])
        .expect("list command should be generated");
    assert_eq!(list.interaction_mode, InteractionMode::Unknown);
    assert_eq!(list.mutation_effect, MutationEffect::Unknown);
}
