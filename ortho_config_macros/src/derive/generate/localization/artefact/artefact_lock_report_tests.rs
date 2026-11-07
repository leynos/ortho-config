//! End-to-end tests for the lock-holder report.
//!
//! The parser and the device split are pinned in `artefact_lock_evidence_tests`
//! on every platform. What is asserted here is the report a *real* hold
//! produces, because that report is the deliverable: issue #606's failures could
//! say only that a released lock looked held, and a report that is well-formed
//! but names nobody would leave the defect exactly as undiagnosable as before.
//!
//! The tests that inspect a real hold exist only where a real hold can be
//! observed — on Linux, where the probe's read-only handle is documented to
//! acquire the `flock`-backed exclusive lock without complaint.
//! `accepts_a_lock_that_is_free` is deliberately ungated: it asserts only that
//! a free lock passes, which every platform can answer, and it keeps the module
//! compiling where the gated tests are removed.
//!
//! Every test here that holds the lock does so through a helper *process*,
//! never with an in-process guard. `flock` belongs to the open file
//! description, and the suites in this binary fork helper processes; each fork
//! duplicates the whole descriptor table, so an in-process guard would keep the
//! lock alive in the child until its `exec`. That is the interference issue #606
//! records, and holding the lock in a helper's own table is what keeps these
//! tests measuring the settle loop rather than the scheduler.

use anyhow::{Context, Result};

#[cfg(target_os = "linux")]
use super::lock_evidence::ProcessId;

