//! Unit tests for MAML rendering.

use super::super::test_fixtures;
use super::*;
use crate::ir::{LocalizedDocMetadata, LocalizedFieldMetadata};
use crate::schema::{CliMetadata, DefaultValue, EnvMetadata, FileMetadata, ValueType};
use rstest::{fixture, rstest};

#[fixture]
fn minimal_doc() -> LocalizedDocMetadata {
    test_fixtures::minimal_doc("en-US", "Fixture app")
}

#[rstest]
fn render_help_includes_common_parameters(minimal_doc: LocalizedDocMetadata) {
    let command = CommandSpec {
        name: "fixture".to_owned(),
        metadata: &minimal_doc,
    };
    let xml = render_help(
        &[command],
        MamlOptions {
            should_include_common_parameters: true,
        },
    );
    assert!(xml.contains("<command:commonParameters"));
}

#[rstest]
fn render_help_renders_enum_values(mut minimal_doc: LocalizedDocMetadata) {
    minimal_doc.fields.push(LocalizedFieldMetadata {
        name: "level".to_owned(),
        help: "Log level".to_owned(),
        long_help: None,
        value: Some(ValueType::Enum {
            variants: vec!["info".to_owned(), "warn".to_owned()],
        }),
        default: Some(DefaultValue {
            display: "info".to_owned(),
        }),
        required: false,
        deprecated: None,
        cli: Some(CliMetadata {
            long: Some("level".to_owned()),
            short: Some('l'),
            value_name: None,
            multiple: false,
            takes_value: true,
            possible_values: vec![],
            hide_in_help: false,
        }),
        env: Some(EnvMetadata {
            var_name: "FIXTURE_LEVEL".to_owned(),
        }),
        file: Some(FileMetadata {
            key_path: "level".to_owned(),
        }),
        examples: vec![],
        links: vec![],
        notes: vec![],
    });

    let command = CommandSpec {
        name: "fixture".to_owned(),
        metadata: &minimal_doc,
    };
    let xml = render_help(
        &[command],
        MamlOptions {
            should_include_common_parameters: false,
        },
    );
    assert!(xml.contains("Possible values: info, warn."));
    assert!(xml.contains("Environment variable: FIXTURE_LEVEL."));
    assert!(xml.contains("Config key: level."));
}

#[rstest]
fn xml_escapes_reserved_chars(mut minimal_doc: LocalizedDocMetadata) {
    minimal_doc.about = "Use <tag> & more".to_owned();
    let command = CommandSpec {
        name: "fixture".to_owned(),
        metadata: &minimal_doc,
    };
    let xml = render_help(
        &[command],
        MamlOptions {
            should_include_common_parameters: false,
        },
    );
    assert!(xml.contains("Use &lt;tag&gt; &amp; more"));
}
