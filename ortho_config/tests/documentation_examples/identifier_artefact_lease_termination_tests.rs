//! An interrupted helper must not hand the target to a replacement.
//!
//! The lease-inheritance regressions prove the ordering: a nested writer keeps
//! the lease after its holder's handle closes. That ordering alone would be a
//! regression of its own if it also meant an orphaned build could never be
//! removed, so this module proves the other half. A helper leases the real
//! fixture target, starts a nested writer through the same runner the fixture
//! build uses, and is then killed from the outside. The target must remain
//! refused to a replacement while the orphan can still write to it, and become
//! acquirable once the orphan is terminated.
//!
//! [`nested_build_helper`] drives [`nested_writer_probe`] directly, because a
//! synthetic writer can be made to outlive its supervisor deterministically.
//! The real-build regression lives in
//! [`identifier_artefact_lease_cargo_build_tests`], which drives an actual
//! `cargo build` and proves the same ordering through the process tree the
//! fixture starts. Cargo gives its own children — build scripts and rustc — a
//! null standard input, so the lease reaches Cargo and stops there; those
//! descendants are removed by the tree termination, not by the lease.
//!
//! On Unix the helper leads its own process group and its nested writer is in
//! another, so [`Descendants`] records and terminates the tree that survives
//! the kill. On Windows the helper's Job Object owns that tree, so closing it
//! with the helper terminates the writer and the target is released without a
//! snapshot.
//!
//! [`nested_writer_probe`]: super::identifier_artefact_lease_inheritance_tests::nested_writer_probe

use super::identifier_artefact_e2e_support::acquire;
use super::identifier_artefact_e2e_support::locate_fixture;
#[cfg(unix)]
use super::identifier_artefact_e2e_support::{Descendants, snapshot_descendants};
#[cfg(unix)]
use super::identifier_artefact_lease_support::REFUSAL_WINDOW;
use super::identifier_artefact_lease_support::{
    HEARTBEAT_FILE, HELPER_STARTUP, HELPER_WORK_BOUND, Helper, REPLACEMENT_WINDOW,
    announced_target, child_test_command, heartbeat_after, heartbeat_length, required_scratch,
    required_scratch_path, wait_for_heartbeat, wait_for_marker,
};
use super::{process_runner, scenario};
use anyhow::{Context, Result, ensure};
use cap_std::ambient_authority;
use cap_std::fs::Dir;
use serial_test::serial;
use std::path::PathBuf;
use std::time::{Duration, Instant};
use tempfile::TempDir;

/// The qualified name of the ignored helper this module drives.
const NESTED_BUILD_HELPER: &str =
    "identifier_artefact_lease_termination_tests::nested_build_helper";

/// The writer the helper starts beneath itself.
const WRITER_PROBE: &str = "identifier_artefact_lease_inheritance_tests::nested_writer_probe";

/// How long the orphan is watched for further writes after the helper dies.
///
/// Only the Unix teardown observes an orphan: on Windows the helper's Job
/// Object has already terminated the writer.
#[cfg(unix)]
const ORPHAN_WINDOW: Duration = Duration::from_millis(400);

/// How long the heartbeat is watched for further growth once the orphan ends.
const FROZEN_WINDOW: Duration = Duration::from_millis(400);

/// Lease the target, then hold a writer beneath it until killed.
///
/// Ignored so an ordinary run never starts it; the test below selects it by
/// exact name when it re-runs this binary as its own child. It reports the
/// target and then announces the lease before starting the writer, in that
/// order, so the parent never confuses a stale heartbeat for a live one.
#[test]
#[ignore = "driven as a child process by a_nested_writer_cannot_outlive_a_killed_helper"]
fn nested_build_helper() -> Result<()> {
    let scratch = required_scratch()?;
    // One envelope covers discovery, the lease and the writer, so time already
    // spent is not granted twice. It stays inside the parent's bound so this
    // process's supervised writer reaches its own deadline first.
    let deadline = scenario::deadline().min(Instant::now() + HELPER_WORK_BOUND);
    let location = locate_fixture(deadline)?;
    let target = location.target_directory().to_path_buf();
    scratch
        .write("target.txt", target.display().to_string())
        .context("announce the leased target")?;
    let cargo = location.lease(deadline, None)?;
    // A heartbeat from an earlier run would satisfy the parent's first wait
    // before this run's writer exists, so clear it while the lease is held.
    let target_directory =
        Dir::open_ambient_dir(&target, ambient_authority()).context("open the leased target")?;
    let _removed = target_directory.remove_file(HEARTBEAT_FILE);
    scratch
        .write("lease_held", b"held")
        .context("announce the held lease")?;
    let mut command = child_test_command(WRITER_PROBE, &required_scratch_path()?)?;
    // The runner terminates the writer at this bound. Nothing is asserted here:
    // the parent kills this process first, which is the interruption under
    // test, and any later outcome is the runner doing its own job.
    let _run = process_runner::run_with_policy_with_stdin(
        &mut command,
        process_runner::Operation("hold the leased target beneath the helper"),
        process_runner::Policy::build().bounded_by(HELPER_WORK_BOUND),
        cargo.inherited_lease()?,
    );
    Ok(())
}