/// Verifies a process is named by its command, parent, and relation.
#[cfg(target_os = "linux")]
#[test]
fn names_the_reporting_process_as_such() {
    let own_pid = ProcessId::new(std::process::id());
    let identity = super::lock_process::process_identity(own_pid)
        .expect("the running process must have a readable /proc/self/stat");
    assert!(
        !identity.command.is_empty(),
        "the command name must be recorded: {identity:?}",
    );
    let described = super::lock_process::describe_process(own_pid, own_pid);
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

/// Verifies a probe that cannot be taken is reported, not read as "free".
///
/// The probe opens the existing lock file rather than creating one, so a
/// missing file is an I/O error and not a free lock. Both assertions read the
/// same `NotFound` the kernel returns, so neither platform detail nor a lock
/// held by anyone is involved: the only thing under test is that the
/// diagnostics propagate the failure with the caller's expectation on it,
/// rather than folding it into the report or answering `Ok`.
///
/// Folding would be the tempting mistake — the report already swallows
/// diagnostic faults so a failure to *explain* a hold cannot mask the hold —
/// but a probe that never ran has not observed anything, and saying "free"
/// there would hide a missing file behind a reassuring answer.
#[test]
fn settles_report_a_probe_error_instead_of_a_free_answer() -> Result<()> {
    use super::fragment_tests::TempRoot;
    use anyhow::ensure;

    let root = TempRoot::new()?;
    // Deliberately no `create_lock`: the file's absence is what the probe
    // reports, and creating it would make the probe succeed.
    ensure!(
        !super::artefact_lock::lock_path(root.path()).exists(),
        "the lock file must be absent for this case to test a probe failure",
    );

    let settle = super::lock_diagnostics::ensure_settles_free(root.path(), "a missing lock file")
        .expect_err("a probe that cannot open the lock file must not answer `Ok`");
    let immediate = super::lock_diagnostics::ensure_released(root.path(), "a missing lock file")
        .expect_err("the strict assertion must report the same probe failure");

    for (label, error) in [("settling", &settle), ("strict", &immediate)] {
        let rendered = format!("{error:#}");
        ensure!(
            rendered.starts_with("a missing lock file"),
            "the {label} failure must lead with the caller's expectation: {rendered}",
        );
        // Checked through the chain rather than the rendered text: the `Display`
        // for an `ErrorKind` is not the `Display` of the `io::Error` carrying it,
        // and matching on platform error prose would fail on another OS.
        ensure!(
            error.chain().any(|cause| cause
                .downcast_ref::<std::io::Error>()
                .is_some_and(|io| io.kind() == std::io::ErrorKind::NotFound)),
            "the {label} failure must carry the probe's own I/O error, not a report \
             built from evidence it never gathered: {rendered}",
        );
        ensure!(
            !rendered.contains("is still held"),
            "a probe that never ran must not claim the lock was held: {rendered}",
        );
    }
    Ok(())
}

/// Verifies a lock released part-way through the window is accepted.
///
/// The holder is a *separate process*, and that is not incidental. `flock`
/// belongs to the open file description, and the suites in this binary spawn
/// helper processes; each spawn forks this process's whole descriptor table, so
/// a guard held here would stay held in the child until its `exec`. A test that
/// provoked the retry with an in-process guard would therefore be measuring the
/// scheduler — the same interference issue #606 records. A helper holds the
/// lock in its own descriptor table, where no peer's fork can reach it, so both
/// the hold and the release are deterministic.
///
/// The elapsed-time check is what makes this non-vacuous. The helper guarantees
/// the first probe reads `Held`, so an implementation that returned `Ok` on it
/// would still satisfy the outcome alone; requiring the call to outlast the
/// release delay proves a `Held` was observed and waited out.
#[cfg(target_os = "linux")]
#[test]
fn settling_waits_out_a_release_from_another_process() -> Result<()> {
    use super::fragment_tests::TempRoot;
    use super::lock_support::{request_release, spawn_helper, wait_for, wait_for_holder};
    use anyhow::ensure;
    use std::thread;
    use std::time::{Duration, Instant};

    /// Long enough that the first probe cannot plausibly follow the release,
    /// and short enough to leave most of `SETTLE_BUDGET` as headroom.
    const RELEASE_DELAY: Duration = Duration::from_millis(100);

    let root = TempRoot::new()?;
    let holder = spawn_helper(root.path(), "hold-until-asked")?;
    // Only returns once the helper has recorded that it owns the lock, so the
    // first probe below cannot race the acquisition.
    wait_for_holder(root.path())?;

    let release_root = root.path().to_path_buf();
    let releaser = thread::spawn(move || {
        thread::sleep(RELEASE_DELAY);
        request_release(&release_root)
    });

    let started = Instant::now();
    let outcome = super::lock_diagnostics::ensure_settles_free(
        root.path(),
        "a release part-way through the window must settle",
    );
    let waited = started.elapsed();
    releaser
        .join()
        .map_err(|_| anyhow::anyhow!("the releasing thread panicked"))?
        .context("request the holder's release")?;
    // Joined before the outcome is reported, so a failure does not leave the
    // helper waiting on its own timeout.
    wait_for(holder, "holding helper")?;
    outcome?;

    ensure!(
        waited >= RELEASE_DELAY,
        "the settle must have outlasted the release, but it returned after \
         {waited:?}; a first-probe success would satisfy the outcome check alone",
    );
    Ok(())
}

/// Verifies a lock held for the whole window is refused, with a report.
///
/// The holder is again a separate process, so the window belongs to a live
/// owner rather than to a guard this process would have to explain. This is the
/// half that keeps the success path from being vacuous: a settle that returned
/// `Ok` merely because time passed, or that swallowed the report, fails here.
#[cfg(target_os = "linux")]
#[test]
fn settling_refuses_a_lock_that_outlives_the_window() -> Result<()> {
    use super::fragment_tests::TempRoot;
    use super::lock_support::{request_release, spawn_helper, wait_for, wait_for_holder};
    use anyhow::ensure;

    let root = TempRoot::new()?;
    let holder = spawn_helper(root.path(), "hold-until-asked")?;
    wait_for_holder(root.path())?;

    let error = super::lock_diagnostics::ensure_settles_free(root.path(), "a leak must not settle")
        .expect_err("a lock held for the whole window must be reported, not accepted");

    // Released before asserting for the reason the success path records: a
    // failure must not leave the helper blocked until its own timeout expires.
    request_release(root.path())?;
    wait_for(holder, "holding helper")?;

    let rendered = format!("{error:#}");
    ensure!(
        rendered.starts_with("a leak must not settle"),
        "the caller's expectation must lead the report: {rendered}",
    );
    ensure!(
        rendered.contains("is still held"),
        "the report must say the lock was still held: {rendered}",
    );
    ensure!(
        rendered.contains("holds the lock file open as fd"),
        "the report must name the descriptor the live holder kept open: {rendered}",
    );
    Ok(())
}
