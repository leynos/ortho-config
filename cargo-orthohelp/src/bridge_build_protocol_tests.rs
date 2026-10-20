//! Regressions for Cargo JSON parsing and explicit diagnostic forwarding.

use super::GeneratedTarget;
use super::read_cargo_messages;
use anyhow::{Result, ensure};
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
    Ok(())
}
