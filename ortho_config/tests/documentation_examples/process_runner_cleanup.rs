//! Bounded cleanup for owned subprocesses and their output readers.

use super::capture::Reader;
use super::diagnostics::{capture_snapshots, format_failure_diagnostic};
use super::{IncompleteCleanup, POLL_INTERVAL, RunContext};
use anyhow::Error;
use process_wrap::std::ChildWrapper;
use std::io;
use std::thread;
use std::time::{Duration, Instant};

pub(super) struct CleanupProcess<'a, Kill> {
    pub(super) child: &'a mut Box<dyn ChildWrapper>,
    pub(super) readers: &'a mut [Reader],
    pub(super) kill: &'a mut Kill,
    pub(super) context: &'a RunContext,
}

#[derive(Default)]
struct CleanupObservation {
    tree_completed: bool,
    wait_error_seen: bool,
    was_incomplete: bool,
    evidence: Vec<String>,
}

pub(super) fn finish_failure<Kill>(primary: Error, mut process: CleanupProcess<'_, Kill>) -> Error
where
    Kill: FnMut(&mut dyn ChildWrapper) -> io::Result<()>,
{
    let cleanup_deadline = Instant::now()
        .checked_add(process.context.policy.cleanup_timeout())
        .unwrap_or_else(Instant::now);
    let mut observation = CleanupObservation::default();
    let kill_result = (process.kill)(process.child.as_mut());
    if let Err(kill_error) = kill_result
        && kill_error.kind() != io::ErrorKind::NotFound
    {
        observation.was_incomplete = true;
        observation
            .evidence
            .push(format!("tree termination failed: {kill_error}"));
    }

    let reader_stop_deadline = cleanup_deadline
        .checked_sub(Duration::from_millis(50))
        .unwrap_or_else(Instant::now);
    while Instant::now() < reader_stop_deadline {
        observe_cleanup_state(&mut process, &mut observation);
        if observation.tree_completed && process.readers.iter().all(Reader::is_finished) {
            break;
        }
        thread::sleep(
            POLL_INTERVAL.min(reader_stop_deadline.saturating_duration_since(Instant::now())),
        );
    }

    for reader in process
        .readers
        .iter()
        .filter(|reader| !reader.is_finished())
    {
        reader.request_stop();
    }
    while Instant::now() < cleanup_deadline {
        observe_cleanup_state(&mut process, &mut observation);
        if observation.tree_completed && process.readers.iter().all(Reader::is_finished) {
            break;
        }
        thread::sleep(
            POLL_INTERVAL.min(cleanup_deadline.saturating_duration_since(Instant::now())),
        );
    }

    if !observation.tree_completed {
        observation.was_incomplete = true;
        observation
            .evidence
            .push("owned process tree did not complete before the cleanup deadline".to_owned());
    }
    collect_reader_outcomes(&mut process, cleanup_deadline, &mut observation);

    let captures = capture_snapshots(process.readers);
    let diagnostic = format_failure_diagnostic(process.context, &captures, &observation.evidence)
        .unwrap_or_else(|_| "subprocess failure diagnostic formatting failed".to_owned());
    let diagnostic_error = primary.context(diagnostic);
    if observation.was_incomplete {
        diagnostic_error.context(IncompleteCleanup)
    } else {
        diagnostic_error
    }
}

fn observe_cleanup_state<Kill>(
    process: &mut CleanupProcess<'_, Kill>,
    observation: &mut CleanupObservation,
) where
    Kill: FnMut(&mut dyn ChildWrapper) -> io::Result<()>,
{
    match process.child.try_wait() {
        Ok(Some(_)) => observation.tree_completed = true,
        Ok(None) => {}
        Err(wait_error) => {
            if !observation.wait_error_seen {
                observation.wait_error_seen = true;
                observation
                    .evidence
                    .push(format!("cleanup wait failed: {wait_error}"));
            }
        }
    }
    for reader in process.readers.iter_mut() {
        reader.refresh();
    }
}

fn collect_reader_outcomes<Kill>(
    process: &mut CleanupProcess<'_, Kill>,
    cleanup_deadline: Instant,
    observation: &mut CleanupObservation,
) where
    Kill: FnMut(&mut dyn ChildWrapper) -> io::Result<()>,
{
    for reader in process.readers.iter_mut() {
        reader.join_until(cleanup_deadline);
    }
    for reader in process
        .readers
        .iter_mut()
        .filter(|reader| !reader.is_finished())
    {
        if reader.detach_if_running() {
            observation.was_incomplete = true;
            observation.evidence.push(format!(
                "{} reader did not finish before the cleanup deadline",
                reader.stream()
            ));
        }
    }
    for reader in process
        .readers
        .iter()
        .filter(|reader| reader.was_cancelled())
    {
        observation.was_incomplete = true;
        observation.evidence.push(format!(
            "{} reader was cancelled before EOF; writer state is unconfirmed",
            reader.stream()
        ));
    }
    observation.evidence.extend(
        process
            .readers
            .iter()
            .filter_map(|reader| reader.error().map(str::to_owned)),
    );
}
