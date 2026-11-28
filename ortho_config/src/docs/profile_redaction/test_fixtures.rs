//! Document fixtures shared by the profile disclosure projection tests.
//!
//! The builders live apart from the tests so each test file stays small enough
//! to read in one sitting, and so a fixture change has a single definition to
//! update rather than one copy per test module.

use super::ProfileFieldExposure;
use crate::docs::{
    CliMetadata, DefaultValue, DocMetadata, Example, FieldMetadata, HeadingIds, SectionsMetadata,
    ValueType,
};

fn heading_ids() -> HeadingIds {
    HeadingIds {
        name: "n".into(),
        synopsis: "s".into(),
        description: "d".into(),
        options: "o".into(),
        environment: "e".into(),
        files: "f".into(),
        precedence: "p".into(),
        exit_status: "x".into(),
        examples: "ex".into(),
        see_also: "sa".into(),
        commands: None,
    }
}

fn sections(examples: Vec<Example>) -> SectionsMetadata {
    SectionsMetadata {
        headings_ids: heading_ids(),
        discovery: None,
        precedence: None,
        examples,
        links: Vec::new(),
        notes: Vec::new(),
    }
}

pub(super) fn example(code: &str) -> Example {
    Example {
        title_id: None,
        code: code.into(),
        body_id: None,
    }
}

fn cli(possible_values: &[&str]) -> CliMetadata {
    CliMetadata {
        long: Some("token".into()),
        short: None,
        value_name: None,
        multiple: false,
        takes_value: true,
        value_optional: false,
        possible_values: possible_values.iter().map(ToString::to_string).collect(),
        hide_in_help: false,
        positional: None,
    }
}

pub(super) fn field(name: &str, exposure: Option<ProfileFieldExposure>) -> FieldMetadata {
    FieldMetadata {
        name: name.into(),
        help_id: format!("help.{name}"),
        long_help_id: Some(format!("long.{name}")),
        value: Some(ValueType::String),
        default: Some(DefaultValue {
            display: format!("{name}-sentinel"),
        }),
        required: false,
        deprecated: None,
        cli: Some(cli(&["one", "two"])),
        env: None,
        file: None,
        examples: vec![example(&format!("{name}-sentinel-example"))],
        links: Vec::new(),
        notes: Vec::new(),
        profile_field: exposure,
    }
}

pub(super) fn doc(
    app: &str,
    fields: Vec<FieldMetadata>,
    subcommands: Vec<DocMetadata>,
) -> DocMetadata {
    DocMetadata {
        ir_version: "2.1".into(),
        app_name: app.into(),
        bin_name: None,
        about_id: format!("{app}.about"),
        synopsis_id: None,
        sections: sections(vec![example(&format!("{app}-example"))]),
        fields,
        subcommands,
        windows: None,
        behaviour: None,
        renderer: None,
        output: None,
        profiles: None,
    }
}

/// A root with command examples, an ordinary field, and a protected child.
pub(super) fn mixed_tree() -> DocMetadata {
    let child = doc(
        "child",
        vec![field("secret", Some(ProfileFieldExposure::Secret))],
        Vec::new(),
    );
    let sibling = doc("sibling", vec![field("plain", None)], Vec::new());
    let mut root = doc("root", vec![field("ordinary", None)], vec![child, sibling]);
    root.sections.examples = vec![example("root-example"), example("root-example-2")];
    root
}

/// Returns the sole field of a document, which every fixture here declares.
pub(super) fn only_field(metadata: &DocMetadata) -> &FieldMetadata {
    metadata.fields.first().expect("fixtures declare one field")
}

/// Mutable counterpart of [`only_field`].
pub(super) fn only_field_mut(metadata: &mut DocMetadata) -> &mut FieldMetadata {
    metadata
        .fields
        .first_mut()
        .expect("fixtures declare one field")
}

/// Returns the first subcommand, which every fixture tree declares.
pub(super) fn first_subcommand(metadata: &DocMetadata) -> &DocMetadata {
    metadata
        .subcommands
        .first()
        .expect("fixture trees declare two subcommands")
}

/// Mutable counterpart of [`first_subcommand`].
pub(super) fn first_subcommand_mut(metadata: &mut DocMetadata) -> &mut DocMetadata {
    metadata
        .subcommands
        .first_mut()
        .expect("fixture trees declare two subcommands")
}

/// Returns the second subcommand, which every fixture tree declares.
pub(super) fn second_subcommand(metadata: &DocMetadata) -> &DocMetadata {
    metadata
        .subcommands
        .get(1)
        .expect("fixture trees declare two subcommands")
}
