//! Check the runner's refusal, timeout, and cleanup branches with real children.
//!
//! Every workspace caller exercises the success path, so the branches that only
//! run when a command is refused, cannot spawn, overruns its budget, or leaves a
//! writer outside the owned group had no direct coverage. A defect in them
//! leaves a stray child and a blocked pipe rather than a wrong answer, which is
//! why they are forced here instead of trusted.
//!
//! The commands are `sh` one-liners on purpose: the runner is a process
//! supervisor, so a fake child would skip exactly the kernel behaviour under
//! test. The one detached-writer case leaves a short-lived `sleep` behind,
//! because a writer outside the owned process group is the only way to make a
//! reader outlive the cleanup budget.

use super::{run_with_deadline, run_with_deadline_until};

use std::process::Command;
use std::time::{Duration, Instant};

/// Short enough to expire while the child is still asleep.
const SHORT_BUDGET: Duration = Duration::from_millis(150);

/// Long enough that only an explicit refusal, not the clock, fails the case.
const LONG_BUDGET: Duration = Duration::from_secs(30);

/// A command that outlives [`SHORT_BUDGET`] and is killed by the cleanup path.
fn sleeping_command() -> Command {
    let mut command = Command::new("sh");
    command.args(["-c", "sleep 30"]);
    command
}

/// Render an error with its whole context chain.
fn rendered(error: &anyhow::Error) -> String {
    format!("{error:#}")
}

#[test]
fn a_command_that_exceeds_its_deadline_reports_the_budget() {
    let error = run_with_deadline(sleeping_command(), SHORT_BUDGET, "deadline probe")
        .expect_err("the child outlives its budget");

    let message = rendered(&error);
    assert!(
        message.contains("deadline probe exceeded the"),
        "the failure names the command: {message}"
    );
    assert!(
        message.contains("command deadline"),
        "the command's own budget scopes the failure: {message}"
    );
}

#[test]
fn an_enclosing_sequence_deadline_bounds_the_command() {
    let sequence_deadline = Instant::now() + SHORT_BUDGET;

    let error = run_with_deadline_until(
        sleeping_command(),
        LONG_BUDGET,
        sequence_deadline,
        "sequence probe",
    )
    .expect_err("the sequence budget is the tighter one");

    let message = rendered(&error);
    assert!(
        message.contains("shared sequence (remaining budget)"),
        "the tighter budget names its scope: {message}"
    );
}

#[test]
fn an_elapsed_sequence_deadline_refuses_to_start_the_command() {
    let sequence_deadline = Instant::now()
        .checked_sub(SHORT_BUDGET)
        .expect("the test has been running for the short budget");

    let error = run_with_deadline_until(
        sleeping_command(),
        LONG_BUDGET,
        sequence_deadline,
        "elapsed probe",
    )
    .expect_err("a command with no budget left must not start");

    let message = rendered(&error);
    assert!(
        message.contains("could not start before its enclosing sequence deadline"),
        "the refusal names the enclosing deadline: {message}"
    );
}

#[test]
fn a_budget_that_overflows_the_clock_is_refused() {
    let error = run_with_deadline(Command::new("/bin/true"), Duration::MAX, "overflow probe")
        .expect_err("no instant can be that far ahead");

    let message = rendered(&error);
    assert!(
        message.contains("deadline is out of range"),
        "the refusal names the budget: {message}"
    );
}

#[test]
fn a_command_that_cannot_be_spawned_is_reported() {
    let command = Command::new("/nonexistent/ortho-config-probe");

    let error =
        run_with_deadline(command, LONG_BUDGET, "missing probe").expect_err("nothing is there");

    let message = rendered(&error);
    assert!(
        message.contains("spawn missing probe"),
        "the failure names the command it could not start: {message}"
    );
}

#[test]
fn both_streams_are_captured_on_success() {
    let mut command = Command::new("sh");
    command.args(["-c", "printf out; printf err >&2"]);

    let output =
        run_with_deadline(command, LONG_BUDGET, "capture probe").expect("the command succeeds");

    assert_eq!(output.stdout, b"out");
    assert_eq!(output.stderr, b"err");
    assert!(output.status.success());
}

/// Whether `setsid` can be run, which the detached-writer case depends on.
///
/// `setsid` is not POSIX, so a `cfg(unix)` target does not imply it exists.
/// Stock macOS has no `setsid` at all, and running the case there would fail
/// with an unfinished-reader assertion that says nothing about the runner.
fn has_setsid() -> bool {
    Command::new("setsid")
        .arg("true")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .is_ok_and(|status| status.success())
}

#[test]
fn a_writer_outside_the_owned_group_is_reported_as_unfinished_readers() {
    // `setsid` puts the writer in a new session, so the group kill that cleans
    // up the root cannot reach it and the pipe stays open past the cleanup
    // budget. That is the one case where the runner has to report readers it
    // could not join rather than data it could not read.
    //
    // The root lingers briefly so the writer has certainly left the root's
    // session before the root exits. Without that settle time the writer is
    // occasionally still in the root's group when the runner reaps it, and the
    // case then sees a clean end-of-file instead of unfinished readers.
    if !has_setsid() {
        return;
    }
    let mut command = Command::new("sh");
    command.args(["-c", "setsid sleep 4 & sleep 0.2"]);

    let error = run_with_deadline(command, LONG_BUDGET, "detached writer probe")
        .expect_err("the reader cannot reach EOF while the writer lives");

    let message = rendered(&error);
    assert!(
        message.contains("output readers exceeded their cleanup deadline"),
        "the failure names the unfinished readers: {message}"
    );
}
