//! Outcome regressions for the private bridge invocation boundary.
//!
//! Each case pins one of the four outcomes `run_bridge` must distinguish:
//! a launch failure, an unsuccessful exit, malformed JSON, and a successful
//! JSON document. The three middle cases inject captured output through the
//! same boundary the production caller uses, so the diagnosis cannot silently
//! detach from the recorded status and streams.

use super::{BridgeOutput, run_bridge, run_bridge_with};
use crate::error::OrthohelpError;
use camino::Utf8PathBuf;
use std::io;

/// A missing bridge binary is reported as I/O against the requested path.
///
/// This case uses the real spawn path: the fixture supplies a path that cannot
/// exist, so the assertion covers the production parameter rather than only
/// the injected seam.
#[test]
fn a_missing_bridge_binary_reports_io_with_its_path() {
    let directory = tempfile::tempdir().expect("temp dir");
    let executable =
        Utf8PathBuf::from_path_buf(directory.path().join("absent-bridge")).expect("UTF-8 path");

    let error = run_bridge(&executable).expect_err("a missing binary must not succeed");

    match error {
        OrthohelpError::Io { path, .. } => assert_eq!(path, executable),
        other => panic!("missing binary surfaced as {other:?}"),
    }
}

/// A failed launch keeps the requested path, whichever way the launch fails.
#[test]
fn a_failed_launch_preserves_the_requested_path() {
    let executable = Utf8PathBuf::from("/private/bridge/runtime");

    let error = run_bridge_with(&executable, |_| {
        Err(io::Error::from(io::ErrorKind::PermissionDenied))
    })
    .expect_err("a refused launch must not succeed");

    match error {
        OrthohelpError::Io { path, source } => {
            assert_eq!(path, executable);
            assert_eq!(source.kind(), io::ErrorKind::PermissionDenied);
        }
        other => panic!("refused launch surfaced as {other:?}"),
    }
}

/// An unsuccessful exit reports its status and both captured streams.
///
/// Retaining standard error matters because the bridge reports its own failure
/// there; a message built from standard output alone would read as empty.
#[test]
fn an_unsuccessful_exit_reports_status_and_combined_streams() {
    let executable = Utf8PathBuf::from("/private/bridge/runtime");

    let error = run_bridge_with(&executable, |_| {
        Ok(BridgeOutput {
            status: 3,
            stdout: b"partial document".to_vec(),
            stderr: b"linker refused".to_vec(),
        })
    })
    .expect_err("an unsuccessful exit must not succeed");

    match error {
        OrthohelpError::BridgeExecutionFailure { status, message } => {
            assert_eq!(status, 3);
            assert_eq!(message, "partial documentlinker refused");
        }
        other => panic!("unsuccessful exit surfaced as {other:?}"),
    }
}

/// A successful exit with malformed output is reported as invalid JSON.
#[test]
fn a_successful_exit_with_malformed_output_reports_invalid_json() {
    let executable = Utf8PathBuf::from("/private/bridge/runtime");

    let error = run_bridge_with(&executable, |_| {
        Ok(BridgeOutput {
            status: 0,
            stdout: b"not a document".to_vec(),
            stderr: Vec::new(),
        })
    })
    .expect_err("malformed output must not be accepted");

    assert!(
        matches!(error, OrthohelpError::IrJson(_)),
        "malformed output surfaced as {error:?}"
    );
}

/// Invalid UTF-8 inside an otherwise valid document is rejected, not repaired.
///
/// A lossy conversion would substitute U+FFFD for the offending byte and the
/// document would still parse, so the caller could cache a name the bridge
/// never produced.
#[test]
fn invalid_utf8_inside_a_document_is_reported_rather_than_repaired() {
    let executable = Utf8PathBuf::from("/private/bridge/runtime");

    let error = run_bridge_with(&executable, |_| {
        Ok(BridgeOutput {
            status: 0,
            stdout: b"{\"name\":\"\xff\"}".to_vec(),
            stderr: Vec::new(),
        })
    })
    .expect_err("repaired output must not be accepted");

    assert!(
        matches!(error, OrthohelpError::IrJson(_)),
        "invalid UTF-8 surfaced as {error:?}"
    );
}

/// A successful exit returns the document pretty-printed for the caller.
#[test]
fn a_successful_exit_returns_the_pretty_printed_document() {
    let executable = Utf8PathBuf::from("/private/bridge/runtime");

    let rendered = run_bridge_with(&executable, |_| {
        Ok(BridgeOutput {
            status: 0,
            stdout: br#"{"ir_version":"1.0.0","nested":{"b":2,"a":1}}"#.to_vec(),
            stderr: Vec::new(),
        })
    })
    .expect("valid output must be returned");

    let expected = concat!(
        "{\n",
        "  \"ir_version\": \"1.0.0\",\n",
        "  \"nested\": {\n",
        "    \"a\": 1,\n",
        "    \"b\": 2\n",
        "  }\n",
        "}"
    );
    assert_eq!(rendered, expected);
}
