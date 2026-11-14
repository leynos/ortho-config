//! Tests for the procfs descriptor scan and the report it feeds.
//!
//! Kept beside `artefact_lock_holder` rather than inside it for the same
//! reason the scan itself was split from the deciding half: the module cap
//! counts every line a module declares, and the scan plus its tests no longer
//! fit in one file.
//!
//! The fake [`ProcFs`] these tests drive lives here rather than in the scan,
//! because the scan must not know it has one. It answers from a table the test
//! builds, so each failure branch is reached by *stating* the fault instead of
//! arranging a filesystem that produces it.

mod holder_tests {
    //! Pins the distinction this module exists to make: only a scan that
    //! finished can say anything about absence, an unfinished one must not, and
    //! neither may discard a descriptor it actually matched.

    use super::super::lock_evidence::{
        Descriptor, DeviceNumber, Inode, LockFileIdentity, ProcessId,
    };
    use super::super::lock_fakes::FakeProcFs;
    use super::super::lock_holder::{
        DescriptorScan, OwnerScan, append_owner, append_owner_lines, inspect_descriptors,
    };
    use super::super::lock_procfs::DescriptorEntries;
    use anyhow::{Result, ensure};
    use rstest::rstest;

    /// Names a lock file identity the tests can compare against.
    const fn identity() -> LockFileIdentity {
        LockFileIdentity::of(DeviceNumber::from_raw(0x0801), Inode::new(42))
    }

