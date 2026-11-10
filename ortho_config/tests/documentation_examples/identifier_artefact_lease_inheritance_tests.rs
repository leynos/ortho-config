//! A nested writer keeps the target leased after its holder has gone.
//!
//! Interrupting a helper test process is not the same as ending the build it
//! started. On Unix the runner gives every nested child its own process group,
//! so the helper's death leaves Cargo and its compilers running. If the lease
//! were owned by the helper alone, that orphaned tree could keep writing to the
//! target while a replacement process acquired the lease and cleaned it.
//!
//! The lease module closes that gap by handing every nested child a duplicate
//! of the lease handle. These regressions prove both halves of the ordering:
//!
//! - [`an_inherited_handle_keeps_the_lease_after_its_holder_releases_it`] shows
//!   that releasing the holder's own handle does not release the lease while a
//!   descendant's inherited duplicate is open, and that the descendant can
//!   still write to the target during that window.
//! - [`the_runner_deadline_kill_releases_an_inherited_lease`] shows that the
//!   runner's own deadline still terminates the child it owns, which closes the
//!   inherited duplicate and frees the target. Handing the child a lease must
//!   not make the build unkillable.
//!
//! Both use the same writer, [`nested_writer_probe`], which appends to a
//! heartbeat file inside the target until it is killed. The heartbeat is the
//! evidence that the writer is alive *and* able to write to the leased
//! directory, rather than merely still in the process table.

use super::identifier_artefact_e2e_support::acquire;
use super::identifier_artefact_lease_support::{
    HEARTBEAT_FILE, HEARTBEAT_INTERVAL, HELPER_STARTUP, Helper, child_test_command,
    heartbeat_after, heartbeat_length, required_scratch, wait_for_heartbeat,
};
use super::process_runner;
use anyhow::{Context, Result, ensure};
use cap_std::ambient_authority;
use cap_std::fs::{Dir, OpenOptions};
use std::io::Write;
use std::path::Path;
use std::time::{Duration, Instant};
use tempfile::TempDir;

/// The qualified name of the writer both regressions drive.
const WRITER_PROBE: &str = "identifier_artefact_lease_inheritance_tests::nested_writer_probe";

/// The envelope an uncontended acquisition in these regressions may use.
///
/// Both tests lease a fresh temporary directory, so the only wait is the one
/// being measured; this bound exists to fail with a diagnostic rather than to
/// accommodate a busy host.
const LEASE_ENVELOPE: Duration = Duration::from_secs(30);

/// How long a replacement keeps trying while the inherited handle is open.
///
/// Long enough for the acquisition loop to poll several times, so a refusal is
/// the lease being held rather than a single unlucky attempt.
const REFUSAL_WINDOW: Duration = Duration::from_millis(500);

/// How long a replacement may take once the inherited handle has closed.
const REPLACEMENT_WINDOW: Duration = Duration::from_secs(10);

/// The deadline the runner applies to a writer that never exits.
const RUNNER_DEADLINE: Duration = Duration::from_secs(5);

/// How long the heartbeat is watched for further growth after the writer ends.
const FROZEN_WINDOW: Duration = Duration::from_millis(400);

/// Append to the leased target's heartbeat until killed.
///
/// Ignored so an ordinary run never starts it; the regressions below select it
/// by exact name when they re-run this binary as their own child. It inherits
/// whatever standard input its parent gave it, which is the lease handle under
/// test, and never reads it.
#[test]
#[ignore = "driven as a child process by the lease-inheritance regressions"]
fn nested_writer_probe() -> Result<()> {
    let scratch = required_scratch()?;
    let target_text = scratch
        .read_to_string("target.txt")
        .context("read the leased target path")?;
    let target = Dir::open_ambient_dir(Path::new(target_text.trim()), ambient_authority())
        .context("open the leased target")?;
    // Truncating once, before the first beat, means a heartbeat left by an
    // earlier run cannot be mistaken for this writer's progress. A single
    // writer owns the file, so the offset can simply advance from here.
    let mut heartbeat = target
        .open_with(
            HEARTBEAT_FILE,
            OpenOptions::new().create(true).write(true).truncate(true),
        )
        .context("open the leased target's heartbeat")?;
    loop {
        heartbeat
            .write_all(b".")
            .context("append to the leased target's heartbeat")?;
        std::thread::sleep(HEARTBEAT_INTERVAL);
    }
}

