//! End-to-end coverage for opt-in identifier artefact emission.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result, ensure};
use serde_json::Value;
use serial_test::serial;

const TARGET_DIR: &str = "target/identifier-e2e";

fn build_fixture(emit: bool) -> Result<()> {
    let mut command = Command::new("cargo");
    command
        .args([
            "build",
            "-p",
            "orthohelp_fixture",
            "--target-dir",
            TARGET_DIR,
        ])
        .env_remove("ORTHO_CONFIG_EMIT_IDENTIFIERS");
    if emit {
        command.env("ORTHO_CONFIG_EMIT_IDENTIFIERS", "1");
    }
    let status = command.status().context("run fixture build")?;
    ensure!(status.success(), "fixture build should succeed");
    Ok(())
}

fn clean_fixture() -> Result<()> {
    let status = Command::new("cargo")
        .args([
            "clean",
            "-p",
            "orthohelp_fixture",
            "--target-dir",
            TARGET_DIR,
        ])
        .status()
        .context("clean fixture build")?;
    ensure!(status.success(), "fixture clean should succeed");
    Ok(())
}

fn artefact_path() -> Result<Option<PathBuf>> {
    let build_root = Path::new(TARGET_DIR).join("debug/build");
    if !build_root.exists() {
        return Ok(None);
    }
    let build_entries = fs::read_dir(build_root)?;
    for entry in build_entries {
        let path = entry?.path().join("out/ortho-config/cli-identifiers.json");
        if path.exists() {
            return Ok(Some(path));
        }
    }
    Ok(None)
}

fn artefact() -> Result<String> {
    let path = artefact_path()?.context("fixture identifier artefact was not emitted")?;
    fs::read_to_string(path).context("read identifier artefact")
}

fn reset_target_dir() -> Result<()> {
    let target = Path::new(TARGET_DIR);
    if target.exists() {
        fs::remove_dir_all(target).context("remove stale identifier artefact target")?;
    }
    Ok(())
}

/// Returns the named field of an entry as a string, failing when absent.
fn string_field<'a>(entry: &'a Value, key: &str, id: &str) -> Result<&'a str> {
    entry
        .get(key)
        .and_then(Value::as_str)
        .with_context(|| format!("entry {id} must have a string `{key}`"))
}

/// Verifies every entry carries the complete, well-formed schema.
///
/// Existence-only checks would accept an artefact whose entries had lost their
/// `kind`, provenance, or field ownership; each field is asserted directly so
/// a partial emission is a hard failure.
fn assert_entries_are_well_formed(entries: &[Value]) -> Result<()> {
    ensure!(!entries.is_empty(), "artefact must contain entries");
    for entry in entries {
        let id = string_field(entry, "id", "<unknown>")?;
        string_field(entry, "kind", id)?;
        string_field(entry, "type", id)?;
        ensure!(
            entry.get("path_scope").and_then(Value::as_str) == Some("standalone"),
            "entry {id} should have standalone scope",
        );
        let source = entry
            .get("source")
            .and_then(Value::as_object)
            .with_context(|| format!("entry {id} must carry a source object"))?;
        ensure!(
            source.get("file").and_then(Value::as_str).is_some_and(|f| !f.is_empty()),
            "entry {id} must record a non-empty source file",
        );
        for coordinate in ["line", "column"] {
            ensure!(
                source.get(coordinate).and_then(Value::as_u64).is_some(),
                "entry {id} must record a numeric source {coordinate}",
            );
        }
        ensure!(
            entry.get("embedded_default").is_some(),
            "entry {id} must serialise `embedded_default` even when null",
        );
    }
    Ok(())
}

/// Verifies command entries carry no field and argument entries carry three
/// distinct suffixes each.
fn assert_entry_ownership(entries: &[Value]) -> Result<()> {
    let mut suffixes_per_field: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for entry in entries {
        let id = string_field(entry, "id", "<unknown>")?;
        let kind = string_field(entry, "kind", id)?;
        match entry.get("field").and_then(Value::as_str) {
            None => ensure!(
                !matches!(kind, "help" | "long_help" | "value_name"),
                "argument entry {id} must name its owning field",
            ),
            Some(field) => {
                suffixes_per_field
                    .entry(field.to_owned())
                    .or_default()
                    .insert(kind.to_owned());
            }
        }
    }

    ensure!(
        !suffixes_per_field.is_empty(),
        "fixture must contribute at least one argument",
    );
    for (field, kinds) in &suffixes_per_field {
        let expected = ["help", "long_help", "value_name"]
            .into_iter()
            .map(str::to_owned)
            .collect::<BTreeSet<_>>();
        ensure!(
            kinds == &expected,
            "field {field} must expose exactly help, long_help, and value_name; got {kinds:?}",
        );
    }
    Ok(())
}

/// Verifies schema output, warm-build preservation, and forced opt-in refresh.
#[test]
#[serial]
fn opt_in_artefact_is_schema_versioned_and_survives_a_warm_build() -> Result<()> {
    reset_target_dir()?;
    ensure!(
        artefact_path()?.is_none(),
        "fresh build target must not contain a stale artefact"
    );
    build_fixture(false)?;
    ensure!(
        artefact_path()?.is_none(),
        "fresh build without opt-in must not emit an identifier artefact"
    );

    reset_target_dir()?;
    build_fixture(true)?;
    let first = artefact()?;
    let document: Value = serde_json::from_str(&first)?;
    ensure!(
        document.get("schema_version") == Some(&Value::from(1)),
        "expected schema version 1",
    );
    let entries = document
        .get("entries")
        .and_then(Value::as_array)
        .context("entries array")?;
    assert_entries_are_well_formed(entries)?;
    assert_entry_ownership(entries)?;

    let ids = entries
        .iter()
        .filter_map(|entry| entry.get("id").and_then(Value::as_str))
        .collect::<BTreeSet<_>>();
    for required in ["fixture-about", "simple-fixture-args-host-help"] {
        ensure!(
            ids.contains(required),
            "identifier artefact should contain {required}",
        );
    }
    // Guards the escaped regression where the arg id or the suffix was
    // re-spelled: `clap_derive` keeps the raw field name as the argument id,
    // and the long-help suffix is underscore-separated.
    ensure!(
        ids.contains("simple-fixture-args-is_dry_run-long_help"),
        "underscored fields must keep their raw field name and `long_help` suffix",
    );
    ensure!(
        !ids.iter().any(|id| id.contains("long-help")),
        "no identifier may use a hyphenated `long-help` suffix",
    );

    build_fixture(false)?;
    ensure!(
        artefact()? == first,
        "warm build without opt-in must not rewrite artefact"
    );

    clean_fixture()?;
    ensure!(
        artefact_path()?.is_none(),
        "clean must remove the previous opt-in artefact"
    );
    build_fixture(true)?;
    ensure!(
        artefact()? == first,
        "forced opt-in rebuild must recreate the identifier artefact"
    );
    Ok(())
}