    /// Verifies only a completed scan can speak about the descriptors it lacked.
    ///
    /// This is the distinction the module exists for. A report that turns a
    /// failed read into "no descriptor" sends a maintainer after a process that
    /// may be holding the lock, so the incomplete case must say so and must not
    /// describe an absence it never established.
    ///
    /// The complete-and-empty case asserts that *both* explanations are named.
    /// The scan and the lock-table record are separate observations, so an
    /// empty scan is equally explained by a release in between; claiming
    /// inheritance alone would accuse a process in the ordinary case.
    #[rstest]
    #[case::complete_and_empty(DescriptorScan::Complete(Vec::new()), true, true)]
    #[case::complete_and_holding(
        DescriptorScan::Complete(vec![Descriptor::new(7)]),
        false,
        true
    )]
    #[case::incomplete(
        DescriptorScan::Incomplete {
            matched: Vec::new(),
            reason: "permission denied".to_owned(),
        },
        false,
        false
    )]
    fn only_a_complete_scan_may_report_inheritance(
        #[case] scan: DescriptorScan,
        #[case] expects_absence: bool,
        #[case] is_complete: bool,
    ) {
        let own_pid = ProcessId::new(std::process::id());
        let mut lines = Vec::new();
        append_owner(
            &mut lines,
            OwnerScan {
                pid: own_pid,
                scan,
                own_pid,
                procfs: &FakeProcFs::default(),
            },
        );
        let rendered = lines.join("\n");

        assert_eq!(
            rendered.contains("has no descriptor on the lock file"),
            expects_absence,
            "only a completed, empty scan may report an absence: {rendered}",
        );
        assert_eq!(
            rendered.contains("either it released the lock"),
            expects_absence,
            "an absence must name release as an explanation, not inheritance alone: {rendered}",
        );
        assert_eq!(
            rendered.contains("the scan is incomplete"),
            !is_complete,
            "an incomplete scan must say so: {rendered}",
        );
        if !is_complete {
            assert!(
                rendered.contains("permission denied"),
                "an incomplete scan must name the fault it hit: {rendered}",
            );
        }
    }

    /// Verifies a scan that hits an unreadable entry still returns its match.
    ///
    /// This is the case that made the report intermittently useless. Another
    /// thread closing an unrelated descriptor between enumeration and the
    /// identity read fails an entry that has nothing to do with the lock, and
    /// `/proc/<pid>/fd` holds every descriptor in the process, so this happens
    /// during ordinary parallel test execution. A scan that discarded its
    /// matches, or that returned at the first fault, would report the lock
    /// unexplained while the guard was plainly still open.
    ///
    /// The two cases differ only in which descriptor reads first, and that is
    /// the whole point of pinning both. `match_before_fault` is the weaker
    /// order: a scan that kept what it had matched and then stopped still
    /// passes it. The listing is sorted, so `fault_before_match` is the order
    /// that actually occurs — an unrelated descriptor closed by another thread
    /// takes the *lowest* free number — and only a scan that reads on past the
    /// fault finds the lock's descriptor at all.
    ///
    /// The listing is sorted here for the same reason the kernel's is: the
    /// order a case exercises is then decided by the descriptor numbers it
    /// states, not by the order the case happens to list them in. The sort is
    /// over those numbers rather than over `Descriptor`, which deliberately
    /// offers no ordering of its own.
    #[rstest]
    #[case::match_before_fault(7, 8)]
    #[case::fault_before_match(7, 2)]
    fn a_fault_does_not_hide_a_match(#[case] matched: u32, #[case] faulted: u32) -> Result<()> {
        let pid = ProcessId::new(1);
        let mut reading_order = [matched, faulted];
        reading_order.sort_unstable();
        let procfs = FakeProcFs::default()
            .with_descriptors(pid, reading_order.map(Descriptor::new).to_vec())
            .with_identity(pid, Descriptor::new(matched), identity())
            .with_identity_failure(pid, Descriptor::new(faulted), "the entry vanished");

        let outcome = inspect_descriptors(pid, identity(), &procfs);
        let Err((found, reason)) = outcome else {
            anyhow::bail!("a scan with an unreadable entry must not report success");
        };

        ensure!(
            found == vec![Descriptor::new(matched)],
            "a match must survive a fault whichever side of it the match sorts \
             on; discarding it, or stopping at the fault, is the regression \
             this test exists for; got {found:?}",
        );
        ensure!(
            reason.contains(&faulted.to_string()),
            "the failure must name the entry it could not read: {reason}",
        );
        Ok(())
    }

    /// Verifies every fault is named, not just the first.
    ///
    /// The scan reads on after a fault, so more than one entry can fail. The
    /// report has one `reason` field, so the scan has to compose them; a scan
    /// that carried on but kept only the first fault would report a directory
    /// as one bad entry when two were unreadable, and a maintainer checking
    /// the first would find it excused and stop.
    #[test]
    fn every_unreadable_entry_is_named() -> Result<()> {
        let pid = ProcessId::new(1);
        let procfs = FakeProcFs::default()
            .with_descriptors(pid, vec![Descriptor::new(2), Descriptor::new(5)])
            .with_identity_failure(pid, Descriptor::new(2), "the entry vanished")
            .with_identity_failure(pid, Descriptor::new(5), "permission denied");

        let outcome = inspect_descriptors(pid, identity(), &procfs);
        let Err((_, reason)) = outcome else {
            anyhow::bail!("a scan with unreadable entries must not report success");
        };

        for descriptor in ["fd 2", "fd 5"] {
            ensure!(
                reason.contains(descriptor),
                "both unreadable entries must be named, missing {descriptor}: {reason}",
            );
        }
        ensure!(
            reason.contains("the entry vanished") && reason.contains("permission denied"),
            "each entry's own fault must survive composition: {reason}",
        );
        Ok(())
    }

    /// Verifies an enumeration that stops part-way keeps what it listed.
    ///
    /// A directory read can fail after it has already yielded entries. The
    /// entries it produced are real observations, so a scan that returned only
    /// the enumeration fault would discard a descriptor it had genuinely read —
    /// the same defect as stopping at a failed identity, one step earlier. The
    /// fault must still reach the report, so the scan is not passed off as
    /// complete.
    #[test]
    fn an_enumeration_fault_keeps_the_entries_it_yielded() -> Result<()> {
        let pid = ProcessId::new(1);
        let procfs = FakeProcFs::default()
            .with_partial_listing(
                pid,
                vec![Descriptor::new(7)],
                "/proc/1/fd: an entry could not be read (Stale file handle)",
            )
            .with_identity(pid, Descriptor::new(7), identity());

        let outcome = inspect_descriptors(pid, identity(), &procfs);
        let Err((found, reason)) = outcome else {
            anyhow::bail!("a listing that stopped part-way must not report success");
        };

        ensure!(
            found == vec![Descriptor::new(7)],
            "a descriptor listed before the enumeration fault must survive it; \
             got {found:?}",
        );
        ensure!(
            reason.contains("could not be read"),
            "the enumeration fault itself must reach the report: {reason}",
        );
        Ok(())
    }

    /// Verifies a partial scan reports the evidence it kept without claiming absence.
    ///
    /// The scan-level test above pins that the matches survive; this one pins
    /// what the report then does with them. A report built from a partial scan
    /// must still name the live descriptor and the fault, and must not say the
    /// process holds nothing — the absence it could not establish.
    #[test]
    fn a_partial_scan_reports_its_matches_without_claiming_an_absence() {
        let own_pid = ProcessId::new(std::process::id());
        let scan = DescriptorScan::Incomplete {
            matched: vec![Descriptor::new(7)],
            reason: "a later entry was unreadable".to_owned(),
        };
        let mut lines = Vec::new();
        append_owner(
            &mut lines,
            OwnerScan {
                pid: own_pid,
                scan,
                own_pid,
                procfs: &FakeProcFs::default(),
            },
        );
        let rendered = lines.join("\n");

        assert!(
            rendered.contains("holds the lock file open as fd 7"),
            "a matched descriptor must appear in the report: {rendered}",
        );
        assert!(
            rendered.contains("a later entry was unreadable"),
            "the report must still name the fault: {rendered}",
        );
        assert!(
            rendered.contains("the scan is incomplete"),
            "the report must still say the scan did not finish: {rendered}",
        );
        assert!(
            !rendered.contains("has no descriptor on the lock file"),
            "a partial scan must not claim an absence it never established: {rendered}",
        );
    }

    /// Verifies a process with no descriptor table reports an incomplete scan.
    ///
    /// This is the injected counterpart to the real-procfs case the report
    /// suite drives: the fake returns the listing failure, so the whole path
    /// from `append_owner_lines` — enumeration, failure, counting, and the
    /// summary line that says the evidence is partial — runs without needing a
    /// pid the kernel is guaranteed to reject.
    #[test]
    fn an_unlistable_descriptor_table_is_an_incomplete_scan() {
        let pid = ProcessId::new(4242);
        let procfs = FakeProcFs::default().with_listing_failure(pid, "/proc/4242/fd: unreadable");

        let error = inspect_descriptors(pid, identity(), &procfs)
            .expect_err("an unlistable table must not report success");
        assert!(
            error.0.is_empty(),
            "nothing was read, so nothing can have matched: {:?}",
            error.0
        );
        assert!(
            error.1.contains("unreadable"),
            "the failure must say the table could not be read: {}",
            error.1,
        );

        let mut lines = Vec::new();
        append_owner_lines(&mut lines, &[pid], identity(), &procfs);
        let rendered = lines.join("\n");
        assert!(
            rendered.contains("could not be read completely"),
            "an unlistable table must make the report say it is partial: {rendered}",
        );
        assert!(
            !rendered.contains("has no descriptor on the lock file"),
            "an unlistable table must not be reported as holding nothing: {rendered}",
        );
    }

    /// Verifies non-descriptor names are skipped without failing the scan.
    ///
    /// Every entry under `/proc/<pid>/fd` is a decimal descriptor number
    /// written by the kernel, so a name that does not parse is not a descriptor
    /// this scan could have examined and omitted. Treating it as a fault would
    /// mark every process incompletely scanned and drown the real failures.
    /// The skipped name must also leave the descriptor that does parse intact,
    /// which is why the good descriptor is listed after it.
    #[test]
    fn names_that_are_not_descriptors_are_skipped() -> Result<()> {
        let pid = ProcessId::new(7);
        let entries = DescriptorEntries::new(
            vec![Descriptor::new(3)],
            vec!["not-a-descriptor".to_owned(), "\u{fffd}\u{fffd}".to_owned()],
        );
        // The reader keeps the names it could not parse, so the skip is
        // recorded rather than silent: a report built from this table can say
        // what it did not understand.
        ensure!(
            entries.unparsed() == ["not-a-descriptor", "\u{fffd}\u{fffd}"],
            "the names that are not descriptors must survive the listing: {:?}",
            entries.unparsed(),
        );
        let procfs = FakeProcFs::default()
            .with_entry_listing(pid, entries)
            .with_identity(pid, Descriptor::new(3), identity());

        let matched = inspect_descriptors(pid, identity(), &procfs).map_err(|(_, reason)| {
            anyhow::anyhow!("non-descriptor names must not fail the scan: {reason}")
        })?;

        ensure!(
            matched == vec![Descriptor::new(3)],
            "only the numeric name may be reported, and the skip must not \
             disturb it; got {matched:?}",
        );
        Ok(())
    }

    /// Verifies a descriptor on another file is not reported as holding the lock.
    ///
    /// The scan's job is to name descriptors *on this file*. A descriptor that
    /// opens something else is an ordinary observation, not a fault, and must
    /// neither appear in the report nor make the scan incomplete.
    #[test]
    fn a_descriptor_on_another_file_is_not_reported() -> Result<()> {
        let pid = ProcessId::new(9);
        let other = LockFileIdentity::of(DeviceNumber::from_raw(0x0802), Inode::new(99));
        let procfs = FakeProcFs::default()
            .with_descriptors(pid, vec![Descriptor::new(5)])
            .with_identity(pid, Descriptor::new(5), other);

        let matched = inspect_descriptors(pid, identity(), &procfs).map_err(|(_, reason)| {
            anyhow::anyhow!("an unrelated descriptor is not a fault: {reason}")
        })?;

        ensure!(
            matched.is_empty(),
            "a descriptor on another file must not be reported as the lock: {matched:?}",
        );
        Ok(())
    }
}