/// Releasing the holder's handle must not release a lease a child inherited.
#[test]
fn an_inherited_handle_keeps_the_lease_after_its_holder_releases_it() -> Result<()> {
    let target = TempDir::new().context("create the lease-inheritance target")?;
    let target_directory =
        Dir::open_ambient_dir(target.path(), ambient_authority()).context("open the target")?;
    let scratch = TempDir::new().context("create the lease-inheritance scratch directory")?;
    let scratch_directory =
        Dir::open_ambient_dir(scratch.path(), ambient_authority()).context("open the scratch")?;
    scratch_directory
        .write("target.txt", target.path().display().to_string())
        .context("announce the target to the writer")?;

    let lease = acquire(target.path(), Instant::now() + LEASE_ENVELOPE)?;
    let mut writer = Helper::spawn_for_with_stdin(
        WRITER_PROBE,
        scratch.path(),
        Some(lease.inherited_handle()?),
    )?;
    wait_for_heartbeat(&mut writer, &target_directory, 2, HELPER_STARTUP)?;

    // This process now releases its own handle. The child's duplicate is the
    // only thing keeping the lease held.
    drop(lease);
    let before = heartbeat_length(&target_directory);
    let _refusal = acquire(target.path(), Instant::now() + REFUSAL_WINDOW)
        .err()
        .context(
            "a replacement acquired the target lease while an inherited handle was still open",
        )?;
    let during = heartbeat_length(&target_directory);
    ensure!(
        during > before,
        "the nested writer should still be writing to the target while the lease is refused; \
         the heartbeat stayed at {before} bytes",
    );

    // Ending the writer closes the duplicate, and only then may a replacement
    // take the lease and clean the target.
    writer.kill_alone()?;
    let _replacement = acquire(target.path(), Instant::now() + REPLACEMENT_WINDOW)
        .context("the lease should be free once the inherited handle has closed")?;
    let settled = heartbeat_length(&target_directory);
    ensure!(
        heartbeat_after(&target_directory, FROZEN_WINDOW) == settled,
        "the heartbeat should stop when the nested writer is killed",
    );
    Ok(())
}

/// The runner's deadline kill must still terminate a child holding the lease.
#[test]
fn the_runner_deadline_kill_releases_an_inherited_lease() -> Result<()> {
    let target = TempDir::new().context("create the deadline-kill target")?;
    let target_directory =
        Dir::open_ambient_dir(target.path(), ambient_authority()).context("open the target")?;
    let scratch = TempDir::new().context("create the deadline-kill scratch directory")?;
    let scratch_directory =
        Dir::open_ambient_dir(scratch.path(), ambient_authority()).context("open the scratch")?;
    scratch_directory
        .write("target.txt", target.path().display().to_string())
        .context("announce the target to the writer")?;

    let lease = acquire(target.path(), Instant::now() + LEASE_ENVELOPE)?;
    let mut command = child_test_command(WRITER_PROBE, scratch.path())?;
    command.stdin(lease.inherited_handle()?);
    let error = process_runner::run_with_policy(
        &mut command,
        process_runner::Operation("hold the leased target past the runner deadline"),
        process_runner::Policy::build().bounded_by(RUNNER_DEADLINE),
    )
    .err()
    .context("the runner should stop a writer that never exits")?;
    ensure!(
        process_runner::deadline_exceeded(&error),
        "the runner should report a deadline kill rather than another failure:\n{error:#}",
    );
    ensure!(
        heartbeat_length(&target_directory) > 0,
        "the writer should have reached the target before the runner killed it",
    );

    // The runner reaped the child it owned, so its inherited duplicate closed.
    // If the lease were still held here, the build would have been made
    // unkillable by inheriting it.
    drop(lease);
    let _replacement = acquire(target.path(), Instant::now() + REPLACEMENT_WINDOW)
        .context("the runner's deadline kill should have closed the inherited lease handle")?;
    Ok(())
}
