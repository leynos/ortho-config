//! Unit tests for the documentation IR to agent-context transformer.

use rstest::rstest;

use super::bridge_ir_to_agent_context;
use crate::schema::ValueType;

#[test]
fn transform_flattens_commands_with_summaries_and_canonical_verbs() {
    let metadata = metadata_with_subcommands(vec![
        doc(DocSpec::child("get", "cmd.get")),
        doc(DocSpec::child("jobs", "cmd.jobs")),
        doc(DocSpec::child("inspect", "cmd.inspect")),
    ]);
    let localizer = StaticLocalizer::new([
        ("root.about", "Root command."),
        ("cmd.get", "Get a resource."),
        ("cmd.jobs", "Work with jobs."),
        ("cmd.inspect", "Inspect state."),
    ]);

    let context = bridge_ir_to_agent_context(&metadata, "demo_pkg", Some(&localizer));
    let commands: Vec<_> = context.commands.iter().map(command_summary).collect();

    assert_eq!(context.kind, "demo_pkg.agent_context");
    assert_eq!(
        commands,
        [
            (
                vec!["demo-bin".to_owned()],
                None,
                Some("Root command.".to_owned()),
            ),
            (
                vec!["demo-bin".to_owned(), "get".to_owned()],
                Some("get".to_owned()),
                Some("Get a resource.".to_owned()),
            ),
            (
                vec!["demo-bin".to_owned(), "inspect".to_owned()],
                None,
                Some("Inspect state.".to_owned()),
            ),
            (
                vec!["demo-bin".to_owned(), "jobs".to_owned()],
                Some("jobs".to_owned()),
                Some("Work with jobs.".to_owned()),
            ),
        ]
    );
}

#[test]
fn transform_maps_visible_cli_fields_and_sorts_inputs() {
    let metadata = doc(DocSpec {
        app_name: "demo",
        bin_name: Some("demo-bin"),
        about_id: "root.about",
        fields: vec![
            cli_field(FieldSpec {
                name: "zeta",
                long: Some("zeta"),
                short: Some('z'),
                takes_value: true,
                hide_in_help: false,
                value: Some(ValueType::Enum {
                    variants: vec!["fast".to_owned(), "slow".to_owned()],
                }),
                default: Some("fast"),
                required: true,
            }),
            cli_field(FieldSpec {
                // `alpha` is intentionally positional: no long or short flag,
                // but still invocable because it takes a value.
                name: "alpha",
                long: None,
                short: None,
                takes_value: true,
                hide_in_help: false,
                value: Some(ValueType::Path),
                default: None,
                required: false,
            }),
            cli_field(FieldSpec {
                name: "hidden",
                long: Some("hidden"),
                short: None,
                takes_value: true,
                hide_in_help: true,
                value: Some(ValueType::String),
                default: None,
                required: false,
            }),
            cli_field(FieldSpec {
                name: "non_invocable",
                long: None,
                short: None,
                takes_value: false,
                hide_in_help: false,
                value: Some(ValueType::Bool),
                default: None,
                required: false,
            }),
            non_cli_field("file_only"),
        ],
        subcommands: Vec::new(),
    });

    let context = bridge_ir_to_agent_context(&metadata, "demo_pkg", None);
    let command = context
        .commands
        .first()
        .expect("root command should be generated");
    assert_visible_inputs(command);
}

#[test]
fn transform_recovers_enum_values_from_cli_metadata_for_custom_types() {
    let metadata = doc(DocSpec {
        app_name: "demo",
        bin_name: Some("demo-bin"),
        about_id: "root.about",
        fields: vec![cli_field_with_possible_values(
            FieldSpec {
                name: "log_level",
                long: Some("log-level"),
                short: None,
                takes_value: true,
                hide_in_help: false,
                value: Some(ValueType::Custom {
                    name: "LogLevel".to_owned(),
                }),
                default: Some("LogLevel :: Info"),
                required: false,
            },
            ["Debug", "Info", "Warn", "Error"],
        )],
        subcommands: Vec::new(),
    });

    let context = bridge_ir_to_agent_context(&metadata, "demo_pkg", None);
    let command = context
        .commands
        .first()
        .expect("root command should be generated");
    let input = command
        .inputs
        .first()
        .expect("log level input should be generated");

    assert_eq!(input.value_type.as_deref(), Some("enum"));
    assert_eq!(input.default.as_deref(), Some("LogLevel::Info"));
    assert_eq!(
        input.enum_values,
        ["Debug", "Info", "Warn", "Error"].map(str::to_owned)
    );
}

fn transform_normalizes_default_path_separators() {
    let metadata = doc(DocSpec {
        app_name: "demo",
        bin_name: Some("demo-bin"),
        about_id: "root.about",
        fields: vec![cli_field(FieldSpec {
            name: "host",
            long: Some("host"),
            short: None,
            takes_value: true,
            hide_in_help: false,
            value: Some(ValueType::String),
            default: Some("String :: from(\"localhost\")"),
            required: false,
        })],
        subcommands: Vec::new(),
    });

    let context = bridge_ir_to_agent_context(&metadata, "demo_pkg", None);
    let command = context
        .commands
        .first()
        .expect("root command should be generated");
    let input = command
        .inputs
        .first()
        .expect("host input should be generated");

    assert_eq!(
        input.default.as_deref(),
        Some("String::from(\"localhost\")")
    );
}

fn transform_projects_nested_tree_with_sorted_commands_and_inputs() {
    let context = bridge_ir_to_agent_context(&nested_metadata(), "demo_pkg", None);
    let commands: Vec<_> = context
        .commands
        .iter()
        .map(nested_command_summary)
        .collect();

    assert_eq!(commands, expected_nested_command_summaries());
}
fn transform_omits_missing_or_blank_summaries(
    #[case] lookup: Option<&str>,
    #[case] expected: Option<&str>,
) {
    let metadata = doc(DocSpec::root_without_fields());
    let localizer = StaticLocalizer::maybe("root.about", lookup);

    let context = bridge_ir_to_agent_context(&metadata, "demo_pkg", Some(&localizer));
    let summary = context
        .commands
        .first()
        .expect("root command should be generated")
        .summary
        .as_deref();

    assert_eq!(summary, expected);
}

fn nested_metadata() -> DocMetadata {
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

fn nested_command_summary(command: &ortho_config::AgentCommand) -> NestedCommandSummary {
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

fn expected_nested_command_summaries() -> Vec<NestedCommandSummary> {
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

#[path = "tests_support.rs"]
mod support;
use support::*;
