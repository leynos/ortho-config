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

use std::collections::BTreeMap;

use ortho_config::agent_context::{OutputContract, StdoutPurpose as AgentStdoutPurpose};
use proptest::prelude::*;
use proptest::strategy::ValueTree;
use serde_json::Value;

use super::bridge_ir_to_agent_context;
use crate::schema::{
    DocMetadata, ExitClass as IrExitClass, HeadingIds, OutputMetadata as IrOutput,
    SectionsMetadata, StdoutPurpose as IrStdoutPurpose,
};

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

/// Reports whether the derive would accept a class name.
///
/// This restates `validate_class_name` in `ortho_config_macros` rather than
/// calling it: that parser lives in a different crate and is exercised by its
/// own compile-fail fixtures. What matters here is that the generator's output
/// satisfies the same grammar, so the E1 property cannot drift onto input an
/// author could not write.
fn is_derivable_name(name: &str) -> bool {
    name.chars()
        .next()
        .is_some_and(|first| first.is_ascii_lowercase())
        && name
            .chars()
            .all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '_')
}

/// The generator only produces declarations the derive would accept.
///
/// E1's whole claim rests on sampling valid input, so the generator is checked
/// on its own terms: names in the accepted grammar, meanings that survive
/// `trim()`, and no repeated name in one declaration.
#[test]
fn generated_declarations_are_well_formed() {
    let mut runner = proptest::test_runner::TestRunner::deterministic();
    let strategy = any_classes();

    for _ in 0..64 {
        let classes = strategy
            .new_tree(&mut runner)
            .expect("the class strategy builds a value")
            .current();
        let mut seen: Vec<&str> = Vec::new();
        for class in &classes {
            assert!(
                is_derivable_name(&class.name),
                "generated name must match [a-z][a-z0-9_]*: {:?}",
                class.name
            );
            assert!(
                !class.meaning.trim().is_empty(),
                "generated meaning must survive trim: {:?}",
                class.meaning
            );
            assert!(
                !seen.contains(&class.name.as_str()),
                "a declaration must not repeat a class name: {:?}",
                class.name
            );
            seen.push(&class.name);
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
fn agent_token(purpose: AgentStdoutPurpose) -> &'static str {
    match purpose {
        AgentStdoutPurpose::Data => "data",
        AgentStdoutPurpose::Path => "path",
        AgentStdoutPurpose::Summary => "summary",
        AgentStdoutPurpose::None => "none",
        AgentStdoutPurpose::Unknown => "unknown",
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

/// Generates a tree of depth 0–4 with at most 31 commands.
///
/// The branching factor is 0–2 and each node consumes one generated
/// declaration spec, so four levels of full branching reach 31 commands while
/// every shallower or narrower shape is reachable too.
fn output_tree() -> impl Strategy<Value = DocMetadata> {
    (
        0_usize..=4,
        0_usize..=2,
        prop::collection::vec(any_output(), 0..=31),
    )
        .prop_map(|(depth, breadth, specs)| {
            let mut declared = specs.into_iter();
            let shape = NodeShape {
                remaining_depth: depth,
                breadth,
            };
            build_node(shape, &mut declared, "root", true)
        })
}

/// The level budget and branching factor shared by one generated tree.
#[derive(Clone, Copy)]
struct NodeShape {
    /// Levels still to build below the current node.
    remaining_depth: usize,
    /// Children each non-leaf node receives.
    breadth: usize,
}

/// Builds one node and, below it, a full `breadth`-ary subtree.
fn build_node(
    shape: NodeShape,
    specs: &mut impl Iterator<Item = Option<IrOutput>>,
    name: &str,
    is_root: bool,
) -> DocMetadata {
    let mut node = doc(name, is_root);
    node.output = specs.next().flatten();
    if shape.remaining_depth > 0 {
        node.subcommands = (0..shape.breadth)
            .map(|index| {
                build_node(
                    NodeShape {
                        remaining_depth: shape.remaining_depth - 1,
                        breadth: shape.breadth,
                    },
                    specs,
                    &format!("{name}-{index}"),
                    false,
                )
            })
            .collect();
    }
    node
}

/// Generates a declaration that covers every combination of members.
fn any_output() -> impl Strategy<Value = Option<IrOutput>> {
    prop::option::of((
        prop::option::of(any::<bool>()),
        prop::option::of(any_stdout()),
        prop::option::of(any::<bool>()),
        any_classes(),
    ))
    .prop_map(|declared| {
        declared.map(
            |(supports_json, stdout, stderr_diagnostics, exit_classes)| IrOutput {
                supports_json,
                stdout,
                stderr_diagnostics,
                exit_classes,
            },
        )
    })
}

/// Generates every accepted stdout purpose.
fn any_stdout() -> impl Strategy<Value = IrStdoutPurpose> {
    prop_oneof![
        Just(IrStdoutPurpose::Data),
        Just(IrStdoutPurpose::Path),
        Just(IrStdoutPurpose::Summary),
        Just(IrStdoutPurpose::None),
    ]
}

/// Generates an exit-class name in the derive's `[a-z][a-z0-9_]*` grammar.
///
/// The regex is the grammar, so the sample varies in length and may contain
/// digits and underscores rather than repeating a handful of literals.
fn any_name() -> impl Strategy<Value = String> {
    "[a-z][a-z0-9_]{0,9}"
}

/// Generates a meaning the derive accepts: any text that is not blank.
///
/// Leading and trailing whitespace, Unicode letters and numbers, and
/// punctuation are all reachable, but a non-whitespace letter or number is
/// always present, so `trim()` can never empty the result.
fn any_meaning() -> impl Strategy<Value = String> {
    "\\s{0,2}[\\p{L}\\p{N}][\\p{L}\\p{N}\\p{Zs}\\p{P}]{0,18}\\s{0,2}"
}

/// Generates a well-formed class: an accepted name and a nonblank meaning.
fn any_class() -> impl Strategy<Value = IrExitClass> {
    (any_name(), any_meaning()).prop_map(|(name, meaning)| IrExitClass { name, meaning })
}

/// Generates a declaration of up to three classes with distinct names.
///
/// E1 samples the declarations an author can actually write, and the derive
/// rejects a repeated name, so uniqueness is enforced here by keeping the first
/// occurrence of each name. The malformed shapes the parser rejects — repeated
/// names and blank meanings — are covered by explicit robustness tests in
/// [`super::output_tests`], where they cannot be mistaken for valid input.
fn any_classes() -> impl Strategy<Value = Vec<IrExitClass>> {
    prop::collection::vec(any_class(), 0..3).prop_map(|classes| {
        let mut unique: Vec<IrExitClass> = Vec::new();
        for class in classes {
            if unique.iter().all(|seen| seen.name != class.name) {
                unique.push(class);
            }
        }
        unique
    })
}

/// Builds a minimal metadata node with no optional metadata declared.
fn doc(app_name: &str, is_root: bool) -> DocMetadata {
    DocMetadata {
        ir_version: crate::schema::ORTHO_DOCS_IR_VERSION.to_owned(),
        app_name: app_name.to_owned(),
        bin_name: is_root.then(|| "demo-bin".to_owned()),
        about_id: format!("{app_name}.about"),
        synopsis_id: None,
        sections: sections(),
        fields: Vec::new(),
        subcommands: Vec::new(),
        windows: None,
        behaviour: None,
        renderer: None,
        output: None,
        profiles: None,
    }
}

/// Builds the standard headings required by every metadata node.
fn sections() -> SectionsMetadata {
    SectionsMetadata {
        headings_ids: HeadingIds {
            name: "ortho.headings.name".to_owned(),
            synopsis: "ortho.headings.synopsis".to_owned(),
            description: "ortho.headings.description".to_owned(),
            options: "ortho.headings.options".to_owned(),
            environment: "ortho.headings.environment".to_owned(),
            files: "ortho.headings.files".to_owned(),
            precedence: "ortho.headings.precedence".to_owned(),
            exit_status: "ortho.headings.exit_status".to_owned(),
            examples: "ortho.headings.examples".to_owned(),
            see_also: "ortho.headings.see_also".to_owned(),
            commands: None,
        },
        discovery: None,
        precedence: None,
        examples: Vec::new(),
        links: Vec::new(),
        notes: Vec::new(),
    }
}
