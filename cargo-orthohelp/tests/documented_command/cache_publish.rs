//! Publish-path contracts for the documented `cargo orthohelp` runs.
//!
//! Every test here pre-seeds the bridge-cache entry the command publishes
//! over, then drives the real binary. A `--cache` run cannot reach the publish
//! comparison while an entry exists: a hit returns the cached IR before any
//! build, and a miss leaves nothing to compare. The pre-seeded entry therefore
//! meets a run without `--cache`, which builds the bridge and publishes
//! unconditionally.

#[cfg(unix)]
use super::EXIT_FAILURE;
use super::cache_entries::{IR_FILE, clear_cached_ir, find_fixture_ir};
use super::{DOCUMENTED_RUN_MUTEX, build_state, run_documented_command};
use anyhow::{Context, Result, ensure};
use cap_std::{ambient_authority, fs::Dir};
use std::path::{Path, PathBuf};
use std::sync::{MutexGuard, PoisonError};
use std::time::{Duration, Instant, SystemTime};

/// The fixture package every publish-path scenario runs.
const PACKAGE: &str = "hello_world";

/// Arguments that build and publish, deliberately without `--cache`.
const BUILD_ARGS: [&str; 2] = ["--package", PACKAGE];

/// The discovery allowance for the shared documentation build state.
const DISCOVERY_TIMEOUT: Duration = Duration::from_secs(30);

/// Longest wait for the filesystem to stamp a later modification tick.
const TICK_TIMEOUT: Duration = Duration::from_secs(5);

/// Restores a pre-seeded entry when a scenario ends, however it ends.
///
/// A failed scenario must not leave another lane's `--cache` run with an entry
/// it cannot parse, so the original bytes go back on both success and failure.
struct RestoreEntry {
    /// The cache entry directory holding the pre-seeded file.
    dir: Dir,
    /// The bytes the seeding run published.
    original: String,
}

impl Drop for RestoreEntry {
    fn drop(&mut self) {
        let _restored = self.dir.write(IR_FILE, &self.original);
    }
}

/// Removes a pre-seeded FIFO when a scenario ends, however it ends.
///
/// Writing the original bytes back is not an option here: opening a FIFO for
/// writing blocks until a reader appears, so the guard removes it instead.
#[cfg(unix)]
struct RemoveFifo {
    /// The cache entry directory holding the FIFO.
    dir: Dir,
}

#[cfg(unix)]
impl Drop for RemoveFifo {
    fn drop(&mut self) {
        let _removed = self.dir.remove_file(IR_FILE);
    }
}

/// Take the lock that serializes documented runs in this test binary.
fn take_run_lock() -> MutexGuard<'static, ()> {
    DOCUMENTED_RUN_MUTEX
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
}

/// Cargo's target directory, where the bridge cache lives.
fn cache_target() -> Result<PathBuf> {
    let repository = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .context("workspace repository")?;
    let state = build_state::BuildState::discover(repository, DISCOVERY_TIMEOUT)?;
    Ok(state.target)
}

/// Clear stale fixture entries, run one build, and return the entry it
/// published.
///
/// Clearing first is what makes the returned path the entry the next run will
/// compare against: an older tool version's entry for the same fixture would
/// otherwise match the application name while the current run publishes
/// somewhere else.
fn seed_cached_ir(target: &Path) -> Result<PathBuf> {
    clear_cached_ir(target, PACKAGE)?;
    let run = run_documented_command(&BUILD_ARGS, &[])?;
    run.ensure_success("cold publish run")?;
    find_fixture_ir(target, PACKAGE)?
        .with_context(|| format!("the cold run should publish cached IR for {PACKAGE}"))
}

/// Open the directory holding the fixture's `ir.json`.
fn open_entry_dir(ir_path: &Path) -> Result<Dir> {
    let entry_dir = ir_path.parent().context("cache entry directory")?;
    Ok(Dir::open_ambient_dir(entry_dir, ambient_authority())?)
}

/// Read the modification time of an entry's `ir.json`.
fn read_mtime(dir: &Dir) -> Result<SystemTime> {
    Ok(dir.metadata(IR_FILE)?.modified()?.into_std())
}

/// Wait until a probe write is stamped later than `previous`.
///
/// A filesystem may stamp modification times in coarse ticks, so waiting on
/// the wall clock does not prove that a later rewrite would be stamped later.
/// Writing a probe observes the tick itself on any granularity, which is what
/// lets the mtime assertion tell an untouched entry from a rewritten one.
fn wait_for_a_later_tick(dir: &Dir, previous: SystemTime) -> Result<SystemTime> {
    const PROBE: &str = "mtime-tick-probe";

    let deadline = Instant::now() + TICK_TIMEOUT;
    loop {
        dir.write(PROBE, b"probe")?;
        let observed = dir.metadata(PROBE)?.modified()?.into_std();
        if observed > previous {
            return Ok(observed);
        }
        ensure!(
            Instant::now() < deadline,
            "the filesystem did not stamp a write later than {previous:?} within {TICK_TIMEOUT:?}"
        );
        std::thread::sleep(Duration::from_millis(25));
    }
}

