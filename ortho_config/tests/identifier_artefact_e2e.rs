//! End-to-end coverage for opt-in identifier artefact emission.

use std::fs;
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

fn artefact() -> Result<String> {
    let build_root = fs::read_dir(format!("{TARGET_DIR}/debug/build"))?;
    for entry in build_root {
        let path = entry?.path().join("out/ortho-config/cli-identifiers.json");
        if path.exists() {
            return fs::read_to_string(path).context("read identifier artefact");
        }
    }
    anyhow::bail!("fixture identifier artefact was not emitted")
}

#[test]
#[serial]
fn opt_in_artefact_is_schema_versioned_and_survives_a_warm_build() -> Result<()> {
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
    Ok(())
}
