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
    assert_eq!(
        input.enum_values,
        ["Debug", "Info", "Warn", "Error"].map(str::to_owned)
    );
}

#[rstest]
#[case(None, None)]
#[case(Some(""), None)]
#[case(Some("   "), None)]
#[case(Some("[missing: root.about]"), None)]
#[case(Some("  Useful summary.  "), Some("Useful summary."))]
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

#[path = "tests_support.rs"]
mod support;
use support::*;
