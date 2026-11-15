//! Check the reader's observer-failure and detach branches.
//!
//! A drain thread normally ends by observing end-of-file. The two ways it does
//! not are worth forcing: an observer that panics must fail the reader rather
//! than kill the thread silently, and a reader still running when cleanup
//! expires must be detachable so the runner can report it instead of blocking
//! forever. Both are only reachable with a real pipe, so the probes spawn one.

use super::Reader;

use std::process::{Child, ChildStdout, Command, Stdio};
use std::time::{Duration, Instant};

/// A deadline generous enough for a local probe to drain.
const JOIN_BUDGET: Duration = Duration::from_secs(5);

/// Spawn a probe whose stdout is piped, returning the child and that stream.
///
/// The spawn cannot be asserted here: `expect` in a test helper outside a
/// `#[test]` function is denied, so the failure travels back to the case.
fn probe(script: &str) -> std::io::Result<(Child, ChildStdout)> {
    let mut child = Command::new("sh")
        .args(["-c", script])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| std::io::Error::other("the probe stdout is not piped"))?;
    Ok((child, stdout))
}

#[cfg_attr(
    ortho_config_dev_fast,
    ignore = "Cranelift cannot unwind on a spawned thread; LLVM coverage runs this test"
)]
#[test]
fn a_panicking_observer_fails_the_reader() {
    let (mut child, stdout) = probe("printf 'hello'").expect("the probe starts");
    let mut reader = Reader::spawn("stdout", stdout, 1024, |_| panic!("observer exploded"))
        .expect("the reader starts over the probe pipe");

    reader.join_until(Instant::now() + JOIN_BUDGET);

    assert!(reader.is_finished(), "the reader reaches end-of-file");
    let error = reader
        .error()
        .expect("a panicking observer is reported, not swallowed");
    assert!(
        error.contains("stdout observer panicked"),
        "the failure names the observer: {error}"
    );
    child.wait().expect("the probe exits");
}

#[test]
fn a_running_reader_is_detached_and_reports_unfinished() {
    let (mut child, stdout) = probe("sleep 5").expect("the probe starts");
    let mut reader =
        Reader::spawn("stdout", stdout, 1024, |_| {}).expect("the reader starts over the pipe");

    assert!(!reader.is_finished(), "the probe still holds the pipe");
    assert!(
        reader.detach_if_running(),
        "a running reader is reported as detached"
    );
    assert!(
        reader.is_finished(),
        "a detached reader is no longer joinable"
    );
    assert!(
        reader.snapshot().bytes.is_empty(),
        "nothing was written before the detach"
    );

    child.kill().expect("the probe is still running");
    child.wait().expect("the probe is reaped");
}

#[test]
fn a_finished_reader_is_not_detached() {
    let (mut child, stdout) = probe("printf 'done'").expect("the probe starts");
    let mut reader = Reader::spawn("stdout", stdout, 1024, |_| {}).expect("the reader starts");
    child.wait().expect("the probe exits");
    reader.join_until(Instant::now() + JOIN_BUDGET);

    assert!(
        !reader.detach_if_running(),
        "a reader that already reached end-of-file has nothing to detach"
    );
    assert_eq!(reader.snapshot().bytes, b"done");
}

#[test]
fn captured_bytes_keep_the_whole_stream_when_it_fits() {
    let (mut child, stdout) = probe("printf 'abcdefghij'").expect("the probe starts");
    let mut reader = Reader::spawn("stdout", stdout, 1024, |_| {}).expect("the reader starts");
    child.wait().expect("the probe exits");
    reader.join_until(Instant::now() + JOIN_BUDGET);

    let snapshot = reader.snapshot();

    assert_eq!(snapshot.bytes, b"abcdefghij");
    assert_eq!(snapshot.bytes_seen, 10);
    assert!(!snapshot.truncated);
    assert_eq!(snapshot.tail, b"abcdefghij");
}

#[test]
fn a_stalled_writer_can_be_told_to_stop() {
    let (mut child, stdout) = probe("printf 'x'; sleep 5").expect("the probe starts");
    let mut reader = Reader::spawn("stdout", stdout, 1024, |_| {}).expect("the reader starts");
    reader.request_stop();
    reader.join_until(Instant::now() + JOIN_BUDGET);

    assert!(reader.is_finished(), "the reader honours the stop request");
    assert!(
        reader.was_cancelled(),
        "the reader reports that it stopped before end-of-file"
    );

    child.kill().expect("the probe is still running");
    child.wait().expect("the probe is reaped");
}
