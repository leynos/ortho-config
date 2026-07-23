//! Nested command fixtures and assertions for agent-context bridge tests.

use ortho_config::AgentCommand;

use super::support::{DocSpec, FieldSpec, cli_field, doc};
use crate::schema::{DocMetadata, FieldMetadata, ValueType};

/// Builds the nested documentation tree used to verify flattened command and
/// input ordering.
pub(super) fn nested_metadata() -> DocMetadata {
    doc(DocSpec {
        app_name: "nested-app",
        bin_name: Some("fixture"),
        about_id: "root.about",
        fields: vec![string_cli_field("global", "global", None, true)],
        subcommands: vec![version_metadata(), admin_metadata(), greet_metadata()],
    })
}

fn version_metadata() -> DocMetadata {
    leaf_metadata("version", "cmd.version", Vec::new())
}

fn admin_metadata() -> DocMetadata {
    doc(DocSpec {
        app_name: "admin",
        bin_name: None,
        about_id: "cmd.admin",
        fields: vec![string_cli_field(
            "scope",
            "scope",
            Some("String :: from(\"local\")"),
            false,
        )],
        subcommands: vec![
            leaf_metadata(
                "grant-access",
                "cmd.admin.grant-access",
                vec![string_cli_field("principal", "principal", None, true)],
            ),
            leaf_metadata(
                "audit",
                "cmd.admin.audit",
                vec![bool_cli_field("dry_run", "dry-run")],
            ),
        ],
    })
}

fn greet_metadata() -> DocMetadata {
    leaf_metadata(
        "greet",
        "cmd.greet",
        vec![
            string_cli_field(
                "recipient",
                "recipient",
                Some("String :: from(\"World\")"),
                false,
            ),
            bool_cli_field("excited", "excited"),
        ],
    )
}

fn leaf_metadata(
    app_name: &'static str,
    about_id: &'static str,
    fields: Vec<FieldMetadata>,
) -> DocMetadata {
    doc(DocSpec {
        app_name,
        bin_name: None,
        about_id,
        fields,
        subcommands: Vec::new(),
    })
}

type NestedInputSummary = (String, Option<String>, Option<String>);
type NestedCommandSummary = (Vec<String>, Vec<NestedInputSummary>, Option<String>);

/// Projects a generated command into the path, inputs, and canonical verb
/// compared by nested-command tests.
pub(super) fn nested_command_summary(command: &AgentCommand) -> NestedCommandSummary {
    (
        command.path.clone(),
        command
            .inputs
            .iter()
            .map(|input| {
                (
                    input.name.clone(),
                    input.long.clone(),
                    input.default.clone(),
                )
            })
            .collect(),
        command.canonical_verb.clone(),
    )
}

/// Returns the ordered command summaries expected from [`nested_metadata`].
pub(super) fn expected_nested_command_summaries() -> Vec<NestedCommandSummary> {
    vec![
        (
            vec!["fixture".to_owned()],
            vec![("global".to_owned(), Some("global".to_owned()), None)],
            None,
        ),
        (
            vec!["fixture".to_owned(), "admin".to_owned()],
            vec![(
                "scope".to_owned(),
                Some("scope".to_owned()),
                Some("String::from(\"local\")".to_owned()),
            )],
            None,
        ),
        (
            vec!["fixture".to_owned(), "admin".to_owned(), "audit".to_owned()],
            vec![("dry_run".to_owned(), Some("dry-run".to_owned()), None)],
            None,
        ),
        (
            vec![
                "fixture".to_owned(),
                "admin".to_owned(),
                "grant-access".to_owned(),
            ],
            vec![("principal".to_owned(), Some("principal".to_owned()), None)],
            None,
        ),
        (
            vec!["fixture".to_owned(), "greet".to_owned()],
            vec![
                ("excited".to_owned(), Some("excited".to_owned()), None),
                (
                    "recipient".to_owned(),
                    Some("recipient".to_owned()),
                    Some("String::from(\"World\")".to_owned()),
                ),
            ],
            None,
        ),
        (
            vec!["fixture".to_owned(), "version".to_owned()],
            Vec::new(),
            None,
        ),
    ]
}

fn string_cli_field(
    name: &'static str,
    long: &'static str,
    default: Option<&'static str>,
    required: bool,
) -> FieldMetadata {
    cli_field(FieldSpec {
        name,
        long: Some(long),
        short: None,
        takes_value: true,
        hide_in_help: false,
        value: Some(ValueType::String),
        default,
        required,
    })
}

fn bool_cli_field(name: &'static str, long: &'static str) -> FieldMetadata {
    cli_field(FieldSpec {
        name,
        long: Some(long),
        short: None,
        takes_value: false,
        hide_in_help: false,
        value: Some(ValueType::Bool),
        default: None,
        required: false,
    })
}
