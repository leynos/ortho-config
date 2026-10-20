//! Regressions for Cargo JSON parsing and explicit diagnostic forwarding.

use super::GeneratedTarget;
use super::read_cargo_messages;
use anyhow::{Result, ensure};
use serde_json::json;
use std::io::Cursor;

#[test]
fn compiler_diagnostics_are_written_to_the_injected_sink() -> Result<()> {
    let record = concat!(
        r#"{"reason":"compiler-message","message":{"rendered":"warning: probe\n"}}"#,
        "\n"
    );
    let mut reader = Cursor::new(record.as_bytes());
    let target = GeneratedTarget {
        directory: camino::Utf8Path::new("/target"),
        binary_name: "orthohelp_bridge_fixture",
    };
    let mut diagnostics = Vec::new();

    let messages = read_cargo_messages(&mut reader, &target, &mut diagnostics)?;

    ensure!(diagnostics == b"warning: probe\n");
    ensure!(messages.protocol_error.is_none());
    ensure!(messages.executable.is_none());
    ensure!(messages.generated_outputs.is_empty());
    Ok(())
}

#[test]
fn bridge_artifact_records_keep_only_generated_target_outputs() -> Result<()> {
    let target = GeneratedTarget {
        directory: camino::Utf8Path::new("/target"),
        binary_name: "orthohelp_bridge_workspace_fixture",
    };
    let record = json!({
        "reason": "compiler-artifact",
        "target": { "name": target.binary_name, "kind": ["bin"] },
        "filenames": [
            "/target/debug/orthohelp_bridge_workspace_fixture",
            "/target/debug/orthohelp_bridge_workspace_fixture.d",
            "/target/debug/deps/orthohelp_bridge_workspace_fixture-abc123",
            "/target/debug/deps/orthohelp_bridge_workspace_other-abc123",
            "/outside/orthohelp_bridge_workspace_fixture-abc123"
        ],
        "executable": "/target/debug/orthohelp_bridge_workspace_fixture"
    });
    let mut stream = serde_json::to_vec(&record)?;
    stream.push(b'\n');
    let mut reader = Cursor::new(stream);
    let mut diagnostics = Vec::new();

    let messages = read_cargo_messages(&mut reader, &target, &mut diagnostics)?;

    ensure!(messages.protocol_error.is_none());
    ensure!(messages.generated_outputs.len() == 3);
    ensure!(
        messages
            .generated_outputs
            .contains(&camino::Utf8PathBuf::from(
                "/target/debug/orthohelp_bridge_workspace_fixture"
            ))
    );
    ensure!(
        messages
            .generated_outputs
            .contains(&camino::Utf8PathBuf::from(
                "/target/debug/orthohelp_bridge_workspace_fixture.d"
            ))
    );
    ensure!(
        messages
            .generated_outputs
            .contains(&camino::Utf8PathBuf::from(
                "/target/debug/deps/orthohelp_bridge_workspace_fixture-abc123"
            ))
    );
    Ok(())
}
