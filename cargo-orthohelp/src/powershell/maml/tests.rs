//! Unit tests for MAML rendering.

use super::super::test_fixtures;
use super::*;
use crate::ir::{LocalizedDocMetadata, LocalizedFieldMetadata};
use crate::schema::{CliMetadata, DefaultValue, EnvMetadata, FileMetadata, ValueType};
use crate::test_support::nested_fixture::nested_doc;
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
            value_optional: false,
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

/// Builds a document holding one optional-value boolean flag with the given
/// flag spellings, so each help-text branch can be exercised in isolation.
fn optional_value_doc(
    mut minimal_doc: LocalizedDocMetadata,
    long: Option<&str>,
    short: Option<char>,
) -> LocalizedDocMetadata {
    minimal_doc.fields.push(LocalizedFieldMetadata {
        name: "excited".to_owned(),
        help: "Adds an exclamation mark to the greeting.".to_owned(),
        long_help: None,
        value: Some(ValueType::Bool),
        default: None,
        required: false,
        deprecated: None,
        cli: Some(CliMetadata {
            long: long.map(str::to_owned),
            short,
            value_name: Some("BOOL".to_owned()),
            multiple: false,
            takes_value: true,
            value_optional: true,
            possible_values: vec!["true".to_owned(), "false".to_owned()],
            hide_in_help: false,
        }),
        env: None,
        file: None,
        examples: vec![],
        links: vec![],
        notes: vec![],
    });
    minimal_doc
}

#[rstest]
#[case::long_preferred(
    Some("excited"),
    Some('e'),
    "`--excited` means `true`",
    "`--excited=false`"
)]
#[case::short_when_no_long(None, Some('e'), "`-e` means `true`", "`-e=false`")]
fn render_help_names_the_optional_value_spelling(
    minimal_doc: LocalizedDocMetadata,
    #[case] long: Option<&str>,
    #[case] short: Option<char>,
    #[case] true_phrase: &str,
    #[case] false_phrase: &str,
) {
    let doc = optional_value_doc(minimal_doc, long, short);
    let command = CommandSpec {
        name: "fixture".to_owned(),
        metadata: &doc,
    };
    let xml = render_help(
        &[command],
        MamlOptions {
            should_include_common_parameters: false,
        },
    );

    assert!(
        xml.contains(true_phrase),
        "expected {true_phrase:?} in:\n{xml}"
    );
    assert!(
        xml.contains(false_phrase),
        "expected {false_phrase:?} in:\n{xml}"
    );
}

#[rstest]
fn render_help_avoids_a_nonsense_flag_name(minimal_doc: LocalizedDocMetadata) {
    // Metadata with neither a long nor a short flag cannot name a spelling.
    // The sentence must still describe the value, and must not render the
    // meaningless `the flag=false` that the previous fallback produced.
    let doc = optional_value_doc(minimal_doc, None, None);
    let command = CommandSpec {
        name: "fixture".to_owned(),
        metadata: &doc,
    };
    let xml = render_help(
        &[command],
        MamlOptions {
            should_include_common_parameters: false,
        },
    );

    assert!(
        !xml.contains("the flag=false"),
        "unexpected fallback in:\n{xml}"
    );
    assert!(
        xml.contains("clears a lower-precedence `true`"),
        "expected the flag-free description in:\n{xml}"
    );
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

#[rstest]
fn render_help_includes_nested_subcommand_help() {
    let metadata = nested_doc();
    let greet = metadata
        .subcommands
        .iter()
        .find(|command| command.app_name == "greet")
        .expect("greet command");
    let audit = metadata
        .subcommands
        .iter()
        .find(|command| command.app_name == "admin")
        .and_then(|admin| {
            admin
                .subcommands
                .iter()
                .find(|command| command.app_name == "audit")
        })
        .expect("admin audit command");
    let commands = [
        CommandSpec {
            name: "greet".to_owned(),
            metadata: greet,
        },
        CommandSpec {
            name: "audit".to_owned(),
            metadata: audit,
        },
    ];
    let xml = render_help(
        &commands,
        MamlOptions {
            should_include_common_parameters: false,
        },
    );

    assert!(xml.contains("<command:name>greet</command:name>"));
    assert!(xml.contains("<command:name>audit</command:name>"));
    assert!(xml.contains("Audits fixture state."));
}
