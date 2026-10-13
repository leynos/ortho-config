//! Property tests for the documentation IR to agent-context transformer.
//!
//! These are sampled checks over generated trees, not proofs. Two of them are
//! deliberately shaped so their generator cannot supply the property for free:
//!
//! - sibling names are generated in arbitrary order, so the path-ordering
//!   property exercises the bridge's own sort instead of observing an order the
//!   generator already supplied;
//! - command preservation is checked against a path set derived here from the
//!   input tree, so a dropped or invented command fails rather than satisfying
//!   a weaker "everything present is ordered" assertion.
//!
//! The remaining properties state their own limits. The input-name ordering
//! property is adjacency-based and only the root receives generated fields, so
//! an empty or singleton input list satisfies it. The normalization property
//! generates ordinary strings only; character literals, raw strings, escaped
//! quotes, and lifetimes are covered by the example table in the sibling
//! `tests` module rather than generated here.

use proptest::prelude::*;
use std::collections::BTreeSet;

use super::{bridge_ir_to_agent_context, normalize_default_display};
use crate::schema::{CliMetadata, DocMetadata, FieldMetadata, HeadingIds, SectionsMetadata};

proptest! {
    #[test]
    fn generated_trees_have_unique_command_paths(tree in metadata_tree()) {
        let context = bridge_ir_to_agent_context(&tree, "demo_pkg", None);
        let mut seen = BTreeSet::new();

        for command in context.commands {
            prop_assert!(seen.insert(command.path));
        }
    }

    /// No command the tree describes is dropped, and none is invented.
    ///
    /// The expected paths are derived in this test from the input tree, so the
    /// property fails if the bridge loses a command. That matters because the
    /// uniqueness and ordering properties are both satisfied by an empty
    /// command list.
    #[test]
    fn every_generated_command_survives_the_bridge(tree in metadata_tree()) {
        let expected = expected_paths(&tree);
        let context = bridge_ir_to_agent_context(&tree, "demo_pkg", None);
        let actual = context
            .commands
            .iter()
            .map(|command| command.path.clone())
            .collect::<BTreeSet<_>>();

        prop_assert_eq!(actual.len(), context.commands.len());
        prop_assert_eq!(&actual, &expected);
    }

    /// Commands are ordered by path even though siblings arrive unsorted.
    #[test]
    fn commands_are_sorted_by_path(tree in metadata_tree()) {
        let context = bridge_ir_to_agent_context(&tree, "demo_pkg", None);

        for (left, right) in context.commands.iter().zip(context.commands.iter().skip(1)) {
            prop_assert!(left.path <= right.path);
        }
    }

    #[test]
    fn command_inputs_are_sorted_by_name(tree in metadata_tree_with_fields()) {
        let context = bridge_ir_to_agent_context(&tree, "demo_pkg", None);

        for command in context.commands {
            for (left, right) in command.inputs.iter().zip(command.inputs.iter().skip(1)) {
                prop_assert!(left.name <= right.name);
            }
        }
    }

    #[test]
    fn transformed_trees_serialize_deterministically(tree in metadata_tree_with_fields()) {
        let first = bridge_ir_to_agent_context(&tree, "demo_pkg", None);
        let second = bridge_ir_to_agent_context(&tree, "demo_pkg", None);

        let first_json = serde_json::to_string_pretty(&first)
            .expect("first transform should serialize");
        let second_json = serde_json::to_string_pretty(&second)
            .expect("second transform should serialize");

        prop_assert_eq!(first_json, second_json);
    }

    #[test]
    fn hidden_fields_are_omitted_from_inputs(hidden_name in field_name("hidden")) {
        let mut tree = doc("root", Some("demo"), Vec::new());
        tree.fields = vec![field(&hidden_name, true)];

        let context = bridge_ir_to_agent_context(&tree, "demo_pkg", None);

        for command in context.commands {
            prop_assert!(
                command.inputs.iter().all(|input| input.name != hidden_name)
            );
        }
    }

    #[test]
    fn default_normalization_preserves_generated_literal_contents(
        left in literal_fragment(),
        right in literal_fragment(),
        inner_before in separator_spacing(),
        inner_after in separator_spacing(),
        outer_before in separator_spacing(),
        outer_after in separator_spacing(),
    ) {
        let literal = format!("\"{left}{inner_before}::{inner_after}{right}\"");
        let display = format!("String{outer_before}::{outer_after}from({literal})");

        prop_assert_eq!(
            normalize_default_display(&display),
            format!("String::from({literal})")
        );
    }
}

/// Derives the command paths a tree must produce, from the tree alone.
///
/// The derivation deliberately does not reuse the bridge's traversal: it works
/// through an explicit worklist and applies the documented path rule (the root
/// prefers `bin_name`, every child appends `app_name`) to each node it pops.
/// The production code is never consulted, so a change in how the bridge walks
/// or names commands shows up as a mismatch here rather than being mirrored.
fn expected_paths(tree: &DocMetadata) -> BTreeSet<Vec<String>> {
    let mut pending = vec![(tree, Vec::<String>::new())];
    let mut paths = BTreeSet::new();

    while let Some((meta, parent)) = pending.pop() {
        let path = if parent.is_empty() {
            vec![
                meta.bin_name
                    .clone()
                    .unwrap_or_else(|| meta.app_name.clone()),
            ]
        } else {
            let mut child_path = parent;
            child_path.push(meta.app_name.clone());
            child_path
        };
        for subcommand in &meta.subcommands {
            pending.push((subcommand, path.clone()));
        }
        paths.insert(path);
    }

    paths
}

