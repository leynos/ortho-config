//! The settle's temporal behaviour, driven by a fake clock.
//!
//! The real-clock tests in `artefact_lock_report_tests` prove the settle works
//! against a genuine holder, which is the behaviour that matters but not the
//! *boundary* behaviour: with a real clock, "the deadline passed" can only be
//! reached by waiting out the real budget, and "it did not sleep after the
//! answer" cannot be observed at all. Both are decisions this module makes, and
//! both are asserted here instead of raced for.
//!
//! The settle still probes a real lock file: the probe is the production one,
//! and only the clock and the procfs behind the *report* are doubled. That
//! keeps the test honest about the part it does exercise — the loop, its
//! deadline, and where it sleeps — rather than replacing the whole function.

mod settle_tests {
    //! Pins the deadline, the sleep placement, and the release-observed case.

    use super::super::lock_diagnostics::{
        Boundaries, SETTLE_BUDGET, ensure_settles_free_observed_with, ensure_settles_free_with,
    };
    use super::super::lock_fakes::{FakeClock, FakeProcFs};
    use crate::derive::generate::localization::artefact::fragment_tests::TempRoot;
    use anyhow::{Result, ensure};
    use std::time::{Duration, Instant};

    /// Verifies a lock held past the budget fails with the hold report.
    ///
    /// The clock advances by one settle interval per sleep, so the deadline is
    /// reached after a bounded number of probes rather than after two seconds
    /// of real waiting. What the test pins is that the deadline *ends the
    /// loop*: the lock never frees, so the only exit is the budget expiring.
    ///
    /// The report is asserted to carry the lock file and the re-probe line, so
    /// this is the full hold report rather than a bare timeout message.
    #[test]
    fn a_hold_that_outlives_the_budget_fails_with_the_report() -> Result<()> {
        let root = TempRoot::new()?;
        super::super::artefact_lock::create_lock(root.path())?;
        let guard = super::super::artefact_lock::lock(root.path())?;

        let clock = FakeClock::new(Instant::now(), Duration::from_millis(10));
        let error = ensure_settles_free_with(
            root.path(),
            "a leak must not settle",
            &Boundaries {
                clock: &clock,
                procfs: &FakeProcFs::default(),
            },
        )
        .expect_err("a lock held past the budget must not settle");

        let rendered = format!("{error:#}");
        ensure!(
            rendered.starts_with("a leak must not settle"),
            "the caller's expectation must lead: {rendered}",
        );
        ensure!(
            rendered.contains("is still held"),
            "the failure must report the hold it observed: {rendered}",
        );
        ensure!(
            rendered.contains("re-probe"),
            "the report must carry its delayed re-probe: {rendered}",
        );

        // The loop must have run until the budget was spent, not returned on
        // the first held probe.
        let sleeps = clock.sleeps();
        ensure!(
            !sleeps.is_empty(),
            "the settle must have probed repeatedly, not given up at once",
        );
        let spent: Duration = sleeps.iter().sum();
        ensure!(
            spent >= SETTLE_BUDGET,
            "the settle must not report a hold before its budget is spent: {spent:?}",
        );

        drop(guard);
        Ok(())
    }

    /// Verifies the settle stops at the deadline rather than sleeping past it.
    ///
    /// This is the assertion a real clock cannot make. The clock advances by
    /// exactly the settle interval, so the loop exits the first time `now` has
    /// reached the deadline; a settle that slept once more before checking
    /// would record an extra wait here.
    ///
    /// The expected count is derived from the budget and the step rather than
    /// written as a literal, so retuning either constant keeps the test
    /// meaningful instead of silently passing for the wrong interval.
    #[test]
    fn the_settle_sleeps_only_between_probes() -> Result<()> {
        let root = TempRoot::new()?;
        super::super::artefact_lock::create_lock(root.path())?;
        let guard = super::super::artefact_lock::lock(root.path())?;

        let step = Duration::from_millis(10);
        let clock = FakeClock::new(Instant::now(), step);
        let probes = std::cell::Cell::new(0_usize);
        ensure_settles_free_observed_with(
            root.path(),
            "a leak must not settle",
            &|_| probes.set(probes.get().saturating_add(1)),
            &Boundaries {
                clock: &clock,
                procfs: &FakeProcFs::default(),
            },
        )
        .expect_err("a held lock must not settle inside its budget");

        let sleeps = clock.sleeps();
        // The last wait is the report's own delayed re-probe, which is a
        // different interval and a different concern; only the waits before it
        // are the settle's.
        let (settle_waits, report_waits) = sleeps.split_at(sleeps.len().saturating_sub(1));
        ensure!(
            settle_waits
                .iter()
                .all(|&wait| wait == super::super::lock_diagnostics::SETTLE_INTERVAL),
            "every settle wait must be one settle interval: {settle_waits:?}",
        );
        ensure!(
            report_waits == [super::super::lock_diagnostics::RECHECK_DELAY],
            "the report must make exactly one delayed re-probe: {report_waits:?}",
        );
        // One settling wait per probe that was followed by another probe. The
        // final probe ends the loop, so it is not followed by a settle wait.
        ensure!(
            settle_waits.len() == probes.get().saturating_sub(1),
            "the settle must not sleep after its last probe: {} waits for {} probes",
            settle_waits.len(),
            probes.get(),
        );
        drop(guard);
        Ok(())
    }

