//! Fixtures for the subcommand-selector disclosure boundary.
//!
//! The subcommand derive composes each variant through the child's public
//! `OrthoConfigDocs` method, and that trait documents handwritten
//! implementations as supported, so a handwritten child that answers with
//! unprojected metadata is a supported input. These fixtures supply one such
//! child, an ordinary sibling, and the selector that mixes them, so the
//! boundary tests in `subcommand_docs.rs` can show which of the two the
//! generated methods trust.

use ortho_config::OrthoConfigSubcommandDocs;
use ortho_config::docs::{
    CliMetadata, DefaultValue, DocMetadata, Example, FieldMetadata, HeadingIds,
    ORTHO_DOCS_IR_VERSION, OrthoConfigDocs, ProfileFieldExposure, SectionsMetadata, ValueType,
};

/// Confidential literal no projection can fabricate.
pub const MARKED_SECRET: &str = "MARKED-CHILD-SECRET";

/// Heading identifiers every fixture document carries.
pub fn headings() -> HeadingIds {
    HeadingIds {
        name: "ortho.headings.name".to_owned(),
        synopsis: "ortho.headings.synopsis".to_owned(),
        description: "ortho.headings.description".to_owned(),
        options: "ortho.headings.options".to_owned(),
        environment: "ortho.headings.environment".to_owned(),
        files: "ortho.headings.files".to_owned(),
        precedence: "ortho.headings.precedence".to_owned(),
        exit_status: "ortho.headings.exit-status".to_owned(),
        examples: "ortho.headings.examples".to_owned(),
        see_also: "ortho.headings.see-also".to_owned(),
        commands: Some("ortho.headings.commands".to_owned()),
    }
}

/// Builds a document with no fields, subcommands, or examples.
pub fn empty_metadata(app_name: &str) -> DocMetadata {
    DocMetadata {
        ir_version: ORTHO_DOCS_IR_VERSION.to_owned(),
        app_name: app_name.to_owned(),
        bin_name: None,
        about_id: format!("{app_name}.about"),
        synopsis_id: None,
        sections: SectionsMetadata {
            headings_ids: headings(),
            discovery: None,
            precedence: None,
            examples: Vec::new(),
            links: Vec::new(),
            notes: Vec::new(),
        },
        fields: Vec::new(),
        subcommands: Vec::new(),
        windows: None,
        behaviour: None,
        renderer: None,
        output: None,
        profiles: None,
    }
}

/// Builds a secret field carrying every value-bearing slot.
fn marked_field() -> FieldMetadata {
    FieldMetadata {
        name: "token".to_owned(),
        help_id: "help.token".to_owned(),
        long_help_id: None,
        value: Some(ValueType::Enum {
            variants: vec![MARKED_SECRET.to_owned()],
        }),
        default: Some(DefaultValue {
            display: MARKED_SECRET.to_owned(),
        }),
        required: false,
        deprecated: None,
        cli: Some(CliMetadata {
            long: Some("token".to_owned()),
            short: None,
            value_name: None,
            multiple: false,
            takes_value: true,
            value_optional: false,
            possible_values: vec![MARKED_SECRET.to_owned()],
            hide_in_help: false,
            positional: None,
        }),
        env: None,
        file: None,
        examples: vec![Example {
            title_id: None,
            code: MARKED_SECRET.to_owned(),
            body_id: None,
        }],
        links: Vec::new(),
        notes: Vec::new(),
        profile_field: Some(ProfileFieldExposure::Secret),
    }
}

/// Handwritten child that returns marked but unprojected metadata.
///
/// If the selector trusted its members, the value-bearing slots below would
/// reach the caller intact.
pub struct MarkedArgs;

impl OrthoConfigDocs for MarkedArgs {
    fn get_doc_metadata() -> DocMetadata {
        let mut nested = empty_metadata("marked-inner");
        nested.fields = vec![marked_field()];

        let mut metadata = empty_metadata("marked-args");
        metadata.sections.examples = vec![Example {
            title_id: None,
            code: format!("marked --token={MARKED_SECRET}"),
            body_id: None,
        }];
        metadata.fields = vec![marked_field()];
        metadata.subcommands = vec![nested];
        metadata
    }
}

/// Handwritten sibling with command examples and no protected field.
pub struct PlainArgs;

impl OrthoConfigDocs for PlainArgs {
    fn get_doc_metadata() -> DocMetadata {
        let mut metadata = empty_metadata("plain-args");
        metadata.sections.examples = vec![Example {
            title_id: None,
            code: "plain --name=visible".to_owned(),
            body_id: None,
        }];
        metadata
    }
}

/// Selector mixing a marked handwritten child with an ordinary sibling.
#[derive(OrthoConfigSubcommandDocs)]
pub enum MarkedCommands {
    Marked(MarkedArgs),
    Plain(PlainArgs),
}
