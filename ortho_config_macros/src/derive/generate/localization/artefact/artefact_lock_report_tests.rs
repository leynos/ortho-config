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

/// Returns every descriptor this process holds on `path`, read from procfs.
///
/// Deliberately independent of the diagnostics: the kernel's descriptor table is
/// the fact a holder report is supposed to reproduce, so the test asks `/proc`
/// itself rather than parsing the report and asserting it agrees with itself.
#[cfg(target_os = "linux")]
fn descriptors_holding(path: &std::path::Path) -> Result<Vec<u32>> {
    use std::os::unix::fs::MetadataExt;

    let target = std::fs::metadata(path).context("read the lock file's identity")?;
    let want = (target.dev(), target.ino());
    let entries = std::fs::read_dir("/proc/self/fd").context("list this process's descriptors")?;
    let found = entries
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let descriptor = entry.file_name().into_string().ok()?.parse::<u32>().ok()?;
            let meta = std::fs::metadata(entry.path()).ok()?;
            ((meta.dev(), meta.ino()) == want).then_some(descriptor)
        })
        .collect();
    Ok(found)
}

/// Verifies a genuine hold is reported with the holder's own identity.
///
/// This is the whole point of the module: issue #606's failures could say only
/// that a released lock looked held. A report that names this process, the file,
/// and the descriptor it still holds turns an unactionable boolean into the one
/// fact needed to tell a leaked handle from a foreign holder.
///
/// The descriptor is checked against `/proc/self/fd`, not against the report's
/// own text. Asserting that the report *contains* the words "holds the lock file
/// open as fd" would pass for any descriptor at all, including one belonging to
/// a different file; asking procfs for the descriptors on this file and
/// requiring the report to name one of them is what makes the claim testable.
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

    // Read before the report is built: the report scans `/proc` itself, and a
    // second question about the same descriptor table is the independent answer
    // this assertion compares it against.
    let expected = descriptors_holding(&super::artefact_lock::lock_path(root.path()))?;

    let report =
        super::lock_diagnostics::ensure_released(root.path(), "the guard must release the lock")
            .expect_err("a lock this process still holds must be reported as held");
    drop(held);

    let rendered = format!("{report:#}");
    ensure!(
        rendered.starts_with("the guard must release the lock"),
        "the caller's expectation must lead the report: {rendered}",
    );
    for expected_text in [
        "is still held",
        "this process",
        "holds the lock file open as fd",
        "re-probe after",
    ] {
        ensure!(
            rendered.contains(expected_text),
            "the report must contain {expected_text:?}, but it did not: {rendered}",
        );
    }
    // The pid is asserted as a *value*, not as the "this process" label, and
    // it is anchored to the `holder:` line rather than to the report as a
    // whole. The `/proc/locks` record line names a pid too, so a bare
    // `contains("pid N")` would be satisfied by the evidence line even when
    // `describe_process` printed the wrong one; only the `holder:` prefix ties
    // the number to the sentence that describes the holder.
    ensure!(
        rendered.contains(&format!("holder: pid {} ", std::process::id())),
        "the holder line must name this process's pid {} because the guard \
         lives here, but it named another: {rendered}",
        std::process::id(),
    );
    ensure!(
        !expected.is_empty(),
        "the held guard must own at least one descriptor on the lock file, or this \
         case asserts nothing about the report: {rendered}",
    );
    // Matched as a whole word, not as a prefix. `contains("as fd 3")` also
    // matches "as fd 30", so a report naming the wrong descriptor would pass
    // whenever the real one happened to be a prefix of it.
    ensure!(
        expected.iter().any(|descriptor| {
            rendered
                .split(|character: char| !character.is_ascii_digit())
                .any(|number| number == descriptor.to_string())
        }),
        "the report must name one of the descriptors procfs shows on this file \
         {expected:?}, but it named none of them: {rendered}",
    );
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
/// lock in its own descriptor table, where no peer's fork can reach it.
///
/// The release is ordered by *what the settle observed*, never by a timer. An
/// earlier draft slept in a releaser thread and asserted the settle outlasted
/// the sleep, which did not hold the probe order it claimed to: a releaser
/// scheduled first, against a delayed test thread, could finish before the
/// first probe, and a correct settle would then return early and fail. Worse,
/// elapsed time cannot distinguish "waited out a hold" from "happened to be
/// slow", so the sleep proved the retry only by coincidence.
///
/// Here the first `Held` the settle reports is what triggers the release, so
/// the release cannot precede the observation that asked for it. The order is
/// then a fact about the code: some number of held probes, the release
/// requested after the first of them, then a free one.
///
/// The number of held probes is *not* fixed. The helper polls for the request
/// on its own `POLL_INTERVAL` while the settle re-probes every
/// `SETTLE_INTERVAL` (10 ms), and the poll interval is the longer of the two,
/// so a correct settle routinely sees several held probes before the holder
/// answers. Requiring a fixed count would make this test fail whenever the
/// helper was merely slow, which is the scheduler again rather than the lock.
/// The intervals are deliberately not restated as literals here: they live in
/// `artefact_lock` and `artefact_lock_diagnostics`, and a copy in a comment
/// silently goes stale when one of them is retuned. What is asserted instead
/// is the *shape*:
/// at least one `Held`, a free probe after it, and no free probe before it.
#[cfg(target_os = "linux")]
#[test]
fn settling_waits_out_a_release_from_another_process() -> Result<()> {
    use super::fragment_tests::TempRoot;
    use super::lock_support::{request_release, spawn_helper, wait_for, wait_for_holder};
    use anyhow::ensure;
    use std::cell::RefCell;

    let root = TempRoot::new()?;
    let holder = spawn_helper(root.path(), "hold-until-asked")?;
    // Only returns once the helper has recorded that it owns the lock, so the
    // first probe below cannot race the acquisition.
    wait_for_holder(root.path())?;

    // Scoped rather than spawned: the closure borrows the record, and a
    // release issued from another thread could still overtake probe one.
    let probes = RefCell::new(Vec::new());
    let outcome = super::lock_diagnostics::ensure_settles_free_observed(
        root.path(),
        "a release part-way through the window must settle",
        &|probe| {
            let is_free = matches!(probe, super::artefact_lock::LockProbe::Free);
            let mut seen = probes.borrow_mut();
            seen.push(is_free);
            if !is_free && seen.len() == 1 {
                request_release(root.path())
                    .expect("the first held probe must be able to ask for the release");
            }
        },
    );

    // Joined before the outcome is reported, so a failure does not leave the
    // helper waiting on its own timeout.
    wait_for(holder, "holding helper")?;
    outcome?;

    let seen = probes.into_inner();
    let held = seen.iter().take_while(|is_free| !**is_free).count();
    ensure!(
        held > 0,
        "the settle must observe a held lock before the release it asked for, \
         but it read a free lock first: {seen:?} (false is held, true is free); \
         a settle that never saw the hold cannot have waited it out",
    );
    ensure!(
        seen.get(held) == Some(&true),
        "the settle must observe a free lock once the holder answered, but it \
         saw {seen:?} (false is held, true is free); a settle that returned `Ok` \
         before the release, or that reported a free probe followed by a held \
         one, cannot produce this",
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
    // Read before the report exists: the pid the report must name is the
    // helper's, and no later call can change what the kernel gave it.
    let holder_pid = holder.id();

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
    // The foreign holder's identity is asserted too, not just the sentence
    // shape. This is the case issue #606 is about — a real other process
    // holding the lock — so a report that named this process, or one of its
    // own children by mistake, has to fail here rather than read as a
    // plausible hold.
    //
    // Anchored to the `holder:` line for the reason the in-process case
    // records: the `/proc/locks` record line also names a pid, so an
    // unanchored match would pass even when the holder line named the wrong
    // process. The relation phrase sits on that same line, so the two
    // together pin one sentence: the number and the label must agree.
    ensure!(
        rendered.contains(&format!("holder: pid {holder_pid} ")),
        "the holder line must name the helper's pid {holder_pid}, because that \
         is the process holding the lock, but it named another: {rendered}",
    );
    ensure!(
        rendered.contains("child of this process"),
        "the report must say how the holder relates to the reporter, and the \
         helper is its child: {rendered}",
    );
    ensure!(
        rendered.contains("holds the lock file open as fd"),
        "the report must name the descriptor the live holder kept open: {rendered}",
    );
    Ok(())
}
