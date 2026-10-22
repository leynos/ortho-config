//! Regressions for operation deadlines around child polling.

use super::super::{WaitProcess, deadline_exceeded, spawn_owned, wait_for_exit};
use anyhow::{Context, Result, ensure};
use process_wrap::std::ChildWrapper;
use std::process::{ExitStatus, Stdio};
use std::thread;
use std::time::{Duration, Instant};

#[test]
fn expired_cached_completion_times_out_without_polling() -> Result<()> {
    let (mut child, status) = completed_probe_child()?;
    let timeout = Duration::from_millis(100);
    let deadline = Instant::now()
        .checked_sub(Duration::from_millis(1))
        .context("construct expired deadline")?;
    let mut readers = Vec::new();
    let mut poll_called = false;
    let error = wait_for_exit(
        WaitProcess {
            child: child.as_mut(),
            readers: &mut readers,
            operation: "reject expired cached completion",
            deadline,
            timeout,
            status: Some(status),
        },
        |active_child| {
            poll_called = true;
            active_child.try_wait()
        },
    )
    .expect_err("cached completion after expiry should time out");
    let diagnostic = format!("{error:#}");

    ensure!(deadline_exceeded(&error), "deadline error lost its type");
    ensure!(!poll_called, "expired operation polled the child");
    ensure!(
        diagnostic.contains("subprocess completed after operation deadline"),
        "late-completion reason missing: {diagnostic}"
    );
    Ok(())
}

#[test]
fn completion_returned_after_poll_deadline_times_out() -> Result<()> {
    let (mut child, status) = completed_probe_child()?;
    let timeout = Duration::from_millis(100);
    let deadline = Instant::now()
        .checked_add(timeout)
        .context("construct operation deadline")?;
    let mut readers = Vec::new();
    let mut poll_returned_late = false;
    let error = wait_for_exit(
        WaitProcess {
            child: child.as_mut(),
            readers: &mut readers,
            operation: "reject late poll completion",
            deadline,
            timeout,
            status: None,
        },
        |_child| {
            while Instant::now() <= deadline {
                thread::sleep(Duration::from_millis(1));
            }
            poll_returned_late = true;
            Ok(Some(status))
        },
    )
    .expect_err("poll completion after expiry should time out");
    let diagnostic = format!("{error:#}");

    ensure!(deadline_exceeded(&error), "deadline error lost its type");
    ensure!(
        poll_returned_late,
        "injected poll did not cross the deadline"
    );
    ensure!(
        diagnostic.contains("subprocess completed after operation deadline"),
        "late-completion reason missing: {diagnostic}"
    );
    Ok(())
}

fn completed_probe_child() -> Result<(Box<dyn ChildWrapper>, ExitStatus)> {
    let mut command = super::probe_command("probes::bounded_success_output_probe")?;
    command
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    let mut child = spawn_owned(command)?;
    let timeout = Duration::from_secs(5);
    let started = Instant::now();
    let deadline = started
        .checked_add(timeout)
        .context("construct completed-probe deadline")?;
    let mut readers = Vec::new();
    let status = wait_for_exit(
        WaitProcess {
            child: child.as_mut(),
            readers: &mut readers,
            operation: "complete owned deadline probe",
            deadline,
            timeout,
            status: None,
        },
        process_wrap::std::ChildWrapper::try_wait,
    )?;
    ensure!(
        status.success(),
        "controlled completion probe failed: {status}"
    );
    Ok((child, status))
}
