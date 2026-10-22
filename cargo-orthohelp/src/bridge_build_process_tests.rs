//! Caller-level regressions for the supervised Cargo build boundary.
//!
//! The protocol reader and the diagnostic writer each have their own tests.
//! Those tests cannot notice a boundary that never forwards, or one that
//! forwards before the stream has been drained, because neither behaviour is
//! observable from the parser or the sink alone. Every case below therefore
//! runs a controlled Cargo-shaped child through `run_cargo_with`, so the
//! assertions are made on bytes that crossed a real process boundary.

use super::super::protocol::CargoMessages;
use super::super::{CargoBuild, CargoStatus, GeneratedTarget};
use super::run_cargo_with;
use anyhow::{Context, Result, ensure};
use camino::Utf8PathBuf;
use serde_json::{Value, json};
use std::cell::RefCell;
use std::process::Command;

/// Binary name reported by the controlled Cargo artifact records.
const BRIDGE_NAME: &str = "orthohelp_bridge_process_fixture";

/// Records needed before retained diagnostics exceed their documented bound.
///
/// The reader caps one record at 128 KiB, so truncation needs several
/// near-limit records; a single record past the cap is rejected as a protocol
/// error instead of being retained.
const TRUNCATING_RECORDS: usize = 9;

/// Rendered diagnostic bytes carried by each truncation record.
const TRUNCATING_RECORD_BYTES: usize = 120 * 1024;

/// Retained-diagnostics bound, mirroring the protocol reader's private cap.
const DIAGNOSTIC_LIMIT_BYTES: usize = 1024 * 1024;

/// A normal build forwards the retained diagnostics exactly once.
#[test]
fn a_normal_build_forwards_retained_diagnostics() -> Result<()> {
    let fixture = Fixture::new()?;
    fixture.write_stream(&[diagnostic_record("warning: probe")])?;
    let forwarded = RefCell::new(None);

    let build = fixture.run(0, |messages| {
        *forwarded.borrow_mut() = Some(messages.diagnostics.clone());
        Ok(())
    })?;

    ensure!(matches!(build.status, CargoStatus::Success));
    ensure!(build.messages.diagnostics == "warning: probe");
    ensure!(
        forwarded.into_inner().as_deref() == Some("warning: probe"),
        "the boundary did not forward the retained diagnostics"
    );
    Ok(())
}

/// A stream past the retained bound is forwarded, bounded, and announced.
#[test]
fn a_truncated_stream_forwards_a_bounded_prefix() -> Result<()> {
    let fixture = Fixture::new()?;
    let oversized = "x".repeat(TRUNCATING_RECORD_BYTES);
    let records = (0..TRUNCATING_RECORDS)
        .map(|_| diagnostic_record(&oversized))
        .collect::<Vec<_>>();
    fixture.write_stream(&records)?;
    let forwarded = RefCell::new(None);

    let build = fixture.run(0, |messages| {
        *forwarded.borrow_mut() = Some(messages.diagnostics.clone());
        Ok(())
    })?;

    ensure!(
        build.messages.diagnostics_truncated,
        "the stream exceeded the retained bound without being marked truncated"
    );
    let retained = forwarded
        .into_inner()
        .context("forwarding never ran after truncation")?;
    ensure!(
        retained.len() <= DIAGNOSTIC_LIMIT_BYTES,
        "forwarded {} bytes past the documented bound",
        retained.len()
    );
    ensure!(
        !retained.is_empty(),
        "a truncated stream still has a retained prefix to forward"
    );
    Ok(())
}

/// Forwarding observes the whole drained stream rather than a prefix.
///
/// The stream interleaves diagnostics with the artifact record the boundary
/// needs, so a forwarder called before the reader finished would report only
/// the first diagnostic and would not yet know the executable.
#[test]
fn forwarding_runs_after_the_whole_stream_is_drained() -> Result<()> {
    let fixture = Fixture::new()?;
    fixture.write_stream(&[
        diagnostic_record("first"),
        artifact_record(&fixture.executable),
        diagnostic_record("second"),
    ])?;
    let forwarded = RefCell::new(None);

    let build = fixture.run(0, |messages| {
        *forwarded.borrow_mut() = Some(messages.diagnostics.clone());
        Ok(())
    })?;

    ensure!(build.messages.diagnostics == "firstsecond");
    ensure!(
        build.messages.executable.as_ref() == Some(&fixture.executable),
        "the artifact record was not drained before forwarding"
    );
    ensure!(
        forwarded.into_inner().as_deref() == Some("firstsecond"),
        "forwarding observed only part of the stream"
    );
    Ok(())
}

