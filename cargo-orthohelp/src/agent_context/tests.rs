//! Unit tests for the documentation IR to agent-context transformer.

use ortho_config::{LocalizationArgs, Localizer};
use rstest::rstest;
use std::collections::BTreeMap;

use super::bridge_ir_to_agent_context;
use crate::schema::{
    CliMetadata, DefaultValue, DocMetadata, FieldMetadata, HeadingIds, SectionsMetadata, ValueType,
};

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

fn command_summary(
    command: &ortho_config::AgentCommand,
) -> (Vec<String>, Option<String>, Option<String>) {
    (
        command.path.clone(),
        command.canonical_verb.clone(),
        command.summary.clone(),
    )
}

fn metadata_with_subcommands(subcommands: Vec<DocMetadata>) -> DocMetadata {
    doc(DocSpec {
        app_name: "demo",
        bin_name: Some("demo-bin"),
        about_id: "root.about",
        fields: Vec::new(),
        subcommands,
    })
}

fn assert_visible_inputs(command: &ortho_config::AgentCommand) {
    let inputs: Vec<_> = command.inputs.iter().map(input_summary).collect();

    assert_eq!(
        inputs,
        [
            (
                "alpha".to_owned(),
                None,
                Some("path".to_owned()),
                None,
                Vec::<String>::new(),
                false,
            ),
            (
                "zeta".to_owned(),
                Some("zeta".to_owned()),
                Some("enum".to_owned()),
                Some("fast".to_owned()),
                vec!["fast".to_owned(), "slow".to_owned()],
                true,
            ),
        ]
    );
}

fn input_summary(
    input: &ortho_config::AgentInput,
) -> (
    String,
    Option<String>,
    Option<String>,
    Option<String>,
    Vec<String>,
    bool,
) {
    (
        input.name.clone(),
        input.long.clone(),
        input.value_type.clone(),
        input.default.clone(),
        input.enum_values.clone(),
        input.required,
    )
}

struct DocSpec {
    app_name: &'static str,
    bin_name: Option<&'static str>,
    about_id: &'static str,
    fields: Vec<FieldMetadata>,
    subcommands: Vec<DocMetadata>,
}

impl DocSpec {
    fn child(app_name: &'static str, about_id: &'static str) -> Self {
        Self {
            app_name,
            bin_name: None,
            about_id,
            fields: Vec::new(),
            subcommands: Vec::new(),
        }
    }

    fn root_without_fields() -> Self {
        Self {
            app_name: "demo",
            bin_name: Some("demo-bin"),
            about_id: "root.about",
            fields: Vec::new(),
            subcommands: Vec::new(),
        }
    }
}

fn doc(spec: DocSpec) -> DocMetadata {
    DocMetadata {
        ir_version: "1.1".to_owned(),
        app_name: spec.app_name.to_owned(),
        bin_name: spec.bin_name.map(str::to_owned),
        about_id: spec.about_id.to_owned(),
        synopsis_id: None,
        sections: sections(),
        fields: spec.fields,
        subcommands: spec.subcommands,
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

struct FieldSpec<'a> {
    name: &'a str,
    long: Option<&'a str>,
    short: Option<char>,
    takes_value: bool,
    hide_in_help: bool,
    value: Option<ValueType>,
    default: Option<&'a str>,
    required: bool,
}

fn cli_field(spec: FieldSpec<'_>) -> FieldMetadata {
    field(FieldParts {
        name: spec.name,
        value: spec.value,
        default: spec.default,
        required: spec.required,
        cli: Some(CliMetadata {
            long: spec.long.map(str::to_owned),
            short: spec.short,
            value_name: None,
            multiple: false,
            takes_value: spec.takes_value,
            possible_values: Vec::new(),
            hide_in_help: spec.hide_in_help,
        }),
    })
}

fn non_cli_field(name: &str) -> FieldMetadata {
    field(FieldParts {
        name,
        value: None,
        default: None,
        required: false,
        cli: None,
    })
}

struct FieldParts<'a> {
    name: &'a str,
    value: Option<ValueType>,
    default: Option<&'a str>,
    required: bool,
    cli: Option<CliMetadata>,
}

fn field(parts: FieldParts<'_>) -> FieldMetadata {
    FieldMetadata {
        name: parts.name.to_owned(),
        help_id: format!("field.{}.help", parts.name),
        long_help_id: None,
        value: parts.value,
        default: parts.default.map(|display| DefaultValue {
            display: display.to_owned(),
        }),
        required: parts.required,
        deprecated: None,
        cli: parts.cli,
        env: None,
        file: None,
        examples: Vec::new(),
        links: Vec::new(),
        notes: Vec::new(),
    }
}

struct StaticLocalizer {
    messages: BTreeMap<String, String>,
}

impl StaticLocalizer {
    fn new<const N: usize>(entries: [(&str, &str); N]) -> Self {
        let messages = entries
            .into_iter()
            .map(|(id, message)| (id.to_owned(), message.to_owned()))
            .collect();
        Self { messages }
    }

    fn maybe(id: &str, message: Option<&str>) -> Self {
        message.map_or_else(
            || Self {
                messages: BTreeMap::new(),
            },
            |resolved| Self::new([(id, resolved)]),
        )
    }
}

impl Localizer for StaticLocalizer {
    fn lookup(&self, id: &str, _args: Option<&LocalizationArgs<'_>>) -> Option<String> {
        self.messages.get(id).cloned()
    }
}
