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
//! test. A timed-out child has to be proven gone, not merely reported as timed
//! out, so each deadline case gives its child a survival marker: a subshell the
//! child holds open writes a file only once it has outlived its budget, the
//! case waits past that point, and the case then asserts no marker was written.
//! A runner that reported the timeout and left the child running would let the
//! marker appear.
//!
//! The write happens in a descendant rather than in the root, so only a
//! group-wide kill prevents it: a runner that killed the root alone would leave
//! the marker unset otherwise, and the case would pass while the descendant
//! lived on.
//!
//! The one detached-writer case leaves a short-lived `sleep` behind, because a
//! writer outside the owned process group is the only way to make a reader
//! outlive the cleanup budget. That case needs `setsid`, which is not POSIX, so
//! it probes for the executable and skips where it is absent (stock macOS); a
//! `cfg(unix)` target does not imply it. The skip writes its reason to standard
//! error, because a silent return would leave the suite reporting a pass for a
//! case that never ran.

use super::{run_with_deadline, run_with_deadline_until};

use camino::Utf8PathBuf;
use cap_std::ambient_authority;
use cap_std::fs_utf8::Dir;
use std::process::Command;
use std::time::{Duration, Instant};

/// Short enough to expire while the child is still asleep.
const SHORT_BUDGET: Duration = Duration::from_millis(150);

/// Long enough that only an explicit refusal, not the clock, fails the case.
const LONG_BUDGET: Duration = Duration::from_secs(30);

/// How long the child would need to outlive its budget and set its marker.
///
/// Comfortably longer than [`SHORT_BUDGET`] and the child's own sleep, so a
/// child that survived to finish its command has certainly written the marker
/// by the time the case looks for it.
const SURVIVAL_WINDOW: Duration = Duration::from_millis(1500);

/// Render an error with its whole context chain.
fn rendered(error: &anyhow::Error) -> String {
    format!("{error:#}")
}

/// A command whose *descendant* records that it outlived its budget.
///
/// The marker is written by a backgrounded subshell, not by the root, and that
/// is the whole point. `sh -c 'sleep 0.5; echo … > "$1"'` forks a `sleep` and
/// writes the marker itself once it returns, so a runner that killed only the
/// root would leave no marker and the case would pass while the descendant
/// lived on. Naming the subshell puts the write outside the root, where only a
/// group-wide kill can prevent it — which is the runner's actual contract.
///
/// `wait` keeps the root alive until the subshell finishes, so the root cannot
/// exit early and let the runner report a clean end-of-file instead of a
/// timeout.
///
/// The path travels as `$1` rather than interpolated text, so a temporary
/// directory containing spaces or quotes needs no escaping.
fn command_surviving_until(marker: &str) -> Command {
    let mut command = Command::new("sh");
    command.args([
        "-c",
        "( sleep 0.5; echo survived > \"$1\" ) & wait",
        "runner",
        marker,
    ]);
    command
}

/// A command that outlives [`SHORT_BUDGET`] and is killed by the cleanup path.
///
/// It writes no marker, so it is only used where the case is about the
/// message rather than the child's fate.
fn sleeping_command() -> Command {
    let mut command = Command::new("sh");
    command.args(["-c", "sleep 30"]);
    command
}

/// A scratch directory the child can set its survival marker in.
///
/// `test_helpers` has no `tempfile` dependency, so the directory sits under the
/// platform temp directory and is named from the test's own label: one
/// directory per case, so concurrent cases cannot read each other's marker. A
/// `cap_std` handle rather than `std::fs` keeps the module inside the
/// workspace's capability policy.
struct Scratch {
    parent: Dir,
    name: String,
    path: Utf8PathBuf,
}

