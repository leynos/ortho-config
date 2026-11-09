//! Draining a child's streams, with a bounded tail kept for diagnostics.
//!
//! A stream is read on its own thread while the child runs, so the child can
//! never block on a full pipe, and the bytes already read survive a deadline.
//! Each reader also publishes the end of its stream while it drains: on a
//! shared host the queue and heartbeat lines a waiting Cargo child prints are
//! the only evidence that distinguishes a compile-admission wait from a hang.
//!
//! The retention is deliberately asymmetric. Callers parse the head of a
//! stream (`cargo metadata` JSON, for instance), so the head must be complete.
//! A diagnostic needs the end, where the last queue line names the wait that
//! was in force, so a separate bounded tail is retained beside it.
//!
//! Collection is bounded in both outcomes. After a kill the tree's copies of
//! the pipes close as its processes die, but a descendant that survived the
//! kill — or one that outlived a normal parent exit — can hold one open, and
//! waiting for that descendant is the same hang by another route.

use super::POLL_INTERVAL;
use anyhow::{Context, Result, anyhow};
use std::collections::VecDeque;
use std::io::{self, Read};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

/// Bytes kept from the end of each child stream to explain a deadline failure.
///
/// The tail, not the head, is the part that says what the child was doing when
/// the deadline expired: compile-admission queue lines and the rustc heartbeat
/// are printed repeatedly, and the last one names the wait that was in force.
pub(super) const DIAGNOSTIC_TAIL_BYTES: usize = 64 * 1024;

/// How long a stream reader may keep draining after the child is gone.
///
/// After a kill, the tree's copies of the pipes close as its processes die. A
/// descendant that survived the kill, or one that outlived a normal parent
/// exit, can still hold one open. The suite has to report what it captured
/// instead of blocking on that descendant, so a reader which has not reached
/// end of file by then is detached and its retained tail is reported.
pub(super) const READER_GRACE: Duration = Duration::from_secs(2);

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

/// One stream's contribution to a failure diagnostic.
pub(super) struct StreamEvidence {
    /// The retained tail bytes.
    bytes: Vec<u8>,
    /// Every byte the stream produced.
    bytes_seen: u64,
}

impl StreamEvidence {
    /// Render the stream, saying when only its tail was retained.
    pub(super) fn describe(&self, label: &str) -> String {
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
pub(super) struct StreamCapture {
    /// The stream's name, for thread names and messages.
    stream: String,
    /// The operation the stream belongs to, for messages.
    operation: String,
    /// The reader thread, taken when it is joined.
    handle: Option<JoinHandle<io::Result<Vec<u8>>>>,
    /// The bounded tail the reader publishes as it drains.
    tail: Arc<Mutex<StreamTail>>,
}

impl StreamCapture {
    /// Snapshot the tail without waiting for the reader to finish.
    pub(super) fn evidence(&self) -> StreamEvidence {
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
            .with_context(|| format!("collect {}", self.stream))?;
        reader
            .join()
            .map_err(|_| anyhow!("the {} reader panicked", self.stream))?
            .with_context(|| format!("read {}", self.stream))
    }

    /// Collect a stream after the child exited, refusing to wait unboundedly.
    ///
    /// A descendant can outlive its parent while holding the inherited pipe.
    /// Joining the reader in that state is the same hang the deadline exists to
    /// catch, so the reader gets a bounded grace and is then detached. The
    /// error carries the retained tail: the stream is not merely slow, a
    /// process the child started is still writing to it.
    pub(super) fn finish_after_exit(self, grace: Duration) -> Result<Vec<u8>> {
        if !self.wait_for_reader(grace) {
            let evidence = self.evidence();
            return Err(anyhow!(
                "the {} pipe remained open after {} exited; a descendant still holds it.\n{}",
                self.stream,
                self.operation,
                evidence.describe("retained output"),
            ));
        }
        self.join()
    }

    /// Give the reader a short grace to reach end of file, then leave it.
    ///
    /// A reader that is still running afterwards is detached rather than
    /// joined: a descendant of the child is holding the pipe, and the
    /// diagnostic must not wait for that descendant.
    pub(super) fn detach_after_kill(&mut self, grace: Duration) {
        if self.wait_for_reader(grace)
            && let Some(finished) = self.handle.take()
        {
            let _joined = finished.join();
        }
    }

    /// Wait up to `grace` for the reader, reporting whether it finished.
    fn wait_for_reader(&self, grace: Duration) -> bool {
        let Some(reader) = self.handle.as_ref() else {
            return true;
        };
        let limit = Instant::now() + grace;
        while !reader.is_finished() && Instant::now() < limit {
            std::thread::sleep(POLL_INTERVAL);
        }
        reader.is_finished()
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
pub(super) fn drain<R: Read + Send + 'static>(
    source: Option<R>,
    stream: &str,
    operation: &str,
) -> Result<StreamCapture> {
    let description = format!("{stream} of {operation}");
    let mut stream_source = source.with_context(|| format!("capture {description}"))?;
    let tail = Arc::new(Mutex::new(StreamTail::default()));
    let published = Arc::clone(&tail);
    let reader = std::thread::Builder::new()
        .name(description.clone())
        .spawn(move || read_into_tail(&mut stream_source, &published))
        .with_context(|| format!("start the reader for {description}"))?;
    Ok(StreamCapture {
        stream: stream.to_owned(),
        operation: operation.to_owned(),
        handle: Some(reader),
        tail,
    })
}