/// A forwarding failure is reported but does not fail the build.
#[test]
fn a_failing_forwarder_keeps_the_build_result() -> Result<()> {
    let fixture = Fixture::new()?;
    fixture.write_stream(&[diagnostic_record("warning: probe")])?;

    let build = fixture.run(0, |_| Err(std::io::Error::other("closed stderr")))?;

    ensure!(matches!(build.status, CargoStatus::Success));
    ensure!(build.messages.diagnostics == "warning: probe");
    Ok(())
}

/// An unsuccessful Cargo exit is forwarded before it is classified.
#[test]
fn an_unsuccessful_exit_is_forwarded_before_it_is_classified() -> Result<()> {
    let fixture = Fixture::new()?;
    fixture.write_stream(&[diagnostic_record("error: bridge failed")])?;
    let forwarded = RefCell::new(None);

    let build = fixture.run(3, |messages| {
        *forwarded.borrow_mut() = Some(messages.diagnostics.clone());
        Ok(())
    })?;

    ensure!(matches!(build.status, CargoStatus::Failure(3)));
    ensure!(
        forwarded.into_inner().as_deref() == Some("error: bridge failed"),
        "an unsuccessful build did not forward its diagnostics"
    );
    Ok(())
}

/// One controlled Cargo-shaped child and the records it may report.
struct Fixture {
    /// Owns the temporary directory for the fixture's lifetime.
    _directory: tempfile::TempDir,
    /// Root of the generated target directory reported to the boundary.
    root: Utf8PathBuf,
    /// Absolute path the artifact record names as the built bridge.
    executable: Utf8PathBuf,
}

impl Fixture {
    /// Create a generated target root with its reported executable path.
    fn new() -> Result<Self> {
        let directory = tempfile::tempdir()?;
        let root =
            Utf8PathBuf::from_path_buf(directory.path().to_path_buf()).map_err(|invalid| {
                anyhow::anyhow!("fixture path was not UTF-8: {}", invalid.display())
            })?;
        let executable = root.join(binary_file_name());
        Ok(Self {
            _directory: directory,
            root,
            executable,
        })
    }

    /// Write the newline-delimited records the child emits on standard output.
    fn write_stream(&self, records: &[Value]) -> Result<()> {
        let mut body = String::new();
        for record in records {
            body.push_str(&serde_json::to_string(record)?);
            body.push('\n');
        }
        std::fs::write(self.stream_path(), body)?;
        Ok(())
    }

    /// Path of the record file the controlled child copies to standard output.
    fn stream_path(&self) -> Utf8PathBuf {
        self.root.join("cargo-stream.json")
    }

    /// Run the boundary against the fixture, exiting the child with `status`.
    fn run<Forward>(&self, status: u8, forward: Forward) -> Result<CargoBuild>
    where
        Forward: FnOnce(&CargoMessages) -> std::io::Result<()>,
    {
        let mut command = self.command(status);
        let target = GeneratedTarget {
            directory: &self.root,
            binary_name: BRIDGE_NAME,
        };
        run_cargo_with(&mut command, target, forward)
            .map_err(|failure| anyhow::anyhow!("controlled Cargo build failed: {}", failure.error))
    }

    /// Build the command that emits the record file and exits with `status`.
    #[cfg(unix)]
    fn command(&self, status: u8) -> Command {
        let mut command = Command::new("/bin/sh");
        command.arg("-c").arg(format!(
            "cat '{}'; exit {status}",
            self.stream_path().as_str()
        ));
        command
    }

    /// Build the command that emits the record file and exits with `status`.
    ///
    /// `/s` makes `cmd.exe` strip only the outermost quote pair and take the
    /// remainder literally, so the quoted path inside the script survives the
    /// parent's own quoting.
    #[cfg(windows)]
    fn command(&self, status: u8) -> Command {
        use std::os::windows::process::CommandExt as _;

        let mut command = Command::new("cmd.exe");
        command.raw_arg(format!(
            "/d /s /c \"type \"{}\" & exit {status}\"",
            self.stream_path().as_str()
        ));
        command
    }
}

/// Generated bridge executable name, including the host's executable suffix.
fn binary_file_name() -> String {
    format!("{BRIDGE_NAME}{}", std::env::consts::EXE_SUFFIX)
}

/// Build one rendered compiler-message record carrying the given text.
fn diagnostic_record(rendered: &str) -> Value {
    json!({
        "reason": "compiler-message",
        "message": { "rendered": rendered },
    })
}

/// Build the compiler-artifact record identifying the fixture executable.
fn artifact_record(executable: &Utf8PathBuf) -> Value {
    json!({
        "reason": "compiler-artifact",
        "target": { "name": BRIDGE_NAME, "kind": ["bin"] },
        "filenames": [executable.as_str()],
        "executable": executable.as_str(),
    })
}
