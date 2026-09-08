//! Focused ordering and schema-preservation tests for identifier artefacts.

use anyhow::{Context, Result, ensure};

use super::super::{ArgIdsModel, ClapArgId, CommandIds, FluentMessageId};
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

/// Verifies rendering sorts deterministically regardless of input order.
#[test]
fn renderer_sorts_entries_deterministically() -> Result<()> {
    let entries = vec![
        entry_with_type_name("zeta", String::from("Fixture")),
        entry_with_type_name("alpha", String::from("Fixture")),
        entry_with_type_name("middle", String::from("Fixture")),
    ];
    let first = render(entries.clone())?;
    let second = render(entries)?;
    let ids = round_trip_entries(&first)?
        .into_iter()
        .map(|entry| entry.id)
        .collect::<Vec<_>>();

    ensure!(
        file_bytes(&first) == file_bytes(&second),
        "repeated renders must have identical file bytes"
    );
    ensure!(
        ids == ["alpha", "middle", "zeta"],
        "renderer must sort entries by stable schema order"
    );
    Ok(())
}

/// Verifies split output round-trips every ordered entry through its index.
#[test]
fn split_renderer_round_trips_ordered_entries() -> Result<()> {
    let entries = vec![
        entry_with_type_name("second", "x".repeat(SPLIT_PAYLOAD_BYTES)),
        entry_with_type_name("first", "x".repeat(SPLIT_PAYLOAD_BYTES)),
        entry_with_type_name("third", "x".repeat(SPLIT_PAYLOAD_BYTES)),
    ];
    let files = render(entries.clone())?;
    let round_tripped = round_trip_entries(&files)?;

    ensure!(
        files
            .first()
            .is_some_and(|file| file.name == "cli-identifiers.index.json"),
        "oversized document must begin with its split index"
    );
    ensure!(
        json(&round_tripped)? == json(&ordered(entries))?,
        "split artefact must round-trip every ordered entry"
    );
    Ok(())
}

/// Verifies the one MiB cap keeps boundary output whole and splits above it.
#[test]
fn renderer_honours_the_one_mebibyte_boundary() -> Result<()> {
    let at_cap = entry_at_cap()?;
    let above_cap = entry_with_type_name("boundary", format!("{}x", at_cap.type_name.as_str()));
    let at_cap_files = render(vec![at_cap])?;
    let above_cap_files = render(vec![above_cap])?;

    ensure!(
        at_cap_files
            .first()
            .is_some_and(|file| file.name == "cli-identifiers.json"),
        "a document exactly at the cap must remain unsplit"
    );
    ensure!(
        above_cap_files
            .first()
            .is_some_and(|file| file.name == "cli-identifiers.index.json"),
        "a document above the cap must use the split index"
    );
    Ok(())
}

/// Verifies one oversized entry remains available as a single indexed part.
#[test]
fn renderer_keeps_one_oversized_entry_in_a_single_part() -> Result<()> {
    let entry = entry_with_type_name("oversized", "x".repeat(CAP_BYTES));
    let files = render(vec![entry.clone()])?;
    let part = files
        .iter()
        .find(|file| file.name == "cli-identifiers.0.json")
        .context("single oversized entry part")?;

    ensure!(
        files.len() == 2,
        "single oversized entry must render only an index and one part"
    );
    ensure!(
        serde_json::from_slice::<Document>(&part.contents)?
            .entries
            .len()
            == 1,
        "single oversized entry part must retain the entry"
    );
    ensure!(
        json(&round_trip_entries(&files)?)? == json(&ordered(vec![entry]))?,
        "single oversized entry must round-trip through the index"
    );
    Ok(())
}
