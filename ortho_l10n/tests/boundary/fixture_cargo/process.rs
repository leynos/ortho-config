//! Running a nested Cargo child under a deadline, with its output retained.
//!
//! A boundary fixture drives Cargo, and Cargo can spend its whole allowance
//! waiting for a compile-admission slot or a package-cache lock without ever
//! reaching a compiler. This module makes that wait attributable: the child's
//! streams are drained on their own threads, a bounded tail of each is
//! published while it runs, and a deadline error carries that tail.
//!
//! It also serializes nested invocations within the test process. `cargo test`
//! runs a binary's tests as threads, which is the one runner where nextest's
//! `boundary_packaging` group cannot help.

#[cfg(test)]
#[path = "deadline_probe_tests.rs"]
mod deadline_probe_tests;

use anyhow::{Context, Result};
use std::collections::VecDeque;
use std::io::{self, Read};
use std::process::{Command, Output, Stdio};
use std::sync::{Arc, LazyLock, Mutex, MutexGuard, PoisonError};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};
use wait_timeout::ChildExt as _;

/// Bytes kept from the end of each child stream to explain a deadline failure.
///
/// The tail, not the head, is the part that says what the child was doing when
/// the deadline expired: compile-admission queue lines and the rustc heartbeat
/// are printed repeatedly, and the last one names the wait that was in force.
const DIAGNOSTIC_TAIL_BYTES: usize = 64 * 1024;

/// How long a stream reader may keep draining after a timed-out child is killed.
///
/// The killed child's own copies of its pipes close, but a build script, linker,
/// or other descendant it started may still hold one. The suite has to report
/// what it captured instead of blocking on that descendant, so a reader which
/// has not reached end of file by then is detached.
const READER_GRACE: Duration = Duration::from_secs(2);

/// Serializes nested Cargo invocations within one test process.
///
/// `cargo test` runs a binary's tests as threads in a single process, so the
/// boundary contracts' nested Cargo calls queue on the build-directory lock of
/// the target directory they share. That wait is charged to whichever deadline
/// is counting, which turns a healthy child into a reported hang. nextest
/// already serializes this binary through its `boundary_packaging` group; this
/// gives `cargo test` and `make boundary` the same guarantee. It is acquired
/// before the child is spawned, so time spent waiting for a sibling is never
/// counted against the child's own deadline.
static NESTED_CARGO: LazyLock<Mutex<()>> = LazyLock::new(Mutex::default);

/// Run `command` to completion or the deadline, whichever comes first.
///
/// Both streams are drained on their own threads. A nested Cargo build prints
/// far more than a pipe buffer holds, and a runner that waits for the exit
/// status before reading the pipes therefore deadlocks: the child blocks
/// writing, the parent blocks waiting, and the deadline reports a hang that is
/// really the harness's own. Draining concurrently removes that failure mode,
/// so a reported deadline means the child itself made no progress.
///
/// Each reader also publishes a bounded tail while it drains, and that tail is
/// part of the deadline error. A child waiting for a compile-admission slot or
/// a package-cache lock prints no compiler diagnostic of its own; the queue and
/// heartbeat lines it does print are the only evidence that distinguishes the
/// wait from a hang. Discarding them made earlier failures unattributable.
///
/// Nested invocations are serialized within the process; see [`NESTED_CARGO`].
///
/// # Errors
///
/// Returns an error when the child cannot be spawned or collected, or when it
/// is still running at the deadline. A timed-out child is killed before the
/// error is returned, so the test process is not left with an orphan holding
/// the package-cache lock. The error carries the bounded tail of both streams.
pub(crate) fn run_with_deadline(
    mut command: Command,
    deadline: Duration,
    description: &str,
) -> Result<Output> {
    let _serialized = NESTED_CARGO.lock().unwrap_or_else(PoisonError::into_inner);
    let mut child = command
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .with_context(|| format!("spawn {description}"))?;
    let mut stdout_reader = drain(child.stdout.take(), &format!("stdout of {description}"))?;
    let mut stderr_reader = drain(child.stderr.take(), &format!("stderr of {description}"))?;

    let waited = child
        .wait_timeout(deadline)
        .with_context(|| format!("wait for {description}"))?;
    let Some(status) = waited else {
        // Reap the child: a killed process releases its package-cache lock and
        // its compile-admission slot, which a leaked one would hold.
        let _killed = child.kill();
        let _reaped = child.wait();
        // Give the killed child's readers a bounded chance to finish, so the
        // diagnostic includes everything already written to the pipes.
        stdout_reader.finish_after_kill(READER_GRACE);
        stderr_reader.finish_after_kill(READER_GRACE);
        return Err(deadline_failure(
            description,
            deadline,
            &stdout_reader.evidence(),
            &stderr_reader.evidence(),
        ));
    };
    let stdout = stdout_reader.join()?;
    let stderr = stderr_reader.join()?;
    Ok(Output {
        status,
        stdout,
        stderr,
    })
}

/// Build the error for a child that outlived its deadline.
///
/// The retained tails are the substance of the message: without them a
/// deadline failure says only that something took too long, which cannot be
/// told apart from a compile-admission wait, a package-cache wait, or a hang.
fn deadline_failure(
    description: &str,
    deadline: Duration,
    stdout: &StreamEvidence,
    stderr: &StreamEvidence,
) -> anyhow::Error {
    anyhow::anyhow!(
        "{description} exceeded the {} second deadline; the child was killed.\n{}\n{}",
        deadline.as_secs(),
        stdout.describe("stdout"),
        stderr.describe("stderr"),
    )
}