fn literal_fragment() -> impl Strategy<Value = String> {
    "[A-Za-z0-9 _:-]{0,32}"
}

fn separator_spacing() -> impl Strategy<Value = &'static str> {
    prop_oneof![Just(""), Just(" "), Just("  "), Just("\t")]
}

/// Builds a valid tree whose siblings arrive in arbitrary order.
///
/// `btree_set` would supply already-sorted sibling names, and the bridge emits
/// the root ahead of each sorted subtree — so a sorted generator would make the
/// path-ordering property hold even with the bridge's own sort removed. Common
/// proptest sets are deduplicated here instead, leaving the arrival order
/// unsorted, which is what puts the sort under test.
///
/// Sizes are `0..8` children (0–7), `0..5` grandchildren (0–4 per child), and
/// every child receives a clone of the same generated grandchild list.
fn metadata_tree() -> impl Strategy<Value = DocMetadata> {
    (
        command_name("root"),
        prop::option::of(command_name("bin")),
        unordered_names("child", 8),
        unordered_names("grandchild", 5),
    )
        .prop_map(|(root, bin_name, child_names, grandchild_names)| {
            let grandchildren: Vec<_> = grandchild_names
                .iter()
                .map(|name| doc(name, None, Vec::new()))
                .collect();
            let children = child_names
                .iter()
                .map(|name| doc(name, None, grandchildren.clone()))
                .collect();
            doc(&root, bin_name.as_deref(), children)
        })
}

/// Generates up to `max - 1` distinct names in an arbitrary, unsorted order.
fn unordered_names(prefix: &'static str, max: usize) -> impl Strategy<Value = Vec<String>> {
    prop::collection::vec(command_name(prefix), 0..max).prop_map(|names| {
        let mut seen = BTreeSet::new();
        names
            .into_iter()
            .filter(|name| seen.insert(name.clone()))
            .collect()
    })
}

/// Attaches 1–7 generated fields to the root, and to the root only.
///
/// The input-name ordering property therefore observes a non-empty, unsorted
/// field vector on the root while every other command keeps an empty one.
fn metadata_tree_with_fields() -> impl Strategy<Value = DocMetadata> {
    (
        metadata_tree(),
        prop::collection::vec(field_name("field"), 1..8),
    )
        .prop_map(|(mut tree, field_names)| {
            tree.fields = field_names
                .into_iter()
                .map(|name| field(&name, false))
                .collect();
            tree
        })
}

fn command_name(prefix: &'static str) -> impl Strategy<Value = String> {
    (0_u16..=4096).prop_map(move |suffix| format!("{prefix}-{suffix}"))
}

fn field_name(prefix: &'static str) -> impl Strategy<Value = String> {
    (0_u16..=4096).prop_map(move |suffix| format!("{prefix}_{suffix}"))
}

fn doc(app_name: &str, bin_name: Option<&str>, subcommands: Vec<DocMetadata>) -> DocMetadata {
    DocMetadata {
        ir_version: crate::schema::ORTHO_DOCS_IR_VERSION.to_owned(),
        app_name: app_name.to_owned(),
        bin_name: bin_name.map(str::to_owned),
        about_id: format!("{app_name}.about"),
        synopsis_id: None,
        sections: sections(),
        fields: Vec::new(),
        subcommands,
        windows: None,
        behaviour: None,
        profiles: None,
    }
}

fn field(name: &str, hide_in_help: bool) -> FieldMetadata {
    FieldMetadata {
        name: name.to_owned(),
        help_id: format!("{name}.help"),
        long_help_id: None,
        value: None,
        default: None,
        required: false,
        deprecated: None,
        cli: Some(CliMetadata {
            long: Some(name.to_owned()),
            short: None,
            value_name: Some("VALUE".to_owned()),
            multiple: false,
            takes_value: true,
            possible_values: Vec::new(),
            hide_in_help,
        }),
        env: None,
        file: None,
        examples: Vec::new(),
        links: Vec::new(),
        notes: Vec::new(),
    }
}

fn sections() -> SectionsMetadata {
    SectionsMetadata {
        headings_ids: HeadingIds {
            name: "heading.name".to_owned(),
            synopsis: "heading.synopsis".to_owned(),
            description: "heading.description".to_owned(),
            options: "heading.options".to_owned(),
            environment: "heading.environment".to_owned(),
            files: "heading.files".to_owned(),
            precedence: "heading.precedence".to_owned(),
            exit_status: "heading.exit-status".to_owned(),
            examples: "heading.examples".to_owned(),
            see_also: "heading.see-also".to_owned(),
            commands: None,
        },
        discovery: None,
        precedence: None,
        examples: Vec::new(),
        links: Vec::new(),
        notes: Vec::new(),
    }
}
