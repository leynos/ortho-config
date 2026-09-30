//! Focused ordering and schema-preservation tests for identifier artefacts.

use anyhow::{Context, Result, ensure};
use proptest::prelude::*;

use super::super::{ArgIdsModel, ClapArgId, CommandIds, FluentMessageId, LocalizationBase};
use super::*;

const SPLIT_PAYLOAD_BYTES: usize = 524_288;

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

/// Creates an entry whose type name provides a controlled JSON payload size.
fn entry_with_type_name(id: &str, type_name: String) -> Entry {
    Entry {
        id: id.to_owned(),
        kind: String::from("about"),
        type_name,
        field: None,
        path_scope: String::from("standalone"),
        source: fixture_source(),
        embedded_default: None,
    }
}

/// Generates entries that force the renderer through its split-file path.
fn split_entry_strategy() -> impl Strategy<Value = Entry> {
    (
        "[a-z]{1,8}",
        prop_oneof![
            Just(String::from("about")),
            Just(String::from("long-about")),
            Just(String::from("help")),
            Just(String::from("value-name")),
        ],
        "[a-z]{1,8}",
        proptest::option::of("[a-z]{1,8}"),
        262_144_usize..327_680,
    )
        .prop_map(|(id, kind, type_name, field, payload_bytes)| Entry {
            id,
            kind,
            type_name: format!("fixture::{type_name}{}", "x".repeat(payload_bytes)),
            field,
            path_scope: String::from("standalone"),
            source: fixture_source(),
            embedded_default: None,
        })
}

/// Returns files as `(name, contents)` pairs for byte-for-byte comparisons.
fn file_bytes(files: &[ArtefactFile]) -> Vec<(&str, &[u8])> {
    files
        .iter()
        .map(|file| (file.name.as_str(), file.contents.as_slice()))
        .collect()
}

/// Reassembles every document entry from rendered single or split output.
fn round_trip_entries(files: &[ArtefactFile]) -> Result<Vec<Entry>> {
    let Some(first) = files.first() else {
        anyhow::bail!("renderer returned no files");
    };
    if first.name == "cli-identifiers.json" {
        return Ok(serde_json::from_slice::<Document>(&first.contents)?.entries);
    }

    let index: Index = serde_json::from_slice(&first.contents)?;
    let mut entries = Vec::new();
    for part in index.parts {
        let file = files
            .iter()
            .find(|file| file.name == part)
            .context("split artefact part named by index")?;
        entries.extend(serde_json::from_slice::<Document>(&file.contents)?.entries);
    }
    Ok(entries)
}

/// Produces an entry whose serialized document is exactly the renderer cap.
fn entry_at_cap() -> Result<Entry> {
    let template = entry_with_type_name("boundary", String::new());
    let base = json(&Document {
        schema_version: SCHEMA_VERSION,
        entries: vec![template],
    })?
    .len();
    Ok(entry_with_type_name(
        "boundary",
        "x".repeat(CAP_BYTES - base),
    ))
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

/// Builds the two-argument localization model shared by combiner tests.
fn fixture_model() -> LocalizationIds {
    let command = CommandIds {
        about_id: message_id("cli-about"),
        long_about_id: message_id("cli-long-about"),
        usage_id: message_id("cli-usage"),
        version_id: message_id("cli-version"),
        long_version_id: message_id("cli-long-version"),
        after_help_id: message_id("cli-after-help"),
        after_long_help_id: message_id("cli-after-long-help"),
    };
    let args = ["first", "second"]
        .into_iter()
        .map(|field| ArgIdsModel {
            field_name: String::from(field),
            name: ClapArgId(String::from(field)),
            help_id: message_id(&format!("cli-args-{field}-help")),
            long_help_id: message_id(&format!("cli-args-{field}-long-help")),
            value_name_id: message_id(&format!("cli-args-{field}-value-name")),
        })
        .collect();
    LocalizationIds {
        base: LocalizationBase(String::from("fixture")),
        command,
        args,
    }
}

/// Verifies every entry carries one shared derive site and type identity.
fn assert_shared_provenance(actual: &[Entry], type_name: &str) -> Result<()> {
    // The concrete span value is not asserted: `Span::call_site()` outside a
    // real macro expansion carries no file, and pinning that would test
    // proc-macro2 rather than the combiner.
    let Some(first) = actual.first() else {
        return Err(anyhow::anyhow!("combiner must emit at least one entry"));
    };
    let expected_source = json(&first.source)?;
    for entry in actual {
        ensure!(
            entry.type_name == type_name,
            "entry {} must carry the derive type name, got {}",
            entry.id,
            entry.type_name,
        );
        ensure!(
            json(&entry.source)? == expected_source,
            "entry {} must carry the shared derive source, got {:?}",
            entry.id,
            entry.source,
        );
        ensure!(
            entry.path_scope == "standalone",
            "entry {} must record the standalone path scope, got {}",
            entry.id,
            entry.path_scope,
        );
    }
    Ok(())
}

/// Verifies the combiner emits every command entry followed by every
/// argument's three suffix entries, with the full schema populated.
///
/// Cardinality is asserted explicitly: the expected-vector comparisons above
/// pin each field, but only a count makes a silently dropped trailing entry
/// (for example, a truncated suffix list) observable.
#[test]
fn entries_emit_all_command_and_argument_entries() -> Result<()> {
    let model = fixture_model();
    let ident = Ident::new("Cli", Span::call_site());
    let actual = entries(&model, &ident, Span::call_site(), "fixture");

    ensure!(
        actual.len() == 7 + 2 * 3,
        "expected seven command entries plus three per argument, got {}",
        actual.len()
    );

    let expected_ids = [
        "cli-about",
        "cli-long-about",
        "cli-usage",
        "cli-version",
        "cli-long-version",
        "cli-after-help",
        "cli-after-long-help",
        "cli-args-first-help",
        "cli-args-first-long-help",
        "cli-args-first-value-name",
        "cli-args-second-help",
        "cli-args-second-long-help",
        "cli-args-second-value-name",
    ];
    let actual_ids = actual
        .iter()
        .map(|entry| entry.id.as_str())
        .collect::<Vec<_>>();
    ensure!(
        actual_ids == expected_ids,
        "combiner must emit command entries before argument entries, got {actual_ids:?}"
    );

    assert_shared_provenance(&actual, "fixture::Cli")?;
    Ok(())
}


// Renderer coverage lives in a sibling module to keep this file within
// the repository's 400-line limit.
#[path = "artefact_renderer_tests.rs"]
mod artefact_renderer_tests;
