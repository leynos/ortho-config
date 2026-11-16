//! Builders and assertion helpers for the agent-context bridge tests.
//!
//! The IR builders are crate-visible because the skill-validation bridge
//! regression drives the real bridge before the surface adapter; sharing them
//! keeps one definition of a valid `DocMetadata` fixture rather than a second,
//! drifting copy.

use ortho_config::{LocalizationArgs, Localizer};
use std::collections::BTreeMap;

use crate::schema::{
    BehaviourMetadata, CliMetadata, DefaultValue, DocMetadata, FieldMetadata, HeadingIds,
    InteractionKind, MutationKind, SectionsMetadata, ValueType,
};

/// Projects a generated command into the path, canonical verb, and summary
/// compared by bridge tests.
pub(crate) fn command_summary(
    command: &ortho_config::AgentCommand,
) -> (Vec<String>, Option<String>, Option<String>) {
    (
        command.path.clone(),
        command.canonical_verb.clone(),
        command.summary.clone(),
    )
}

/// Builds root documentation metadata containing the supplied subcommands.
pub(crate) fn metadata_with_subcommands(subcommands: Vec<DocMetadata>) -> DocMetadata {
    doc(DocSpec {
        app_name: "demo",
        bin_name: Some("demo-bin"),
        about_id: "root.about",
        fields: Vec::new(),
        subcommands,
        behaviour: None,
    })
}

/// Asserts that a command contains only the visible, invocable fixture inputs
/// in their canonical order.
pub(crate) fn assert_visible_inputs(command: &ortho_config::AgentCommand) {
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

/// Projects an agent input into the fields compared by bridge tests.
pub(crate) fn input_summary(
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

pub(crate) struct DocSpec {
    pub(crate) app_name: &'static str,
    pub(crate) bin_name: Option<&'static str>,
    pub(crate) about_id: &'static str,
    pub(crate) fields: Vec<FieldMetadata>,
    pub(crate) subcommands: Vec<DocMetadata>,
    pub(crate) behaviour: Option<BehaviourMetadata>,
}

impl DocSpec {
    /// Creates an empty child-command specification with the given command and
    /// localization identifiers.
    pub(crate) fn child(app_name: &'static str, about_id: &'static str) -> Self {
        Self {
            app_name,
            bin_name: None,
            about_id,
            fields: Vec::new(),
            subcommands: Vec::new(),
            behaviour: None,
        }
    }

    /// Creates the standard root-command specification without fields.
    pub(crate) fn root_without_fields() -> Self {
        Self {
            app_name: "demo",
            bin_name: Some("demo-bin"),
            about_id: "root.about",
            fields: Vec::new(),
            subcommands: Vec::new(),
            behaviour: None,
        }
    }
}

/// Expands a compact test specification into documentation metadata.
pub(crate) fn doc(spec: DocSpec) -> DocMetadata {
    DocMetadata {
        ir_version: crate::schema::ORTHO_DOCS_IR_VERSION.to_owned(),
        app_name: spec.app_name.to_owned(),
        bin_name: spec.bin_name.map(str::to_owned),
        about_id: spec.about_id.to_owned(),
        synopsis_id: None,
        sections: sections(),
        fields: spec.fields,
        subcommands: spec.subcommands,
        windows: None,
        behaviour: spec.behaviour,
        renderer: None,
        output: None,
        profiles: None,
    }
}

/// Builds the stable section-heading metadata shared by bridge fixtures.
pub(crate) fn sections() -> SectionsMetadata {
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

pub(crate) struct FieldSpec<'a> {
    pub(crate) name: &'a str,
    pub(crate) long: Option<&'a str>,
    pub(crate) short: Option<char>,
    pub(crate) takes_value: bool,
    pub(crate) hide_in_help: bool,
    pub(crate) value: Option<ValueType>,
    pub(crate) default: Option<&'a str>,
    pub(crate) required: bool,
}

/// Builds CLI-backed field metadata without enumerated possible values.
pub(crate) fn cli_field(spec: FieldSpec<'_>) -> FieldMetadata {
    cli_field_with_possible_values(spec, [])
}

/// Builds CLI-backed field metadata and records the supplied possible values.
pub(crate) fn cli_field_with_possible_values<const N: usize>(
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
            value_optional: false,
            possible_values: possible_values.map(str::to_owned).to_vec(),
            hide_in_help: spec.hide_in_help,
        }),
    })
}

/// Builds field metadata with no CLI invocation contract.
pub(crate) fn non_cli_field(name: &str) -> FieldMetadata {
    field(FieldParts {
        name,
        value: None,
        default: None,
        required: false,
        cli: None,
    })
}

pub(crate) struct FieldParts<'a> {
    pub(crate) name: &'a str,
    pub(crate) value: Option<ValueType>,
    pub(crate) default: Option<&'a str>,
    pub(crate) required: bool,
    pub(crate) cli: Option<CliMetadata>,
}

/// Expands field fixture parts into complete documentation field metadata.
pub(crate) fn field(parts: FieldParts<'_>) -> FieldMetadata {
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

/// Builds a fully declared behaviour block for bridge test fixtures.
pub(crate) fn declared_behaviour(
    interaction: InteractionKind,
    mutation: MutationKind,
    bypass: Option<&str>,
    dry_run: Option<&str>,
) -> BehaviourMetadata {
    BehaviourMetadata {
        interaction: Some(interaction),
        mutation: Some(mutation),
        bypass: bypass.map(str::to_owned),
        dry_run: dry_run.map(str::to_owned),
    }
}

pub(crate) struct StaticLocalizer {
    messages: BTreeMap<String, String>,
}

impl StaticLocalizer {
    /// Creates a deterministic localizer from fixed identifier-message pairs.
    pub(crate) fn new<const N: usize>(entries: [(&str, &str); N]) -> Self {
        let messages = entries
            .into_iter()
            .map(|(id, message)| (id.to_owned(), message.to_owned()))
            .collect();
        Self { messages }
    }

    /// Creates a localizer containing one optional message for omission tests.
    pub(crate) fn maybe(id: &str, message: Option<&str>) -> Self {
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