/// The most recent bytes of one child stream, bounded for a diagnostic.
#[derive(Default)]
struct StreamTail {
    /// The retained tail, never longer than [`DIAGNOSTIC_TAIL_BYTES`].
    bytes: VecDeque<u8>,
    /// Every byte the stream produced, retained or not.
    bytes_seen: u64,
}

impl StreamTail {
    /// Retain the end of `chunk`, dropping bytes that fall out of the window.
    fn publish(&mut self, chunk: &[u8]) {
        self.bytes_seen = self
            .bytes_seen
            .saturating_add(u64::try_from(chunk.len()).unwrap_or(u64::MAX));
        self.bytes.extend(chunk.iter().copied());
        let excess = self.bytes.len().saturating_sub(DIAGNOSTIC_TAIL_BYTES);
        if excess > 0 {
            self.bytes.drain(..excess);
        }
    }
}

/// One stream's contribution to a deadline failure.
struct StreamEvidence {
    /// The retained tail bytes.
    bytes: Vec<u8>,
    /// Every byte the stream produced.
    bytes_seen: u64,
}

impl StreamEvidence {
    /// Render the stream, saying when only its tail was retained.
    fn describe(&self, label: &str) -> String {
        let text = String::from_utf8_lossy(&self.bytes);
        let trimmed = text.trim_end();
        let body = if trimmed.is_empty() {
            "(no output)"
        } else {
            trimmed
        };
        let retained = if self.bytes_seen > u64::try_from(self.bytes.len()).unwrap_or(u64::MAX) {
            format!(", last {} bytes retained", self.bytes.len())
        } else {
            String::new()
        };
        format!(
            "{label} ({} bytes seen{retained}):\n{body}",
            self.bytes_seen
        )
    }
}

/// One drained child stream, readable while its reader still runs.
struct StreamCapture {
    /// The stream's name, for messages.
    description: String,
    /// The reader thread, taken when it is joined.
    handle: Option<JoinHandle<io::Result<Vec<u8>>>>,
    /// The bounded tail the reader publishes as it drains.
    tail: Arc<Mutex<StreamTail>>,
}

impl StreamCapture {
    /// Snapshot the tail without waiting for the reader to finish.
    fn evidence(&self) -> StreamEvidence {
        let tail = lock_tail(&self.tail);
        StreamEvidence {
            bytes: tail.bytes.iter().copied().collect(),
            bytes_seen: tail.bytes_seen,
        }
    }

    /// Wait for the reader and return everything it collected.
    fn join(mut self) -> Result<Vec<u8>> {
        let reader = self
            .handle
            .take()
            .with_context(|| format!("collect {}", self.description))?;
        reader
            .join()
            .map_err(|_| anyhow::anyhow!("the {} reader panicked", self.description))?
            .with_context(|| format!("read {}", self.description))
    }

    /// Give the reader a short grace to reach end of file, then leave it.
    ///
    /// A reader that is still running afterwards is detached rather than
    /// joined: a descendant of the killed child is holding the pipe, and the
    /// diagnostic must not wait for that descendant.
    fn finish_after_kill(&mut self, grace: Duration) {
        let Some(reader) = self.handle.as_ref() else {
            return;
        };
        let limit = Instant::now() + grace;
        while !reader.is_finished() && Instant::now() < limit {
            std::thread::sleep(Duration::from_millis(10));
        }
        let is_finished = reader.is_finished();
        if !is_finished {
            return;
        }
        if let Some(finished) = self.handle.take() {
            let _joined = finished.join();
        }
    }
}

/// Lock a stream's tail, tolerating a reader thread that panicked.
fn lock_tail(tail: &Mutex<StreamTail>) -> MutexGuard<'_, StreamTail> {
    tail.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Read `stream` to end of file, publishing a bounded tail as bytes arrive.
fn read_into_tail<R: Read>(stream: &mut R, tail: &Mutex<StreamTail>) -> io::Result<Vec<u8>> {
    let mut collected = Vec::new();
    let mut buffer = [0_u8; 8192];
    loop {
        let read = stream.read(&mut buffer)?;
        if read == 0 {
            return Ok(collected);
        }
        let Some(chunk) = buffer.get(..read) else {
            return Err(io::Error::other(
                "a stream reader received more bytes than its buffer holds",
            ));
        };
        collected.extend_from_slice(chunk);
        lock_tail(tail).publish(chunk);
    }
}

/// Start a thread that drains `source`, publishing its tail as it goes.
fn drain<R: Read + Send + 'static>(source: Option<R>, description: &str) -> Result<StreamCapture> {
    let mut stream = source.with_context(|| format!("capture {description}"))?;
    let tail = Arc::new(Mutex::new(StreamTail::default()));
    let published = Arc::clone(&tail);
    let reader = std::thread::Builder::new()
        .name(description.to_owned())
        .spawn(move || read_into_tail(&mut stream, &published))
        .with_context(|| format!("start the reader for {description}"))?;
    Ok(StreamCapture {
        description: description.to_owned(),
        handle: Some(reader),
        tail,
    })
}
