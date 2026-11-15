//! Generators for the structured-output properties (roadmap 7.2.3).
//!
//! The strategies and their own contract test sit beside the properties rather
//! than inside them, so neither file outgrows the module length limit.
//! Everything here produces declarations an author can write: names in the
//! derive's own grammar, meanings that survive `trim()`, and command trees of
//! a bounded depth and breadth.

use proptest::prelude::*;
use proptest::strategy::ValueTree;

use crate::schema::{
    DocMetadata, ExitClass as IrExitClass, HeadingIds, OutputMetadata as IrOutput,
    SectionsMetadata, StdoutPurpose as IrStdoutPurpose,
};

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
/// Generates a tree of depth 0–4 with at most 31 commands.
///
/// The branching factor is 0–2 and each node consumes one generated
/// declaration spec, so four levels of full branching reach 31 commands while
/// every shallower or narrower shape is reachable too.
pub(super) fn output_tree() -> impl Strategy<Value = DocMetadata> {
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
/// [`super::super::output_tests`], where they cannot be mistaken for valid
/// input.
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
