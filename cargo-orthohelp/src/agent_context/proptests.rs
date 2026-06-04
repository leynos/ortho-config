//! Property tests for the documentation IR to agent-context transformer.

use proptest::collection::btree_set;
use proptest::prelude::*;
use std::collections::BTreeSet;

use super::bridge_ir_to_agent_context;
use crate::schema::{DocMetadata, HeadingIds, SectionsMetadata};

proptest! {
    #[test]
    fn generated_trees_have_unique_command_paths(tree in metadata_tree()) {
        let context = bridge_ir_to_agent_context(&tree, "demo_pkg", None);
        let mut seen = BTreeSet::new();

        for command in context.commands {
            prop_assert!(seen.insert(command.path));
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

fn command_name(prefix: &'static str) -> impl Strategy<Value = String> {
    (0_u16..=4096).prop_map(move |suffix| format!("{prefix}-{suffix}"))
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
