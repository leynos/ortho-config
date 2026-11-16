//! Property tests for declared structured-output metadata (roadmap 7.2.3).
//!
//! These are sampled checks over generated trees, not proofs. The generator
//! bounds itself to depth 4 and at most 31 commands, so every tree is a
//! legitimate command hierarchy rather than an unbounded stress case.
//!
//! The preservation property derives its expectation from the input tree by
//! walking that tree here, rather than by calling the bridge's mapping again.
//! A bridge that dropped a declaration, moved one to a sibling, or reordered
//! classes therefore fails instead of satisfying a weaker presence check.

#[path = "output_proptests/generators.rs"]
mod generators;

use std::collections::BTreeMap;

use ortho_config::agent_context::{OutputContract, StdoutPurpose as AgentStdoutPurpose};
use proptest::prelude::*;
use serde_json::Value;

use super::bridge_ir_to_agent_context;
use crate::schema::{DocMetadata, OutputMetadata as IrOutput, StdoutPurpose as IrStdoutPurpose};
use generators::output_tree;

proptest! {
    /// Every declared block reaches exactly the command that declared it.
    #[test]
    fn declared_output_survives_the_bridge(tree in output_tree()) {
        let expected = expected_outputs(&tree);
        let context = bridge_ir_to_agent_context(&tree, "demo_pkg", None);
        // Walk every generated path, not only the ones that came back: a bridge
        // that dropped a command would otherwise never be asked about it.
        let mut generated = expected.keys().cloned().collect::<Vec<_>>();
        let mut emitted = context
            .commands
            .iter()
            .map(|command| command.path.clone())
            .collect::<Vec<_>>();
        generated.sort();
        emitted.sort();
        prop_assert_eq!(
            &emitted,
            &generated,
            "the projection must emit exactly the generated command paths"
        );

        for command in context.commands {
            let declared = expected.get(&command.path).copied().flatten();
            match (declared, command.output.as_ref()) {
                (None, None) => {}
                (None, Some(invented)) => prop_assert!(
                    false,
                    "command {:?} acquired an undeclared block: {:?}",
                    command.path,
                    invented
                ),
                (Some(_), None) => prop_assert!(
                    false,
                    "command {:?} lost its declaration",
                    command.path
                ),
                (Some(source), Some(projected)) => {
                    prop_assert_eq!(
                        projected.supports_json,
                        source.supports_json,
                        "JSON support changed for {:?}",
                        command.path
                    );
                    prop_assert_eq!(
                        projected.stderr_diagnostics,
                        source.stderr_diagnostics,
                        "diagnostic stderr changed for {:?}",
                        command.path
                    );
                    prop_assert_eq!(
                        projected.stdout.map(agent_token),
                        source.stdout.map(ir_token),
                        "stdout purpose changed for {:?}",
                        command.path
                    );
                    prop_assert_eq!(
                        projected.exit_classes.iter().map(|class| class.name.as_str()).collect::<Vec<_>>(),
                        source.exit_classes.iter().map(|class| class.name.as_str()).collect::<Vec<_>>(),
                    );
                    prop_assert_eq!(
                        projected.exit_classes.iter().map(|class| class.meaning.as_str()).collect::<Vec<_>>(),
                        source.exit_classes.iter().map(|class| class.meaning.as_str()).collect::<Vec<_>>(),
                    );
                }
            }
        }
    }

    /// A projection never writes `null`: an undeclared member is omitted.
    #[test]
    fn projected_output_omits_undeclared_members(tree in output_tree()) {
        let context = bridge_ir_to_agent_context(&tree, "demo_pkg", None);

        for command in context.commands {
            let Some(output) = command.output.as_ref() else {
                continue;
            };
            let encoded = serde_json::to_value(output).expect("output contract serializes");
            let members = encoded.as_object().expect("a contract is a JSON object");
            prop_assert!(
                members.values().all(|value| !value.is_null()),
                "no member may be written as null: {encoded}"
            );
            prop_assert_eq!(members.len(), output_member_count(output));
        }
    }

    /// Re-encoding a projected block preserves it, so the wire is stable.
    #[test]
    fn projected_output_round_trips_through_json(tree in output_tree()) {
        let context = bridge_ir_to_agent_context(&tree, "demo_pkg", None);

        for command in context.commands {
            let Some(output) = command.output.as_ref() else {
                continue;
            };
            let encoded: Value =
                serde_json::to_value(output).expect("output contract serializes");
            let decoded: OutputContract =
                serde_json::from_value(encoded).expect("output contract decodes");
            prop_assert_eq!(&decoded, output);
        }
    }
}
/// Counts the members a contract should serialize for its declared leaves.
///
/// An empty contract has no members at all, which is the `{}` case rather than
/// a malformed one.
fn output_member_count(output: &OutputContract) -> usize {
    let declared = [
        output.supports_json.is_some(),
        output.stdout.is_some(),
        output.stderr_diagnostics.is_some(),
        !output.exit_classes.is_empty(),
    ];
    declared.iter().filter(|present| **present).count()
}

/// Returns the wire token for a tolerant stdout purpose.
///
/// The enum is `#[non_exhaustive]`, so the wildcard arm keeps this helper
/// compiling when a future variant lands. A variant with no token of its own
/// reads back as the tolerant `unknown`, which is exactly what the agent-context
/// reader does with a token it does not recognize.
fn agent_token(purpose: AgentStdoutPurpose) -> &'static str {
    match purpose {
        AgentStdoutPurpose::Data => "data",
        AgentStdoutPurpose::Path => "path",
        AgentStdoutPurpose::Summary => "summary",
        AgentStdoutPurpose::None => "none",
        // `Unknown` and every future variant share the tolerant token. The
        // wildcard arm is what the `#[non_exhaustive]` attribute requires, and
        // it reads the same way the agent-context reader treats an
        // unrecognized token.
        _ => "unknown",
    }
}

/// Returns the wire token for a strict stdout purpose.
fn ir_token(purpose: IrStdoutPurpose) -> &'static str {
    match purpose {
        IrStdoutPurpose::Data => "data",
        IrStdoutPurpose::Path => "path",
        IrStdoutPurpose::Summary => "summary",
        IrStdoutPurpose::None => "none",
    }
}

/// Maps every command path in the tree to the block of the node declaring it.
fn expected_outputs(tree: &DocMetadata) -> BTreeMap<Vec<String>, Option<&IrOutput>> {
    let mut collected = BTreeMap::new();
    collect_outputs(tree, &[], &mut collected);
    collected
}

/// Walks the tree, recording each node's declaration under its command path.
fn collect_outputs<'a>(
    node: &'a DocMetadata,
    parent: &[String],
    out: &mut BTreeMap<Vec<String>, Option<&'a IrOutput>>,
) {
    let path = if parent.is_empty() {
        vec![node.bin_name.as_ref().unwrap_or(&node.app_name).to_owned()]
    } else {
        let mut path = parent.to_vec();
        path.push(node.app_name.clone());
        path
    };
    out.insert(path.clone(), node.output.as_ref());
    for child in &node.subcommands {
        collect_outputs(child, &path, out);
    }
}
