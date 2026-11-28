//! Direct-entry regressions for the public localisation boundary.
//!
//! The CLI projects the IR before localising it, so a purely pipeline-level
//! test cannot show whether [`localize_doc`] is safe on its own. These tests
//! deliberately hand it unprojected metadata, the state a library caller or a
//! handwritten producer can supply.
//!
//! Each sentinel appears in exactly one slot, and none appears in a public
//! descriptor, so a rendered artefact containing one is proof that the
//! projection was skipped rather than a coincidence of shared text.

use ortho_config::docs::{
    CliMetadata, DefaultValue, DocMetadata, Example, FieldMetadata, HeadingIds,
    ProfileFieldExposure, SectionsMetadata, ValueType,
};
use ortho_config::{LanguageIdentifier, NoOpLocalizer};

use super::localize_doc;

/// Protected default sentinel, also used as the enum member and CLI value.
const SECRET_DEFAULT: &str = "LOCALIZE-SECRET-DEFAULT-SENTINEL";
/// Command-level example sentinel, suppressed by the global example rule.
const SECRET_EXAMPLE: &str = "LOCALIZE-SECRET-EXAMPLE-SENTINEL";

fn sections() -> SectionsMetadata {
    SectionsMetadata {
        headings_ids: HeadingIds {
            name: "n".to_owned(),
            synopsis: "s".to_owned(),
            description: "d".to_owned(),
            options: "o".to_owned(),
            environment: "e".to_owned(),
            files: "f".to_owned(),
            precedence: "p".to_owned(),
            exit_status: "x".to_owned(),
            examples: "ex".to_owned(),
            see_also: "sa".to_owned(),
            commands: None,
        },
        discovery: None,
        precedence: None,
        examples: vec![Example {
            title_id: None,
            code: SECRET_EXAMPLE.to_owned(),
            body_id: None,
        }],
        links: Vec::new(),
        notes: Vec::new(),
    }
}

fn protected_field() -> FieldMetadata {
    FieldMetadata {
        name: "token".to_owned(),
        help_id: "help.token".to_owned(),
        long_help_id: None,
        value: Some(ValueType::Enum {
            variants: vec![SECRET_DEFAULT.to_owned()],
        }),
        default: Some(DefaultValue {
            display: SECRET_DEFAULT.to_owned(),
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
            possible_values: vec![SECRET_DEFAULT.to_owned()],
            hide_in_help: false,
            positional: None,
        }),
        env: None,
        file: None,
        examples: vec![Example {
            title_id: None,
            code: SECRET_EXAMPLE.to_owned(),
            body_id: None,
        }],
        links: Vec::new(),
        notes: Vec::new(),
        profile_field: Some(ProfileFieldExposure::Secret),
    }
}

/// Builds an unprojected root document with one protected child command.
///
/// The protection lives in the child, so a projection applied only at the root
/// node — or only to the node carrying the label — would leave the disclosure
/// visible in the nested artefact.
fn unprojected_tree() -> DocMetadata {
    let child = DocMetadata {
        ir_version: "2.1".to_owned(),
        app_name: "child".to_owned(),
        bin_name: None,
        about_id: "child.about".to_owned(),
        synopsis_id: None,
        sections: sections(),
        fields: vec![protected_field()],
        subcommands: Vec::new(),
        windows: None,
        behaviour: None,
        renderer: None,
        output: None,
        profiles: None,
    };

    DocMetadata {
        ir_version: "2.1".to_owned(),
        app_name: "demo".to_owned(),
        bin_name: None,
        about_id: "demo.about".to_owned(),
        synopsis_id: None,
        sections: sections(),
        fields: Vec::new(),
        subcommands: vec![child],
        windows: None,
        behaviour: None,
        renderer: None,
        output: None,
        profiles: None,
    }
}

fn localize_unprojected() -> super::LocalizedDocMetadata {
    let locale: LanguageIdentifier = "en-US".parse().expect("en-US is a valid locale");
    localize_doc(&unprojected_tree(), &locale, &NoOpLocalizer::new())
}

#[test]
fn direct_entry_projects_protected_slots() {
    let raw = unprojected_tree();
    let raw_child = raw.subcommands.first().expect("child command present");
    let raw_field = raw_child.fields.first().expect("protected field present");
    assert!(
        !raw_field.examples.is_empty(),
        "the fixture must start with a protected field example, or the \
         assertion below cannot detect a retained one"
    );

    let localized = localize_unprojected();
    let child = localized
        .subcommands
        .first()
        .expect("child command present");
    let field = child.fields.first().expect("protected field present");

    assert!(
        field.default.is_none(),
        "a direct caller must not receive the protected default"
    );
    assert!(
        field.examples.is_empty(),
        "a direct caller must not receive protected field examples"
    );
    let cli = field
        .cli
        .as_ref()
        .expect("CLI metadata survives projection");
    assert!(
        cli.possible_values.is_empty(),
        "a direct caller must not receive protected possible values"
    );
    assert!(
        matches!(field.value, Some(ValueType::Enum { ref variants }) if variants.is_empty()),
        "nested enum members of a protected field must be cleared, got {:?}",
        field.value
    );
}

#[test]
fn direct_entry_suppresses_command_examples_across_the_tree() {
    let localized = localize_unprojected();

    assert!(
        localized.sections.examples.is_empty(),
        "the root command example must be suppressed once a descendant is protected"
    );
    let child = localized
        .subcommands
        .first()
        .expect("child command present");
    assert!(
        child.sections.examples.is_empty(),
        "the child command example must be suppressed as well"
    );
}

#[test]
fn direct_entry_keeps_the_public_descriptor() {
    let localized = localize_unprojected();
    let child = localized
        .subcommands
        .first()
        .expect("child command present");
    let field = child.fields.first().expect("protected field present");
    let cli = field
        .cli
        .as_ref()
        .expect("CLI metadata survives projection");

    assert_eq!(field.name, "token", "the public field name survives");
    assert_eq!(
        cli.long.as_deref(),
        Some("token"),
        "the public CLI spelling survives"
    );
    assert_eq!(localized.app_name, "demo", "the document identity survives");
}
