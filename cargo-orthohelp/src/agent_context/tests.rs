//! Unit tests for the documentation IR to agent-context transformer.

use rstest::rstest;

use super::{bridge_ir_to_agent_context, normalize_default_display};
use crate::schema::{InteractionKind, MutationKind, ValueType};

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
        behaviour: None,
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
        behaviour: None,
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

#[test]
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
            default: Some("String :: from(\"left :: right\")"),
            required: false,
        })],
        subcommands: Vec::new(),
        behaviour: None,
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
        Some("String::from(\"left :: right\")")
    );
}

#[rstest]
#[case(
    r#"Type :: new("left \" :: right")"#,
    r#"Type::new("left \" :: right")"#
)]
#[case(
    r##"Type :: new(r#"left "quoted" :: right"#)"##,
    r##"Type::new(r#"left "quoted" :: right"#)"##
)]
#[case(
    r###"Type :: new(br##"left :: right"##)"###,
    r###"Type::new(br##"left :: right"##)"###
)]
#[case(
    r#"Tuple :: new('\"', Other :: new())"#,
    r#"Tuple::new('\"', Other::new())"#
)]
#[case("Type :: <'static> :: value", "Type::<'static>::value")]
fn default_normalization_preserves_quoted_contents(#[case] display: &str, #[case] expected: &str) {
    assert_eq!(normalize_default_display(display), expected);
}

#[test]
fn transform_projects_nested_tree_with_sorted_commands_and_inputs() {
    let context = bridge_ir_to_agent_context(&nested_metadata(), "demo_pkg", None);
    let commands: Vec<_> = context
        .commands
        .iter()
        .map(nested_command_summary)
        .collect();

    assert_eq!(commands, expected_nested_command_summaries());
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

/// Carries one declared-behaviour scenario for [`transform_maps_declared_behaviour`].
struct DeclaredBehaviourCase {
    app_name: &'static str,
    about_id: &'static str,
    interaction: InteractionKind,
    mutation: MutationKind,
    declared_bypass: Option<&'static str>,
    declared_dry_run: Option<&'static str>,
    expected_interaction: ortho_config::InteractionMode,
    expected_mutation: ortho_config::MutationEffect,
    expected_bypass: Option<&'static str>,
    expected_dry_run: Option<&'static str>,
}

#[rstest]
#[case::interactive_destructive(DeclaredBehaviourCase {
    app_name: "purge",
    about_id: "cmd.purge",
    interaction: InteractionKind::Interactive,
    mutation: MutationKind::Delete,
    declared_bypass: Some("--force"),
    declared_dry_run: None,
    expected_interaction: ortho_config::InteractionMode::Interactive,
    expected_mutation: ortho_config::MutationEffect::Delete,
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
    expected_interaction: ortho_config::InteractionMode::NonInteractive,
    expected_mutation: ortho_config::MutationEffect::ReadOnly,
    expected_bypass: None,
    expected_dry_run: Some("--dry-run"),
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

#[test]
fn transform_keeps_behaviour_fields_unknown_when_undeclared() {
    let metadata = doc(DocSpec::child("version", "cmd.version"));

    let context = bridge_ir_to_agent_context(&metadata, "demo_pkg", None);
    let command = context
        .commands
        .first()
        .expect("version command should be generated");

    assert_eq!(
        command.interaction_mode,
        ortho_config::InteractionMode::Unknown
    );
    assert_eq!(
        command.mutation_effect,
        ortho_config::MutationEffect::Unknown
    );
    assert_eq!(command.bypass_flag, None);
    assert_eq!(command.dry_run_flag, None);
}

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

    assert_eq!(
        prune.interaction_mode,
        ortho_config::InteractionMode::Interactive
    );
    assert_eq!(prune.mutation_effect, ortho_config::MutationEffect::Delete);
    assert_eq!(prune.bypass_flag, None);

    let list = context
        .commands
        .iter()
        .find(|command| command.path == ["demo-bin", "list"])
        .expect("list command should be generated");
    assert_eq!(
        list.interaction_mode,
        ortho_config::InteractionMode::Unknown
    );
    assert_eq!(list.mutation_effect, ortho_config::MutationEffect::Unknown);
}

#[path = "tests_support.rs"]
mod support;
use support::*;

#[path = "tests_nested_support.rs"]
mod nested_support;
use nested_support::*;
