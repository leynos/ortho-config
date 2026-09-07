//! Focused ordering and schema-preservation tests for identifier artefacts.

use anyhow::{Result, ensure};

use super::super::{ArgIdsModel, ClapArgId, CommandIds, FluentMessageId};
use super::*;

/// Creates a Fluent message identifier for a fixed artefact fixture.
fn message_id(value: &str) -> FluentMessageId {
    FluentMessageId(value.to_owned())
}

/// Supplies a stable source location for an expected artefact entry.
fn fixture_source() -> Source {
    Source {
        file: String::from("fixture.rs"),
        line: 12,
        column: 4,
    }
}

/// Builds the complete expected schema entry for the fixed fixture type.
fn expected_entry(id: &str, kind: MessageSuffix, field: Option<&str>, source: &Source) -> Entry {
    Entry {
        id: id.to_owned(),
        kind: kind.as_ref().to_owned(),
        type_name: String::from("fixture::Cli"),
        field: field.map(str::to_owned),
        path_scope: String::from("standalone"),
        source: source.clone(),
        embedded_default: None,
    }
}

/// Verifies command entries retain their schema values, order, and JSON bytes.
#[test]
fn command_entries_preserve_schema_order_and_bytes() -> Result<()> {
    let command = CommandIds {
        about_id: message_id("cli-about"),
        long_about_id: message_id("cli-long-about"),
        usage_id: message_id("cli-usage"),
        version_id: message_id("cli-version"),
        long_version_id: message_id("cli-long-version"),
        after_help_id: message_id("cli-after-help"),
        after_long_help_id: message_id("cli-after-long-help"),
    };
    let source = fixture_source();
    let context = EntryContext {
        type_name: "fixture::Cli",
        source: &source,
    };
    let actual = command_entries(&command, &context);
    let expected = vec![
        expected_entry("cli-about", MessageSuffix::About, None, &source),
        expected_entry("cli-long-about", MessageSuffix::LongAbout, None, &source),
        expected_entry("cli-usage", MessageSuffix::Usage, None, &source),
        expected_entry("cli-version", MessageSuffix::Version, None, &source),
        expected_entry(
            "cli-long-version",
            MessageSuffix::LongVersion,
            None,
            &source,
        ),
        expected_entry("cli-after-help", MessageSuffix::AfterHelp, None, &source),
        expected_entry(
            "cli-after-long-help",
            MessageSuffix::AfterLongHelp,
            None,
            &source,
        ),
    ];

    ensure!(
        json(&actual)? == json(&expected)?,
        "command entries must preserve their schema order and JSON bytes"
    );
    Ok(())
}

/// Verifies argument entries retain field ownership, suffix order, and JSON bytes.
#[test]
fn argument_entries_preserve_field_and_suffix_order() -> Result<()> {
    let args = vec![
        ArgIdsModel {
            field_name: String::from("first"),
            name: ClapArgId(String::from("first")),
            help_id: message_id("cli-args-first-help"),
            long_help_id: message_id("cli-args-first-long-help"),
            value_name_id: message_id("cli-args-first-value-name"),
        },
        ArgIdsModel {
            field_name: String::from("second"),
            name: ClapArgId(String::from("second")),
            help_id: message_id("cli-args-second-help"),
            long_help_id: message_id("cli-args-second-long-help"),
            value_name_id: message_id("cli-args-second-value-name"),
        },
    ];
    let source = fixture_source();
    let context = EntryContext {
        type_name: "fixture::Cli",
        source: &source,
    };
    let actual = argument_entries(&args, &context);
    let expected = vec![
        expected_entry(
            "cli-args-first-help",
            MessageSuffix::Help,
            Some("first"),
            &source,
        ),
        expected_entry(
            "cli-args-first-long-help",
            MessageSuffix::LongHelp,
            Some("first"),
            &source,
        ),
        expected_entry(
            "cli-args-first-value-name",
            MessageSuffix::ValueName,
            Some("first"),
            &source,
        ),
        expected_entry(
            "cli-args-second-help",
            MessageSuffix::Help,
            Some("second"),
            &source,
        ),
        expected_entry(
            "cli-args-second-long-help",
            MessageSuffix::LongHelp,
            Some("second"),
            &source,
        ),
        expected_entry(
            "cli-args-second-value-name",
            MessageSuffix::ValueName,
            Some("second"),
            &source,
        ),
    ];

    ensure!(
        json(&actual)? == json(&expected)?,
        "argument entries must preserve their schema order and JSON bytes"
    );
    Ok(())
}
