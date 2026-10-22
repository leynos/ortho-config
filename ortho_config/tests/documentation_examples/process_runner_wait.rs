//! Poll an owned subprocess while draining its captured output before deadline.

use super::POLL_INTERVAL;
use super::capture::Reader;
use anyhow::{Context, Result, anyhow};
use process_wrap::std::ChildWrapper;
use std::fmt;
use std::io;
use std::process::ExitStatus;
use std::thread;
use std::time::{Duration, Instant};

pub(crate) struct WaitProcess<'a> {
    pub(crate) child: &'a mut dyn ChildWrapper,
    pub(crate) readers: &'a mut [Reader],
    pub(crate) operation: &'a str,
    pub(crate) deadline: Instant,
    pub(crate) timeout: Duration,
    pub(crate) status: Option<ExitStatus>,
}

#[derive(Debug)]
pub(super) struct DeadlineExceeded {
    operation: String,
    reason: &'static str,
    timeout: Duration,
}

impl fmt::Display for DeadlineExceeded {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{}: {} after {}ms",
            self.operation,
            self.reason,
            self.timeout.as_millis()
        )
    }
}

impl std::error::Error for DeadlineExceeded {}

pub(crate) fn wait_for_exit<Poll>(
    mut process: WaitProcess<'_>,
    mut poll: Poll,
) -> Result<ExitStatus>
where
    Poll: FnMut(&mut dyn ChildWrapper) -> io::Result<Option<ExitStatus>>,
{
    loop {
        refresh_output_readers(&mut process)?;
        ensure_before_deadline(&process)?;
        poll_running_process(&mut process, &mut poll)?;
        ensure_before_deadline(&process)?;
        if let Some(status) = completed_process_status(&process) {
            return Ok(status);
        }
        sleep_until_next_poll(&process);
    }
}

fn refresh_output_readers(process: &mut WaitProcess<'_>) -> Result<()> {
    for reader in process.readers.iter_mut() {
        reader.refresh();
    }
    if let Some(reader_error) = process.readers.iter().find_map(|reader| reader.error()) {
        Err(anyhow!("{}: {reader_error}", process.operation))
    } else {
        Ok(())
    }
}

fn ensure_before_deadline(process: &WaitProcess<'_>) -> Result<()> {
    if Instant::now() >= process.deadline {
        Err(expired_operation_error(process))
    } else {
        Ok(())
    }
}

fn poll_running_process<Poll>(process: &mut WaitProcess<'_>, poll: &mut Poll) -> Result<()>
where
    Poll: FnMut(&mut dyn ChildWrapper) -> io::Result<Option<ExitStatus>>,
{
    if process.status.is_none() {
        process.status = poll(process.child)
            .with_context(|| format!("{}: wait for subprocess", process.operation))?;
    }
    Ok(())
}

fn completed_process_status(process: &WaitProcess<'_>) -> Option<ExitStatus> {
    process
        .status
        .filter(|_| process.readers.iter().all(Reader::is_finished))
}

fn sleep_until_next_poll(process: &WaitProcess<'_>) {
    thread::sleep(POLL_INTERVAL.min(process.deadline.saturating_duration_since(Instant::now())));
}

fn expired_operation_error(process: &WaitProcess<'_>) -> anyhow::Error {
    let reason = match (
        process.status.is_some(),
        process.readers.iter().all(Reader::is_finished),
    ) {
        (true, true) => "subprocess completed after operation deadline",
        (true, false) => "captured output remained open",
        (false, _) => "subprocess timed out",
    };
    DeadlineExceeded {
        operation: process.operation.to_owned(),
        reason,
        timeout: process.timeout,
    }
    .into()
}
