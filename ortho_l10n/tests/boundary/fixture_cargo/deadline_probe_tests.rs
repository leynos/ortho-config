//! Regression tests for the deadline diagnostic in [`super::run_with_deadline`].
//!
//! The defect this guards against was silence. Six packaged boundary contracts
//! reported only "exceeded the 240 second deadline" while the nested Cargo child
//! had printed its compile-admission queue in front of it, so the retained
//! evidence could not distinguish a queue wait from a hang. Asserting that the
//! timeout message carries the child's own output needs a real timed-out child,
//! and the only portable child this binary can rely on is itself: the ignored
//! probe below prints to both streams and then outlives the deadline the test
//! gives it.

use super::run_with_deadline;
use anyhow::{Context, Result, ensure};
use std::io::Write as _;
use std::process::Command;
use std::time::Duration;

/// Deadline given to the probe.
///
/// Long enough that a loaded runner can start the child and let it print,
/// short enough that the regression test does not add a meaningful delay to a
/// suite whose other members take minutes.
const PROBE_DEADLINE: Duration = Duration::from_secs(5);

/// How long the probe sleeps after printing, so it always outlives the deadline.
const PROBE_LIFETIME: Duration = Duration::from_secs(30);

/// Print to both streams, then outlive the deadline under test.
///
/// Ignored so an ordinary run never waits for it; the test below selects it by
/// name when it re-runs this binary as its own child. The writes use `std::io`
/// rather than `println!` because the workspace denies `clippy::print_stdout`
/// and `clippy::print_stderr` in test targets too.
#[test]
#[ignore = "driven as a child process by deadline_diagnostic_reports_the_child"]
fn deadline_probe() -> Result<()> {
    let mut stdout = std::io::stdout().lock();
    writeln!(stdout, "probe-stdout-before-deadline")?;
    stdout.flush()?;
    let mut stderr = std::io::stderr().lock();
    writeln!(stderr, "probe-stderr-before-deadline")?;
    stderr.flush()?;
    std::thread::sleep(PROBE_LIFETIME);
    Ok(())
}

/// A timed-out child's own output must appear in the error.
#[test]
fn deadline_diagnostic_reports_the_child() -> Result<()> {
    let error = run_with_deadline(probe_command()?, PROBE_DEADLINE, "deadline probe")
        .err()
        .context("a probe that sleeps past its deadline should time out")?;
    let message = format!("{error:#}");
    for expected in [
        "deadline probe exceeded",
        "probe-stdout-before-deadline",
        "probe-stderr-before-deadline",
    ] {
        ensure!(
            message.contains(expected),
            "the deadline error omits {expected:?}:\n{message}",
        );
    }
    Ok(())
}

/// Build the command that re-runs this binary as the timed-out probe.
fn probe_command() -> Result<Command> {
    let executable = std::env::current_exe().context("the test binary should have a path")?;
    let mut command = Command::new(executable);
    // `--nocapture` matters: without it libtest buffers each test's output and
    // the child would print nothing to the pipes this test is asserting about.
    command.args([
        "--ignored",
        "--quiet",
        "--nocapture",
        "--test-threads=1",
        "deadline_probe",
    ]);
    Ok(command)
}
