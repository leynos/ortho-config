//! End-to-end tests for the lock-holder report.
//!
//! The parser and the device split are pinned in `artefact_lock_evidence_tests`
//! on every platform. What is asserted here is the report a *real* hold
//! produces, because that report is the deliverable: issue #606's failures could
//! say only that a released lock looked held, and a report that is well-formed
//! but names nobody would leave the defect exactly as undiagnosable as before.
//!
//! The tests that take the lock for real exist only where the lock itself does
//! — on Linux, where the probe's read-only handle is documented to acquire the
//! `flock`-backed exclusive lock without complaint. `accepts_a_lock_that_is_free`
//! is deliberately ungated: it asserts only that a free lock passes, which every
//! platform can answer, and it keeps the module compiling where the gated tests
//! are removed.

use anyhow::{Context, Result};

/// Verifies a process is named by its command, parent, and relation.
#[cfg(target_os = "linux")]
#[test]
fn names_the_reporting_process_as_such() {
    let own_pid = std::process::id();
    let identity = super::lock_diagnostics::process_identity(own_pid)
        .expect("the running process must have a readable /proc/self/stat");
    assert!(
        !identity.command.is_empty(),
        "the command name must be recorded: {identity:?}",
    );
    let described = super::lock_diagnostics::describe_process(own_pid, own_pid);
    assert!(
        described.contains("this process"),
        "a hold named after this process must say so: {described}",
    );
}

/// Verifies a genuine hold is reported with the holder's own identity.
///
/// This is the whole point of the module: issue #606's failures could say only
/// that a released lock looked held. A report that names this process, the file,
/// and the descriptor it still holds turns an unactionable boolean into the one
/// fact needed to tell a leaked handle from a foreign holder. The assertion is
/// on the *presence* of those facts, not on exact wording, so the report can be
/// reworded without silently losing its evidence.
#[cfg(target_os = "linux")]
#[test]
fn reports_the_process_and_descriptor_that_still_hold_the_lock() -> Result<()> {
    // Imported inside this Linux-gated test rather than at module scope: the
    // module still compiles on every target, where the gate removes the only
    // caller and a module-level import would be an unused one.
    use super::artefact_lock::{create_lock, lock};
    use super::fragment_tests::TempRoot;
    use anyhow::ensure;

    let root = TempRoot::new()?;
    create_lock(root.path()).context("create the lock file for the holder")?;
    let held = lock(root.path())?;

    let report =
        super::lock_diagnostics::ensure_released(root.path(), "the guard must release the lock")
            .expect_err("a lock this process still holds must be reported as held");
    drop(held);

    let rendered = format!("{report:#}");
    ensure!(
        rendered.starts_with("the guard must release the lock"),
        "the caller's expectation must lead the report: {rendered}",
    );
    for expected in [
        "is still held",
        "this process",
        "holds the lock file open as fd",
        "re-probe after",
    ] {
        ensure!(
            rendered.contains(expected),
            "the report must contain {expected:?}, but it did not: {rendered}",
        );
    }
    Ok(())
}

/// Verifies a free lock passes without building a report at all.
///
/// The success path pays for a probe and nothing else: no `/proc` scan runs
/// when there is nothing to explain.
#[test]
fn accepts_a_lock_that_is_free() -> Result<()> {
    use super::artefact_lock::create_lock;
    use super::fragment_tests::TempRoot;

    let root = TempRoot::new()?;
    create_lock(root.path()).context("create the lock file for the probe")?;
    super::lock_diagnostics::ensure_released(root.path(), "an unheld lock must be reported free")?;
    Ok(())
}
