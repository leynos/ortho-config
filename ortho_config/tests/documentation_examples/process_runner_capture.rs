//! Bounded, continuously drained stdout and stderr capture for child processes.

use super::pipe::{Pipe, ReadProgress};
use std::io;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

#[cfg(test)]
#[path = "process_runner_capture_property_tests.rs"]
mod property_tests;

const PIPE_POLL_INTERVAL: Duration = Duration::from_millis(10);

#[derive(Clone, Debug)]
pub(crate) struct Snapshot {
    pub(super) bytes: Vec<u8>,
    pub(super) bytes_seen: u64,
    pub(super) truncated: bool,
}

#[derive(Default)]
pub(super) struct Progress {
    bytes: Vec<u8>,
    bytes_seen: u64,
    truncated: bool,
}

impl Progress {
    pub(super) fn append(&mut self, bytes: &[u8], limit: usize) {
        self.bytes_seen = self.bytes_seen.saturating_add(bytes.len() as u64);
        let remaining = limit.saturating_sub(self.bytes.len());
        self.bytes.extend(bytes.iter().take(remaining).copied());
        self.truncated |= remaining < bytes.len();
    }

    pub(super) fn snapshot(&self) -> Snapshot {
        Snapshot {
            bytes: self.bytes.clone(),
            bytes_seen: self.bytes_seen,
            truncated: self.truncated,
        }
    }
}

struct DrainState<Observer> {
    pipe: Pipe,
    progress: Arc<Mutex<Progress>>,
    cancellation: Arc<AtomicBool>,
    output_limit: usize,
    observer: Observer,
    observer_panicked: bool,
}

impl<Observer> DrainState<Observer>
where
    Observer: FnMut(&[u8]),
{
    fn read_next(&mut self, buffer: &mut [u8]) -> io::Result<Option<ReaderEnd>> {
        let read_progress = self.pipe.read(buffer).map_err(|error| {
            io::Error::new(error.kind(), format!("read subprocess output: {error}"))
        })?;
        match read_progress {
            ReadProgress::Bytes(bytes_read) => {
                self.record_bytes(buffer, bytes_read)?;
                Ok(None)
            }
            ReadProgress::Pending => {
                thread::sleep(PIPE_POLL_INTERVAL);
                Ok(None)
            }
            ReadProgress::End => self.finish(ReaderEnd::Eof).map(Some),
        }
    }

    fn record_bytes(&mut self, buffer: &[u8], bytes_read: usize) -> io::Result<()> {
        let bytes = buffer
            .get(..bytes_read)
            .ok_or_else(|| io::Error::other("pipe returned more bytes than the read buffer"))?;
        lock_progress(&self.progress).append(bytes, self.output_limit);
        if !self.observer_panicked {
            let observer_result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                (self.observer)(bytes);
            }));
            self.observer_panicked = observer_result.is_err();
        }
        Ok(())
    }

    fn finish(&self, completion: ReaderEnd) -> io::Result<ReaderEnd> {
        if self.observer_panicked {
            Err(io::Error::other("stdout observer panicked"))
        } else {
            Ok(completion)
        }
    }
}

fn drain_pipe<Observer>(mut state: DrainState<Observer>) -> io::Result<ReaderEnd>
where
    Observer: FnMut(&[u8]),
{
    let mut buffer = [0_u8; 8192];
    loop {
        if state.cancellation.load(Ordering::Acquire) {
            return state.finish(ReaderEnd::Cancelled);
        }
        if let Some(completion) = state.read_next(&mut buffer)? {
            return Ok(completion);
        }
    }
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum ReaderEnd {
    Eof,
    Cancelled,
}

/// Owns a cancellable drain thread and exposes partial capture while it runs.
pub(crate) struct Reader {
    stream: &'static str,
    progress: Arc<Mutex<Progress>>,
    cancellation: Arc<AtomicBool>,
    join: Option<JoinHandle<io::Result<ReaderEnd>>>,
    completion: Option<ReaderEnd>,
    error: Option<String>,
}

impl Reader {
    pub(crate) fn spawn<PipeHandle, Observer>(
        stream: &'static str,
        pipe_handle: PipeHandle,
        output_limit: usize,
        observer: Observer,
    ) -> io::Result<Self>
    where
        PipeHandle: super::pipe::Input,
        Observer: FnMut(&[u8]) + Send + 'static,
    {
        #[cfg(unix)]
        let pipe = Pipe::new(pipe_handle)?;
        #[cfg(windows)]
        let pipe = Pipe::new(pipe_handle);
        let progress = Arc::new(Mutex::new(Progress::default()));
        let cancellation = Arc::new(AtomicBool::new(false));
        let drain_state = DrainState {
            pipe,
            progress: Arc::clone(&progress),
            cancellation: Arc::clone(&cancellation),
            output_limit,
            observer,
            observer_panicked: false,
        };
        let join = thread::Builder::new()
            .name(format!("documentation-{stream}-reader"))
            .spawn(move || drain_pipe(drain_state))?;

        Ok(Self {
            stream,
            progress,
            cancellation,
            join: Some(join),
            completion: None,
            error: None,
        })
    }

    pub(crate) fn refresh(&mut self) {
        let Some(active_join) = self.join.as_ref() else {
            return;
        };
        if !active_join.is_finished() {
            return;
        }
        let Some(completed_join) = self.join.take() else {
            return;
        };
        match completed_join.join() {
            Ok(Ok(completion)) => self.completion = Some(completion),
            Ok(Err(error)) => self.error = Some(format!("{} reader failed: {error}", self.stream)),
            Err(_) => self.error = Some(format!("{} reader thread panicked", self.stream)),
        }
    }

    pub(crate) const fn is_finished(&self) -> bool {
        self.join.is_none()
    }

    pub(crate) const fn stream(&self) -> &'static str {
        self.stream
    }

    pub(crate) fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }

    pub(crate) fn was_cancelled(&self) -> bool {
        self.completion == Some(ReaderEnd::Cancelled)
    }

    pub(crate) fn snapshot(&self) -> Snapshot {
        lock_progress(&self.progress).snapshot()
    }

    pub(crate) fn request_stop(&self) {
        self.cancellation.store(true, Ordering::Release);
    }

    pub(crate) fn join_until(&mut self, deadline: Instant) {
        while !self.is_finished() && Instant::now() < deadline {
            self.refresh();
            if !self.is_finished() {
                thread::sleep(
                    PIPE_POLL_INTERVAL.min(deadline.saturating_duration_since(Instant::now())),
                );
            }
        }
        self.refresh();
    }

    pub(crate) fn detach_if_running(&mut self) -> bool {
        if self.join.as_ref().is_some_and(JoinHandle::is_finished) {
            self.refresh();
            return false;
        }
        self.join.take().is_some()
    }
}

fn lock_progress(progress: &Mutex<Progress>) -> MutexGuard<'_, Progress> {
    progress
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}
