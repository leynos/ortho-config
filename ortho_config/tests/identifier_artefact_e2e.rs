//! End-to-end coverage for opt-in identifier artefact emission.

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

/// Verifies schema output, warm-build preservation, and forced opt-in refresh.
#[test]
#[serial]
fn opt_in_artefact_is_schema_versioned_and_survives_a_warm_build() -> Result<()> {
    reset_target_dir()?;
    ensure!(
        artefact_path()?.is_none(),
        "fresh build target must not contain a stale artefact"
    );
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
    ensure!(
        entries
            .iter()
            .all(|entry| entry.get("path_scope") == Some(&Value::from("standalone"))),
        "all entries should have standalone scope",
    );
    ensure!(
        entries
            .iter()
            .any(|entry| entry.get("id") == Some(&Value::from("fixture-about"))),
        "fixture command identifier should be present",
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