impl Scratch {
    /// Create the scratch directory named `label` under the temp directory.
    fn new(label: &str) -> Result<Self, std::io::Error> {
        // Converted once, at the boundary: `std::env::temp_dir` yields a
        // `PathBuf`, and every `cap_std` call below wants a `Utf8Path`.
        let temp = Utf8PathBuf::from_path_buf(std::env::temp_dir())
            .map_err(|path| std::io::Error::other(format!("not UTF-8: {}", path.display())))?;
        let parent = Dir::open_ambient_dir(&temp, ambient_authority())?;
        let name = format!("ortho-config-{label}-{}", std::process::id());
        // A previous run of this binary may have been killed before its
        // cleanup, so an existing directory holds that run's marker, not this
        // case's; start from empty either way.
        if parent.exists(&name) {
            parent.remove_dir_all(&name)?;
        }
        parent.create_dir(&name)?;
        Ok(Self {
            parent,
            path: temp.join(&name),
            name,
        })
    }

    /// What the child writes to record that it outlived its budget.
    fn marker(&self) -> String {
        self.path.join("survived").into_string()
    }

    /// Whether the child outlived its budget and recorded that.
    fn child_survived(&self) -> bool {
        self.parent.exists(format!("{}/survived", self.name))
    }

    /// Remove the scratch directory, including any marker the child wrote.
    fn remove(&self) -> Result<(), std::io::Error> {
        self.parent.remove_dir_all(&self.name)
    }
}

/// Wait past the point where a surviving child would have set its marker.
///
/// Only the cases that expect a dead child pay this. It is what turns the
/// marker's absence into "the child never got there" rather than "the child had
/// not got there yet".
fn wait_out_the_survival_window() {
    std::thread::sleep(SURVIVAL_WINDOW);
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
#[expect(
    clippy::panic_in_result_fn,
    reason = "Assertions state the child's fate more clearly than errors"
)]
fn a_command_that_exceeds_its_deadline_is_killed_first() -> Result<(), std::io::Error> {
    let scratch = Scratch::new("killed-first")?;

    let error = run_with_deadline(
        command_surviving_until(&scratch.marker()),
        SHORT_BUDGET,
        "kill probe",
    )
    .expect_err("the child outlives its budget");

    wait_out_the_survival_window();
    let survived = scratch.child_survived();
    let outcome = scratch.remove();
    assert!(
        !survived,
        "a child that reported a timeout must be gone, not left to finish: {:#}",
        rendered(&error)
    );
    outcome
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
#[expect(
    clippy::panic_in_result_fn,
    reason = "Assertions state the child's fate more clearly than errors"
)]
fn an_enclosing_sequence_deadline_also_kills_the_command() -> Result<(), std::io::Error> {
    let scratch = Scratch::new("sequence-kill")?;
    let sequence_deadline = Instant::now() + SHORT_BUDGET;

    let error = run_with_deadline_until(
        command_surviving_until(&scratch.marker()),
        LONG_BUDGET,
        sequence_deadline,
        "sequence kill probe",
    )
    .expect_err("the sequence budget is the tighter one");

    wait_out_the_survival_window();
    let survived = scratch.child_survived();
    let outcome = scratch.remove();
    assert!(
        !survived,
        "a command stopped by the sequence budget must be killed too: {:#}",
        rendered(&error)
    );
    outcome
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

/// Say on standard error why the detached-writer case is not running.
///
/// A silent early return leaves `cargo test` reporting a pass for a case that
/// never executed, so a platform without `setsid` would look the same as one
/// where the runner was proven to report unfinished readers. The suite's
/// output is the only place that difference can be seen.
///
/// `print_stderr` is denied workspace-wide because production code should log
/// through `tracing`; this is a test process announcing a skip on the platform
/// that cannot run the case, and there is no subscriber to receive an event.
#[expect(
    clippy::print_stderr,
    reason = "a skipped test has no other way to report that it did not run"
)]
fn report_missing_setsid() {
    eprintln!(
        "SKIP a_writer_outside_the_owned_group_is_reported_as_unfinished_readers: \
         `setsid` is not available on this platform"
    );
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
        report_missing_setsid();
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
