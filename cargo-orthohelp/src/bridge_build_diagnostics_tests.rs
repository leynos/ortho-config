//! Forwarding regressions for bounded Cargo diagnostics.
//!
//! These cases exercise the forwarding step directly, so the assertions remain
//! meaningful when a no-op implementation would satisfy every parser test.

use super::super::protocol::CargoMessages;
use super::write_cargo_diagnostics;
use anyhow::{Result, ensure};

/// Retained diagnostics reach the sink even when nothing was truncated.
///
/// A forwarding implementation that only reported truncation, or one that
/// wrote nothing at all, fails here.
#[test]
fn complete_diagnostics_are_forwarded_without_a_truncation_notice() -> Result<()> {
    let messages = cargo_messages("warning: probe\n", false);
    let mut sink = Vec::new();

    write_cargo_diagnostics(&mut sink, &messages)?;

    ensure!(sink == b"warning: probe\n");
    Ok(())
}

/// Truncated diagnostics keep their retained prefix and announce the loss.
///
/// The notice must follow the retained bytes: a sink that announced truncation
/// first would attach the notice to a different diagnostic stream position.
#[test]
fn truncated_diagnostics_end_with_the_explicit_truncation_notice() -> Result<()> {
    let messages = cargo_messages("error[E0425]: retained prefix\n", true);
    let mut sink = Vec::new();

    write_cargo_diagnostics(&mut sink, &messages)?;

    let emitted = String::from_utf8(sink)?;
    let expected = format!(
        "error[E0425]: retained prefix\n{}\n",
        super::TRUNCATION_NOTICE
    );
    ensure!(emitted == expected, "emitted diagnostics were {emitted:?}");
    ensure!(emitted.ends_with(&format!("{}\n", super::TRUNCATION_NOTICE)));
    Ok(())
}

/// A sink failure is reported rather than swallowed.
///
/// Without this, the caller's warning on forwarding failure could never fire.
#[test]
fn a_failing_sink_reports_the_write_error() -> Result<()> {
    let messages = cargo_messages("warning: probe\n", false);
    let mut sink = FailingSink;

    ensure!(write_cargo_diagnostics(&mut sink, &messages).is_err());
    Ok(())
}

fn cargo_messages(diagnostics: &str, diagnostics_truncated: bool) -> CargoMessages {
    CargoMessages {
        executable: None,
        generated_outputs: Vec::new(),
        diagnostics: diagnostics.to_owned(),
        diagnostics_truncated,
        protocol_error: None,
    }
}

/// Sink that always refuses writes, standing in for a closed stderr.
struct FailingSink;

impl std::io::Write for FailingSink {
    fn write(&mut self, _buffer: &[u8]) -> std::io::Result<usize> {
        Err(std::io::Error::from(std::io::ErrorKind::BrokenPipe))
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Err(std::io::Error::from(std::io::ErrorKind::BrokenPipe))
    }
}