    /// Verifies a release observed part-way through settles.
    ///
    /// The guard is released from the observation hook, which runs after a
    /// probe has reported `Held`. A settle that gave up on the first held
    /// probe, or that stopped observing, would fail this. The clock never
    /// advances, so success cannot come from the budget running out: the only
    /// way out of the loop is the free probe the release produces.
    ///
    /// The release happens *inside* the hook, so it lands strictly after the
    /// observation that triggers it — held, released, free is a fact about the
    /// code rather than a race the assertion hopes to win.
    #[test]
    fn a_release_observed_part_way_through_settles() -> Result<()> {
        let root = TempRoot::new()?;
        super::super::artefact_lock::create_lock(root.path())?;
        let guard = std::cell::RefCell::new(Some(super::super::artefact_lock::lock(root.path())?));

        let clock = FakeClock::new(Instant::now(), Duration::ZERO);
        let observed = std::cell::RefCell::new(Vec::new());
        let outcome = ensure_settles_free_observed_with(
            root.path(),
            "an observed release must settle",
            &|probe| {
                let is_free = matches!(probe, super::super::artefact_lock::LockProbe::Free);
                observed.borrow_mut().push(is_free);
                if !is_free {
                    // Dropping the guard is the release. `RefCell` because the
                    // hook is a `Fn`: it must be callable once per probe, so it
                    // cannot hold a mutable borrow of its captures.
                    drop(guard.borrow_mut().take());
                }
            },
            &Boundaries {
                clock: &clock,
                procfs: &FakeProcFs::default(),
            },
        );

        outcome?;
        let seen = observed.into_inner();
        let held = seen.iter().take_while(|is_free| !**is_free).count();
        ensure!(
            held > 0,
            "the settle must observe the hold before the release it caused: {seen:?}",
        );
        ensure!(
            seen.get(held) == Some(&true),
            "a free probe must follow the release the settle observed: {seen:?}",
        );
        Ok(())
    }

    /// Verifies an unreadable lock table is reported inside the hold report.
    ///
    /// The table is the evidence that names the *acquirer*, and the report is
    /// already being written when it fails. It must therefore appear as a line
    /// in that report rather than replace it: a failure to explain a hold can
    /// never be allowed to mask the hold. The fake states the fault, so this
    /// branch is reached without needing a `/proc` that will not read.
    #[test]
    fn an_unreadable_lock_table_is_reported_within_the_hold_report() -> Result<()> {
        let root = TempRoot::new()?;
        super::super::artefact_lock::create_lock(root.path())?;
        let guard = super::super::artefact_lock::lock(root.path())?;

        let clock = FakeClock::new(Instant::now(), Duration::from_millis(10));
        let procfs = FakeProcFs::default().with_table_failure("/proc/locks: unreadable (injected)");
        let error = ensure_settles_free_with(
            root.path(),
            "a leak must not settle",
            &Boundaries {
                clock: &clock,
                procfs: &procfs,
            },
        )
        .expect_err("a lock held past the budget must not settle");

        let rendered = format!("{error:#}");
        ensure!(
            rendered.contains("/proc/locks: unreadable (injected)"),
            "the table fault must appear as a line in the report: {rendered}",
        );
        ensure!(
            rendered.contains("is still held"),
            "the hold itself must still be reported: {rendered}",
        );
        drop(guard);
        Ok(())
    }

    /// Verifies a described process is named by its command and relation.
    ///
    /// The counterpart to the unreadable case: when the stat line is
    /// available, the report must use it. A report that always said
    /// "<stat unreadable>" would pass the other test while telling a reader
    /// nothing, so the two are asserted together.
    #[test]
    fn a_described_process_is_named_by_its_command() {
        use super::super::lock_evidence::ProcessId;

        let pid = ProcessId::new(4243);
        let own_pid = ProcessId::new(std::process::id());
        let procfs = FakeProcFs::default().with_process(pid, "cargo-orthohelp", own_pid);

        let described = super::super::lock_process::describe_process(pid, own_pid, &procfs);
        assert!(
            described.contains("cargo-orthohelp"),
            "the command name must be reported: {described}",
        );
        assert!(
            described.contains("child of this process"),
            "the parent recorded by the stat line must set the relation: {described}",
        );
        assert!(
            !described.contains("<stat unreadable>"),
            "a readable stat line must not be reported as unreadable: {described}",
        );
    }

    /// Verifies a process the kernel cannot describe is still named in the report.
    ///
    /// A lock table naming a pid the report cannot describe is ordinary: the
    /// process may have exited, or may belong to another user. The report must
    /// name it anyway and say the stat line was unreadable, because the reader
    /// needs to know the relation is *unknown* rather than absent.
    #[test]
    fn an_undescribable_process_is_named_as_such() -> Result<()> {
        use super::super::lock_evidence::{DeviceNumber, Inode, LockFileIdentity, ProcessId};

        let pid = ProcessId::new(4242);
        let mut lines = Vec::new();
        super::super::lock_holder::append_owner_lines(
            &mut lines,
            &[pid],
            LockFileIdentity::of(DeviceNumber::from_raw(0x0801), Inode::new(42)),
            &FakeProcFs::default().with_listing_failure(pid, "/proc/4242/fd: unreadable"),
        );

        let rendered = lines.join("\n");
        ensure!(
            rendered.contains("pid 4242"),
            "the report must name the process the table named: {rendered}",
        );
        ensure!(
            rendered.contains("<stat unreadable>"),
            "an undescribable process must say its stat was unreadable: {rendered}",
        );
        ensure!(
            !rendered.contains("has no descriptor on the lock file"),
            "an unlistable table must not be reported as holding nothing: {rendered}",
        );
        Ok(())
    }
}