/// A publish replaces an oversized entry instead of reading it whole.
///
/// The in-process test proves the byte bound through a counting reader; this
/// one proves its consequence at the boundary users run: an entry far larger
/// than the regenerated IR is replaced, so the command does not inherit a
/// poisoned cache from a crashed or older writer.
#[test]
fn an_oversized_cached_entry_is_replaced_through_the_cli() -> Result<()> {
    const OVERSIZED_BYTES: usize = 4 * 1024 * 1024;

    let _run_guard = take_run_lock();
    let target = cache_target()?;
    let ir_path = seed_cached_ir(&target)?;
    let entry = open_entry_dir(&ir_path)?;
    let original = entry.read_to_string(IR_FILE).context("read seeded IR")?;
    entry
        .write(IR_FILE, vec![b'x'; OVERSIZED_BYTES])
        .with_context(|| format!("overwrite the entry with {OVERSIZED_BYTES} bytes"))?;
    let restore = RestoreEntry {
        dir: entry,
        original,
    };

    let replacement = run_documented_command(&BUILD_ARGS, &[])?;
    replacement.ensure_success("publish over an oversized entry")?;

    let replaced = restore
        .dir
        .read_to_string(IR_FILE)
        .context("read replaced IR")?;
    ensure!(
        replaced == restore.original,
        "the oversized entry should be replaced with regenerated IR, got {} bytes where the cold run wrote {}",
        replaced.len(),
        restore.original.len()
    );
    Ok(())
}

/// A non-regular cache entry fails the publish at the read stage without
/// blocking.
///
/// The open uses `O_NONBLOCK`, so a FIFO cannot stall the command waiting for
/// a writer, and the documented-run deadline bounds the scenario if that
/// regresses. The FIFO is left in place by the failed publish and removed here
/// by the guard: reporting the entry is the contract, deleting an operator's
/// file is not.
#[cfg(unix)]
#[test]
fn a_fifo_cached_entry_fails_the_publish_without_blocking() -> Result<()> {
    use cap_std::fs::FileTypeExt;
    use nix::sys::stat::Mode;
    use nix::unistd::mkfifo;

    let _run_guard = take_run_lock();
    let target = cache_target()?;
    let ir_path = seed_cached_ir(&target)?;
    let entry = open_entry_dir(&ir_path)?;
    entry
        .remove_file(IR_FILE)
        .with_context(|| format!("remove {}", ir_path.display()))?;
    mkfifo(&ir_path, Mode::S_IRUSR | Mode::S_IWUSR)
        .with_context(|| format!("create FIFO at {}", ir_path.display()))?;
    let remove_fifo = RemoveFifo { dir: entry };

    let failed = run_documented_command(&BUILD_ARGS, &[])?;
    let stderr = String::from_utf8_lossy(&failed.output.stderr);
    ensure!(
        failed.output.status.code() == Some(EXIT_FAILURE),
        "a FIFO cache entry should fail the run, got {:?}:\n{stderr}",
        failed.output.status
    );
    ensure!(
        stderr.contains("cache entry is not a regular file"),
        "the failure should name the read-stage rejection:\n{stderr}"
    );
    ensure!(
        remove_fifo.dir.metadata(IR_FILE)?.file_type().is_fifo(),
        "the failed publish should leave the FIFO in place"
    );
    Ok(())
}

/// Identical content preserves the entry's modification time.
///
/// Waiting for an observed tick is what makes the assertion meaningful: a
/// rewrite stamped within the same coarse tick would leave the mtime unchanged,
/// so the test proves the publish left the file alone rather than merely that
/// the clock had not advanced.
#[test]
fn identical_cached_ir_keeps_its_mtime_through_the_cli() -> Result<()> {
    let _run_guard = take_run_lock();
    let target = cache_target()?;
    let ir_path = seed_cached_ir(&target)?;
    let entry = open_entry_dir(&ir_path)?;
    let published = entry.read_to_string(IR_FILE).context("read seeded IR")?;
    let before = read_mtime(&entry)?;
    let _later_tick = wait_for_a_later_tick(&entry, before)?;

    let repeat = run_documented_command(&BUILD_ARGS, &[])?;
    repeat.ensure_success("republish identical IR")?;

    let after = read_mtime(&entry)?;
    ensure!(
        after == before,
        "an identical publish should not touch the entry: mtime moved from {before:?} to {after:?}"
    );
    ensure!(
        entry
            .read_to_string(IR_FILE)
            .context("read republished IR")?
            == published,
        "the repeated run should leave the published IR byte-identical"
    );
    Ok(())
}