/// A killed helper's nested writer keeps both the lease and its write access.
#[test]
#[serial]
fn a_nested_writer_cannot_outlive_a_killed_helper() -> Result<()> {
    let scratch = TempDir::new().context("create the helper scratch directory")?;
    let scratch_directory = Dir::open_ambient_dir(scratch.path(), ambient_authority())
        .context("open the helper scratch directory")?;
    let mut helper = Helper::spawn_for(NESTED_BUILD_HELPER, scratch.path())?;
    wait_for_marker(
        &mut helper,
        &scratch_directory,
        "lease_held",
        HELPER_STARTUP,
    )?;
    let target = announced_target(&scratch_directory)?;
    let target_directory =
        Dir::open_ambient_dir(&target, ambient_authority()).context("open the leased target")?;
    wait_for_heartbeat(&mut helper, &target_directory, 2, HELPER_STARTUP)?;

    // The snapshot has to be taken while the helper is still alive: killing it
    // re-parents its children, and their parent links are the only way to find
    // them afterwards.
    #[cfg(unix)]
    let descendants = snapshot_descendants(helper.pid())?;
    helper.kill_alone()?;
    let observation = Observation {
        target,
        target_directory,
        #[cfg(unix)]
        descendants,
    };
    observation.assert_nested_writer_was_removed()
}

/// What the parent still holds once the helper has been killed.
struct Observation {
    /// The leased target, for replacement acquisitions.
    target: PathBuf,
    /// A capability handle on the target, for reading the heartbeat.
    target_directory: Dir,
    /// The helper's descendants, recorded before it was killed.
    #[cfg(unix)]
    descendants: Descendants,
}

impl Observation {
    /// The heartbeat must stop advancing after the nested writer is gone.
    fn ensure_heartbeat_is_frozen(&self) -> Result<()> {
        let settled = heartbeat_length(&self.target_directory);
        ensure!(
            heartbeat_after(&self.target_directory, FROZEN_WINDOW) == settled,
            "the target heartbeat should stop once the nested writer has been terminated",
        );
        Ok(())
    }

    /// A replacement must wait for the orphan, then succeed once it is gone.
    #[cfg(unix)]
    fn assert_nested_writer_was_removed(&self) -> Result<()> {
        ensure!(
            !self.descendants.is_empty(),
            "the snapshot should record the helper's nested writer",
        );
        let before = heartbeat_length(&self.target_directory);
        ensure!(
            heartbeat_after(&self.target_directory, ORPHAN_WINDOW) > before,
            "the nested writer should still be writing to the target after the helper was killed",
        );
        // This is the gap the lease's inherited handle closes: the helper's own
        // handle closed with it, yet the target must stay refused while the
        // orphan it left behind can still write.
        let _refusal = acquire(&self.target, Instant::now() + REFUSAL_WINDOW)
            .err()
            .context(
                "a replacement acquired the target while the orphaned writer still held its lease",
            )?;
        self.descendants.terminate()?;
        let _replacement = acquire(&self.target, Instant::now() + REPLACEMENT_WINDOW)
            .context("the target should be acquirable once the orphaned writer is terminated")?;
        self.ensure_heartbeat_is_frozen()
    }

    /// Closing the helper's job must terminate the writer and free the target.
    #[cfg(not(unix))]
    fn assert_nested_writer_was_removed(&self) -> Result<()> {
        // No snapshot is taken on Windows: the helper's Job Object was created
        // with `KILL_ON_JOB_CLOSE`, so terminating the helper already
        // terminated the tree beneath it.
        let _replacement = acquire(&self.target, Instant::now() + REPLACEMENT_WINDOW)
            .context("closing the helper's job object should release the inherited lease")?;
        self.ensure_heartbeat_is_frozen()
    }
}
