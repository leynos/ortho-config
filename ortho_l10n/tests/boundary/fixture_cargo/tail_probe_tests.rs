//! Regression for the boundary runner's bounded diagnostic tail.
//!
//! A deadline error is only useful if it carries the *end* of each stream: the
//! last compile-admission queue line or package-cache wait is what says why the
//! child had not progressed. The existing deadline probe writes one short line,
//! which a head-only capture would also retain, so it cannot distinguish a tail
//! from a head. This probe writes a stream several times the retained window,
//! with markers at the very start, in the middle, exactly at the first byte the
//! window keeps, and at the very end.
//!
//! The deadline error must contain the two markers inside the window and must
//! not contain the two that fell out of it, on both streams. A capture that
//! kept only the head would fail the first assertion; one that kept everything
//! would fail the second.

use super::probe_support::{
    announce_ready, scratch_directory, spawn_owned_probe, wait_for_ready_in,
};
use super::readers::DIAGNOSTIC_TAIL_BYTES;
use super::supervise;
use anyhow::{Context, Result, ensure};
use std::io::Write as _;
use std::time::Duration;
use tempfile::TempDir;

/// Deadline given to the tail probe.
///
/// Long enough that a loaded host can start the child and let it write, short
/// enough that the regression adds little to a suite whose members take
/// minutes.
const PROBE_DEADLINE: Duration = Duration::from_secs(5);

/// How long the probe sleeps after writing, so it always outlives the deadline.
const PROBE_LIFETIME: Duration = Duration::from_secs(30);

/// Total bytes the probe writes to each stream.
///
/// Comfortably more than the retained window, so both an early and a middle
/// marker are evicted while the last window remains.
const STREAM_LEN: usize = 100_000;

/// Marks the first bytes of the stream, which the window must not keep.
const HEAD_MARKER: &str = "<HEAD-MARKER>";

/// Marks a point long before the retained window.
const MIDDLE_MARKER: &str = "<MIDDLE-MARKER>";

/// Marks the first bytes the retained window keeps.
const WINDOW_MARKER: &str = "<WINDOW-MARKER>";

/// Marks the end of the stream, which the window must keep.
const TAIL_MARKER: &str = "<TAIL-MARKER>";

/// Where the middle marker sits, well before the retained window.
const MIDDLE_OFFSET: usize = 10_000;

/// Announce readiness, write a stream larger than the retained window, sleep.
///
/// Ignored so an ordinary run never waits for it; the regression selects it by
/// name. Readiness is announced *before* the bytes are written because the
/// probe is spawned without a reader: a stream four times the pipe capacity
/// would block the probe before it could announce anything, and the parent
/// would wait out its readiness bound for a child that is healthy. The parent
/// starts draining the moment it sees the marker, so the writes then complete
/// immediately and the deadline measures retention rather than back-pressure.
#[test]
#[ignore = "driven as a child process by a_deadline_reports_only_the_retained_window"]
fn tail_output_probe() -> Result<()> {
    let stream = tail_fixture();
    announce_ready(&scratch_directory()?)?;
    let mut stdout = std::io::stdout().lock();
    stdout.write_all(&stream)?;
    stdout.flush()?;
    let mut stderr = std::io::stderr().lock();
    stderr.write_all(&stream)?;
    stderr.flush()?;
    std::thread::sleep(PROBE_LIFETIME);
    Ok(())
}

/// A timed-out child's error keeps the window and drops everything before it.
#[test]
fn a_deadline_reports_only_the_retained_window() -> Result<()> {
    let scratch = TempDir::new().context("create the tail probe scratch directory")?;
    let mut child = spawn_owned_probe("tail_output_probe", scratch.path())?;
    wait_for_ready_in(scratch.path(), child.as_mut())?;
    let error = match supervise(child, PROBE_DEADLINE, "tail probe") {
        Err(error) => error,
        Ok(output) => anyhow::bail!(
            "the tail probe should have timed out; it exited with {:?}:\n{}",
            output.status,
            String::from_utf8_lossy(&output.stderr),
        ),
    };
    let message = format!("{error:#}");
    for marker in [WINDOW_MARKER, TAIL_MARKER] {
        ensure!(
            message.matches(marker).count() == 2,
            "both streams should retain {marker}:\n{message}",
        );
    }
    for marker in [HEAD_MARKER, MIDDLE_MARKER] {
        ensure!(
            !message.contains(marker),
            "the error should have evicted {marker}, not retained it:\n{message}",
        );
    }
    let window = format!("last {DIAGNOSTIC_TAIL_BYTES} bytes retained");
    ensure!(
        message.matches(&window).count() == 2,
        "both streams should report the window they kept:\n{message}",
    );
    Ok(())
}

/// Build a stream whose markers sit at the window's edges and outside it.
///
/// The window keeps the last [`DIAGNOSTIC_TAIL_BYTES`] bytes, so the marker
/// placed at `STREAM_LEN - DIAGNOSTIC_TAIL_BYTES` is the window's first content
/// and the tail marker ends the stream. The head and middle markers are before
/// the window and must be evicted. Any preamble a test harness writes shifts
/// the whole stream and the window by the same amount, so these offsets remain
/// correct relative to the retained window.
fn tail_fixture() -> Vec<u8> {
    let mut stream = vec![b'.'; STREAM_LEN];
    place(&mut stream, 0, HEAD_MARKER);
    place(&mut stream, MIDDLE_OFFSET, MIDDLE_MARKER);
    place(
        &mut stream,
        STREAM_LEN - DIAGNOSTIC_TAIL_BYTES,
        WINDOW_MARKER,
    );
    place(&mut stream, STREAM_LEN - TAIL_MARKER.len() - 1, TAIL_MARKER);
    stream
}

/// Write `marker` into `stream` at `offset`.
fn place(stream: &mut [u8], offset: usize, marker: &str) {
    let end = offset + marker.len();
    stream
        .get_mut(offset..end)
        .expect("a marker fits inside the generated stream")
        .copy_from_slice(marker.as_bytes());
}
