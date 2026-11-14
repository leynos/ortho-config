//! Tests for the procfs descriptor scan and the report it feeds.
//!
//! Kept beside `artefact_lock_holder` rather than inside it for the same
//! reason the scan itself was split from the deciding half: the module cap
//! counts every line a module declares, and the scan plus its tests no longer
//! fit in one file.

mod holder_tests {
    //! Pins the distinction this module exists to make: only a scan that
    //! finished can say anything about absence, an unfinished one must not, and
    //! neither may discard a descriptor it actually matched.

    use super::super::lock_holder::{DescriptorScan, append_owner, inspect_descriptors};
    use crate::derive::generate::localization::artefact::fragment_tests::TempRoot;
    use crate::derive::generate::localization::artefact::lock_evidence::{Descriptor, ProcessId};
    use crate::derive::generate::localization::artefact::lock_evidence::{
        DeviceNumber, Inode, LockFileIdentity,
    };
    use anyhow::{Result, ensure};
    use rstest::rstest;
    use std::fs;
    use std::os::unix::fs::MetadataExt;
    use std::path::Path;

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
        append_owner(&mut lines, own_pid, scan, own_pid);
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

    /// Verifies a scan that fails after matching a descriptor still returns it.
    ///
    /// This is the case that made the report intermittently useless. Another
    /// thread closing an unrelated descriptor between enumeration and the
    /// metadata read fails an entry that has nothing to do with the lock, and
    /// `/proc/<pid>/fd` holds every descriptor in the process, so this happens
    /// during ordinary parallel test execution. A scan that discarded its
    /// matches would report the lock unexplained while the guard was plainly
    /// still open.
    ///
    /// The scan is driven over a real temporary directory rather than asserted
    /// through `append_owner`, and that is deliberate. An earlier version of
    /// this test built a `DescriptorScan::Incomplete` by hand and passed it to
    /// the report; it stayed green when the scan was mutated to discard its
    /// matches, because the discard happens in the scan and the test never ran
    /// it. Only driving `inspect_descriptors` over real entries pins the
    /// behaviour that regressed.
    ///
    /// The entries are numbered so the live descriptor sorts before the broken
    /// one, which is what makes the expectation exact: the scan reads `7` —
    /// a real hard link to the lock file, so its metadata matches — and then
    /// fails on `8`, a dangling symlink whose metadata cannot be read. Sorting
    /// is why this is deterministic rather than dependent on enumeration order.
    #[test]
    fn a_partial_scan_still_returns_the_descriptors_it_matched() -> Result<()> {
        use crate::derive::generate::localization::artefact::lock_evidence::{
            DeviceNumber, Inode, LockFileIdentity,
        };
        use std::os::unix::fs::MetadataExt;

        let root = TempRoot::new()?;
        let lock_file = root.path().join("lock");
        fs::write(&lock_file, b"")?;
        let metadata = fs::metadata(&lock_file)?;
        let identity = LockFileIdentity::of(
            DeviceNumber::from_raw(metadata.dev()),
            Inode::new(metadata.ino()),
        );

        let scanned = root.path().join("fd");
        fs::create_dir(&scanned)?;
        // A hard link is the same inode on the same device, so the scan's
        // identity check must accept it exactly as it accepts `/proc`'s.
        fs::hard_link(&lock_file, scanned.join("7"))?;
        // Dangling: `fs::metadata` follows the link and fails, which is the
        // ordinary "entry vanished between enumeration and read" case.
        std::os::unix::fs::symlink(root.path().join("gone"), scanned.join("8"))?;

        let outcome = inspect_descriptors(&scanned.to_string_lossy(), identity);
        let Err((matched, reason)) = outcome else {
            anyhow::bail!("a scan with an unreadable entry must not report success");
        };

        ensure!(
            matched == vec![Descriptor::new(7)],
            "the descriptor matched before the failure must survive it; \
             discarding it is the regression this test exists for; got {matched:?}",
        );
        ensure!(
            reason.contains('8'),
            "the failure must name the entry it could not read: {reason}",
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
        append_owner(&mut lines, own_pid, scan, own_pid);
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

    /// Verifies a process with no `/proc` entry reports an incomplete scan.
    ///
    /// Unlike the table above, nothing here is injected: this is a real pid
    /// the kernel cannot describe, so the whole path from `append_owner_lines`
    /// — enumeration, failure, counting, and the summary line that says the
    /// evidence is partial — is exercised against procfs itself. A pid of
    /// `u32::MAX` is above `pid_max` on every supported kernel, so the scan
    /// cannot accidentally observe a live process.
    #[test]
    fn a_scan_that_cannot_be_read_reports_the_report_as_incomplete() {
        use super::super::lock_holder::append_owner_lines;
        use crate::derive::generate::localization::artefact::lock_evidence::{
            DeviceNumber, Inode, LockFileIdentity,
        };

        let missing = ProcessId::new(u32::MAX);
        let identity = LockFileIdentity::of(DeviceNumber::from_raw(0), Inode::new(0));
        let mut lines = Vec::new();
        append_owner_lines(&mut lines, &[missing], identity);
        let rendered = lines.join("\n");

        assert!(
            rendered.contains(&format!("/proc/{missing}/fd")),
            "the report must name the directory it could not read: {rendered}",
        );
        assert!(
            rendered.contains("the scan is incomplete"),
            "an unreadable process must be reported as an incomplete scan: \
             {rendered}",
        );
        assert!(
            rendered.contains("1 process(es) could not be read completely"),
            "the summary must count the process the scan could not read: \
             {rendered}",
        );
        assert!(
            !rendered.contains("has no descriptor on the lock file"),
            "an unreadable process must not be reported as holding nothing: \
             {rendered}",
        );
    }

    /// Verifies an unscannable directory is a failure, not an empty result.
    ///
    /// This is the other half of "did this scan finish": nothing at all was
    /// read, so the report must say the scan is incomplete and must not treat
    /// the emptiness as an absence. The directory is one that cannot exist, so
    /// `read_dir` fails for every user and no permissions manifest is needed.
    #[test]
    fn a_directory_that_cannot_be_listed_is_an_incomplete_scan() {
        let identity = LockFileIdentity::of(DeviceNumber::from_raw(0), Inode::new(0));

        let missing = Path::new("/nonexistent-ortho-config-artefact-scan");
        let outcome = inspect_descriptors(&missing.to_string_lossy(), identity);
        let Err((matched, reason)) = outcome else {
            panic!("a directory that cannot be listed must not report success");
        };

        assert!(
            matched.is_empty(),
            "nothing was read, so nothing can have matched: {matched:?}",
        );
        assert!(
            reason.contains("unreadable"),
            "the failure must say the directory could not be read: {reason}",
        );
    }

    /// Verifies non-descriptor names are skipped without failing the scan.
    ///
    /// The skip is deliberate: every entry under `/proc/<pid>/fd` is a decimal
    /// descriptor number written by the kernel, so a non-numeric name is not a
    /// descriptor this scan could have examined and omitted. Treating it as a
    /// fault would report every process as incompletely scanned and drown the
    /// real failures. A placeholder word and a non-UTF-8 byte sequence cover
    /// the two ways a name can fail to parse, and neither may affect the
    /// descriptor that does parse.
    #[test]
    fn names_that_are_not_descriptors_are_skipped() -> Result<()> {
        use std::ffi::OsStr;
        use std::os::unix::ffi::OsStrExt;

        let root = TempRoot::new()?;
        let lock_file = root.path().join("lock");
        fs::write(&lock_file, b"")?;
        let metadata = fs::metadata(&lock_file)?;
        let identity = LockFileIdentity::of(
            DeviceNumber::from_raw(metadata.dev()),
            Inode::new(metadata.ino()),
        );

        let scanned = root.path().join("fd");
        fs::create_dir(&scanned)?;
        fs::hard_link(&lock_file, scanned.join("7"))?;
        // A word where a number belongs, and a name that is not even valid
        // UTF-8. Both are skipped; neither may turn the scan into a failure.
        fs::hard_link(&lock_file, scanned.join("not-a-descriptor"))?;
        let non_utf8 = OsStr::from_bytes(b"\xff\xfe");
        fs::hard_link(&lock_file, scanned.join(non_utf8))?;

        let outcome = inspect_descriptors(&scanned.to_string_lossy(), identity);
        let Ok(matched) = outcome else {
            return Err(anyhow::anyhow!(
                "non-descriptor names must be skipped, not treated as a failed scan"
            ));
        };

        ensure!(
            matched == vec![Descriptor::new(7)],
            "only the numeric name may be reported, and the skip must not \
             disturb it; got {matched:?}",
        );
        Ok(())
    }
}
