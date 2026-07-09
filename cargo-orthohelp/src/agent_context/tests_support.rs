//! Builders and assertion helpers for the agent-context bridge tests.

use ortho_config::{LocalizationArgs, Localizer};
use std::collections::BTreeMap;

use crate::schema::{
    CliMetadata, DefaultValue, DocMetadata, FieldMetadata, HeadingIds, SectionsMetadata, ValueType,
};

pub(super) fn command_summary(
    command: &ortho_config::AgentCommand,
) -> (Vec<String>, Option<String>, Option<String>) {
    (
        command.path.clone(),
        command.canonical_verb.clone(),
        command.summary.clone(),
    )
}

pub(super) fn metadata_with_subcommands(subcommands: Vec<DocMetadata>) -> DocMetadata {
    doc(DocSpec {
        app_name: "demo",
        bin_name: Some("demo-bin"),
        about_id: "root.about",
        fields: Vec::new(),
        subcommands,
    })
}

pub(super) fn assert_visible_inputs(command: &ortho_config::AgentCommand) {
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

pub(super) fn input_summary(
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

pub(super) struct DocSpec {
    pub(super) app_name: &'static str,
    pub(super) bin_name: Option<&'static str>,
    pub(super) about_id: &'static str,
    pub(super) fields: Vec<FieldMetadata>,
    pub(super) subcommands: Vec<DocMetadata>,
}

impl DocSpec {
    pub(super) fn child(app_name: &'static str, about_id: &'static str) -> Self {
        Self {
            app_name,
            bin_name: None,
            about_id,
            fields: Vec::new(),
            subcommands: Vec::new(),
        }
    }

    pub(super) fn root_without_fields() -> Self {
        Self {
            app_name: "demo",
            bin_name: Some("demo-bin"),
            about_id: "root.about",
            fields: Vec::new(),
            subcommands: Vec::new(),
        }
    }
}

pub(super) fn doc(spec: DocSpec) -> DocMetadata {
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

pub(super) fn sections() -> SectionsMetadata {
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

pub(super) struct FieldSpec<'a> {
    pub(super) name: &'a str,
    pub(super) long: Option<&'a str>,
    pub(super) short: Option<char>,
    pub(super) takes_value: bool,
    pub(super) hide_in_help: bool,
    pub(super) value: Option<ValueType>,
    pub(super) default: Option<&'a str>,
    pub(super) required: bool,
}

pub(super) fn cli_field(spec: FieldSpec<'_>) -> FieldMetadata {
    cli_field_with_possible_values(spec, [])
}

pub(super) fn cli_field_with_possible_values<const N: usize>(
    spec: FieldSpec<'_>,
    possible_values: [&str; N],
) -> FieldMetadata {
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
            possible_values: possible_values.map(str::to_owned).to_vec(),
            hide_in_help: spec.hide_in_help,
        }),
    })
}

pub(super) fn non_cli_field(name: &str) -> FieldMetadata {
    field(FieldParts {
        name,
        value: None,
        default: None,
        required: false,
        cli: None,
    })
}

pub(super) struct FieldParts<'a> {
    pub(super) name: &'a str,
    pub(super) value: Option<ValueType>,
    pub(super) default: Option<&'a str>,
    pub(super) required: bool,
    pub(super) cli: Option<CliMetadata>,
}

pub(super) fn field(parts: FieldParts<'_>) -> FieldMetadata {
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

pub(super) struct StaticLocalizer {
    messages: BTreeMap<String, String>,
}

impl StaticLocalizer {
    pub(super) fn new<const N: usize>(entries: [(&str, &str); N]) -> Self {
        let messages = entries
            .into_iter()
            .map(|(id, message)| (id.to_owned(), message.to_owned()))
            .collect();
        Self { messages }
    }

    pub(super) fn maybe(id: &str, message: Option<&str>) -> Self {
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
