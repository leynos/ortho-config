//! Property tests for the documentation IR to agent-context transformer.

use proptest::collection::btree_set;
use proptest::prelude::*;
use std::collections::BTreeSet;

use super::bridge_ir_to_agent_context;
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
}

fn metadata_tree() -> impl Strategy<Value = DocMetadata> {
    (
        command_name("root"),
        prop::option::of(command_name("bin")),
        btree_set(command_name("child"), 0..8),
        btree_set(command_name("grandchild"), 0..5),
    )
        .prop_map(|(root, bin_name, child_names, grandchild_names)| {
            let grandchildren: Vec<_> = grandchild_names
                .into_iter()
                .map(|name| doc(&name, None, Vec::new()))
                .collect();
            let children = child_names
                .into_iter()
                .map(|name| doc(&name, None, grandchildren.clone()))
                .collect();
            doc(&root, bin_name.as_deref(), children)
        })
}

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
        ir_version: "1.1".to_owned(),
        app_name: app_name.to_owned(),
        bin_name: bin_name.map(str::to_owned),
        about_id: format!("{app_name}.about"),
        synopsis_id: None,
        sections: sections(),
        fields: Vec::new(),
        subcommands,
        windows: None,
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
