//! Reader cancellation regressions with a pipe-ready child.

use super::super::{WaitProcess, capture::Reader, wait_for_exit};
use anyhow::{Context, Result, anyhow, ensure};
use std::process::{Child, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};

#[test]
fn reader_cancellation_finishes_before_a_pipe_holder_exits() -> Result<()> {
    let mut command = super::probe_command("probes::reader_cancellation_ready_probe")?;
    command.stdout(Stdio::piped()).stderr(Stdio::null());
    let mut writer = command
        .spawn()
        .context("start the controlled pipe writer")?;
    let mut reader = None;
    let (ready_sender, ready_receiver) = mpsc::sync_channel(1);
    let primary = (|| -> Result<()> {
        let stdout = writer
            .stdout
            .take()
            .context("take the controlled writer stdout")?;
        reader = Some(Reader::spawn("stdout", stdout, 64, move |chunk| {
            if chunk.contains(&b'!') {
                signal_readiness(&ready_sender);
            }
        })?);
        ready_receiver
            .recv_timeout(Duration::from_secs(2))
            .context("wait for the child readiness byte")?;

        let active_reader = reader
            .as_mut()
            .ok_or_else(|| anyhow!("reader was not started"))?;
        active_reader.request_stop();
        active_reader.join_until(Instant::now() + Duration::from_millis(200));
        ensure!(
            active_reader.is_finished(),
            "reader did not stop before its bound"
        );
        ensure!(
            active_reader.was_cancelled(),
            "reader did not report cancellation"
        );
        let writer_status = writer.try_wait().context("poll the controlled writer")?;
        ensure!(
            writer_status.is_none(),
            "controlled pipe writer exited early: {writer_status:?}"
        );
        Ok(())
    })();
    let cleanup = cleanup_writer(&mut writer, reader.as_mut());
    preserve_primary(primary, cleanup)
}

fn signal_readiness(sender: &mpsc::SyncSender<()>) {
    if let Err(mpsc::TrySendError::Disconnected(())) = sender.try_send(()) {
        tracing::debug!("readiness receiver exited before the child marker");
    }
}

fn cleanup_writer(writer: &mut Child, reader: Option<&mut Reader>) -> Result<()> {
    let cleanup_timeout = Duration::from_secs(1);
    let cleanup_started = Instant::now();
    let cleanup_deadline = cleanup_started
        .checked_add(cleanup_timeout)
        .unwrap_or(cleanup_started);
    let reader_deadline = cleanup_started
        .checked_add(Duration::from_millis(200))
        .map_or(cleanup_deadline, |deadline| deadline.min(cleanup_deadline));
    let mut cleanup_errors = Vec::new();
    if let Some(active_reader) = reader {
        active_reader.request_stop();
        active_reader.join_until(reader_deadline);
        if !active_reader.is_finished() {
            let _ = active_reader.detach_if_running();
            cleanup_errors.push("stdout reader did not finish before cleanup deadline".to_owned());
        }
        if let Some(reader_error) = active_reader.error() {
            cleanup_errors.push(format!("stdout reader cleanup failed: {reader_error}"));
        }
    }

    let kill_result = writer.kill();
    let mut readers = Vec::new();
    let wait_result = wait_for_exit(
        WaitProcess {
            child: writer,
            readers: &mut readers,
            operation: "cleanup reader-cancellation probe",
            deadline: cleanup_deadline,
            timeout: cleanup_timeout,
            status: None,
        },
        process_wrap::std::ChildWrapper::try_wait,
    );
    let wait_error = wait_result.err().map(|error| format!("{error:#}"));
    if let Some(wait_diagnostic) = &wait_error {
        cleanup_errors.push(format!("writer was not reaped: {wait_diagnostic}"));
    }
    if let Err(kill_error) = kill_result
        && should_report_kill_error(&kill_error, wait_error.is_some())
    {
        cleanup_errors.push(format!("writer termination failed: {kill_error}"));
    }
    if cleanup_errors.is_empty() {
        Ok(())
    } else {
        Err(anyhow!(
            "bounded writer cleanup failed: {}",
            cleanup_errors.join("; ")
        ))
    }
}

fn should_report_kill_error(kill_error: &std::io::Error, writer_was_not_reaped: bool) -> bool {
    kill_error.kind() != std::io::ErrorKind::NotFound || writer_was_not_reaped
}

fn preserve_primary(primary: Result<()>, cleanup: Result<()>) -> Result<()> {
    match (primary, cleanup) {
        (outcome, Ok(())) => outcome,
        (Ok(()), Err(cleanup_error)) => Err(cleanup_error),
        (Err(primary_error), Err(cleanup_error)) => {
            Err(primary_error.context(format!("cleanup also failed: {cleanup_error:#}")))
        }
    }
}
