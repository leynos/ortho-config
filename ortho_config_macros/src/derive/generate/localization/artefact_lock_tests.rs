//! Cross-process regression tests for artefact publication locking.
//!
//! `OUT_DIR` is package-scoped, so a package with several targets can have
//! several compiler processes publishing into one artefact directory. These
//! tests exercise real file locks between real processes rather than
//! in-process threads, because an in-process mutex would satisfy a
//! thread-based test while leaving the compiler-process race intact.
//!
//! The exclusivity test is deliberately not built on wall-clock thresholds.
//! A helper publishes a sentinel while it holds the lock, and the contender
//! reads that sentinel only after acquiring the lock itself. A slow or loaded
//! machine changes how long the contender blocks, never what the contender
//! observes, so the assertion stays meaningful under coverage-instrumented CI.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Child, Command};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use anyhow::{Context, Result, ensure};

use super::*;

/// Names the environment variable that turns this test binary into a helper.
const LOCK_HELPER_ENV: &str = "ORTHO_CONFIG_ARTEFACT_LOCK_HELPER";

/// Names the environment variable carrying the helper's mode.
const LOCK_MODE_ENV: &str = "ORTHO_CONFIG_ARTEFACT_LOCK_MODE";

/// Names the environment variable carrying the helper's artefact root.
const LOCK_ROOT_ENV: &str = "ORTHO_CONFIG_ARTEFACT_LOCK_ROOT";

/// Filters libtest down to the single helper entry point.
///
/// The extra `pub(super)` sibling modules under `artefact` also match the
/// `localization::artefact` filter, so without the full path the child would
/// run the whole fragment and renderer suites as well as the helper.
const LOCK_HELPER_FILTER: &str =
    "derive::generate::localization::artefact::lock_tests::lock_helper_entry_point";

/// Records the state a holder leaves in the artefacts directory.
const SENTINEL_FILE: &str = "lock-sentinel.txt";

/// Names the sentinel value written while the holder still owns the lock.
const SENTINEL_ACQUIRED: &str = "acquired";

/// Names the sentinel value written just before the holder releases the lock.
const SENTINEL_RELEASED: &str = "released";

/// Bounds how long a helper holds the lock before releasing it.
const HOLD_PERIOD: Duration = Duration::from_millis(900);

/// Bounds how long the parent waits for the holder to take the lock.
const ACQUIRE_TIMEOUT: Duration = Duration::from_secs(20);

/// How long the parent sleeps between sentinel polls.
const POLL_INTERVAL: Duration = Duration::from_millis(25);

static TEMP_ROOT_SEQUENCE: AtomicUsize = AtomicUsize::new(0);

/// Owns an isolated temporary directory used by one lock regression test.
struct TempRoot(PathBuf);

impl TempRoot {
    /// Creates a unique directory below the system temporary directory.
    fn new() -> Result<Self> {
        let sequence = TEMP_ROOT_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "ortho-config-artefact-lock-tests-{}-{sequence}",
            std::process::id()
        ));
        fs::create_dir_all(&path).context("create isolated artefact lock test root")?;
        Ok(Self(path))
    }

    /// Returns the root directory available to the current test.
    fn path(&self) -> &Path {
        &self.0
    }

    /// Reads the sentinel, treating a missing file as "not yet written".
    fn sentinel(&self) -> Option<String> {
        fs::read_to_string(self.path().join(SENTINEL_FILE)).ok()
    }

    /// Waits until the holder records that it owns the lock.
    ///
    /// Ordering the two children this way is what makes the exclusivity
    /// assertion meaningful: the contender must arrive while the holder is
    /// still inside its hold window, not before the holder has started.
    fn wait_for_holder(&self) -> Result<()> {
        let deadline = Instant::now() + ACQUIRE_TIMEOUT;
        while Instant::now() < deadline {
            if self.sentinel().as_deref() == Some(SENTINEL_ACQUIRED) {
                return Ok(());
            }
            std::thread::sleep(POLL_INTERVAL);
        }
        Err(anyhow::anyhow!(
            "holder did not acquire the lock within {ACQUIRE_TIMEOUT:?}"
        ))
    }
}

impl Drop for TempRoot {
    fn drop(&mut self) {
        drop(fs::remove_dir_all(&self.0));
    }
}

/// Spawns this test binary in helper mode against one artefact root.
///
/// The child re-enters through the helper's own test name, so the helper runs
/// in a genuinely separate process and therefore contends on a genuinely
/// separate lock handle. The mode travels through the environment rather than
/// argv: libtest consumes any extra positional argument as an additional name
/// filter, which silently selects zero tests and still exits successfully.
fn spawn_helper(root: &Path, mode: &str) -> Result<Child> {
    let executable = std::env::current_exe().context("locate the test executable")?;
    let child = Command::new(executable)
        .args(["--exact", "--nocapture", LOCK_HELPER_FILTER])
        .env(LOCK_HELPER_ENV, "1")
        .env(LOCK_MODE_ENV, mode)
        .env(LOCK_ROOT_ENV, root)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .context("spawn artefact lock helper process")?;
    Ok(child)
}

/// Waits for a helper child and reports the exit status with its diagnostics.
fn wait_for(child: Child, label: &str) -> Result<()> {
    let output = child
        .wait_with_output()
        .with_context(|| format!("wait for {label}"))?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    ensure!(
        output.status.success(),
        "{label} must succeed; stdout: {stdout}; stderr: {stderr}"
    );
    // A filtered-out helper still exits successfully, so require proof that the
    // helper body actually ran rather than trusting the exit status alone.
    ensure!(
        stdout.contains("1 passed"),
        "{label} must have run the helper body; stdout: {stdout}",
    );
    Ok(())
}

/// Verifies the lock excludes a second process for as long as it is held.
///
/// The holder advertises ownership through the sentinel while still holding
/// the lock. A contender that acquires the lock must therefore observe the
/// released value; observing "acquired" proves the two processes overlapped.
#[test]
fn artefact_lock_is_exclusive_across_processes() -> Result<()> {
    if std::env::var_os(LOCK_HELPER_ENV).is_some() {
        return Ok(());
    }
    let root = TempRoot::new()?;
    let holder = spawn_helper(root.path(), "hold")?;
    root.wait_for_holder()?;

    let contender = spawn_helper(root.path(), "contend")?;
    wait_for(contender, "contending helper")?;
    wait_for(holder, "holding helper")?;
    ensure!(
        root.sentinel().as_deref() == Some(SENTINEL_RELEASED),
        "holder must record its release before exiting",
    );
    Ok(())
}

/// Verifies the lock is released when a publication path returns.
///
/// The reader uses a non-blocking acquisition, so a lock left held fails the
/// assertion immediately instead of hanging the suite.
#[test]
fn artefact_lock_is_released_after_publication_returns() -> Result<()> {
    if std::env::var_os(LOCK_HELPER_ENV).is_some() {
        return Ok(());
    }
    let root = TempRoot::new()?;
    let writer = spawn_helper(root.path(), "publish")?;
    wait_for(writer, "publishing helper")?;

    let reader = spawn_helper(root.path(), "probe");
    wait_for(reader?, "probing helper")?;
    Ok(())
}

/// Re-enters the test binary as a lock helper in a separate process.
///
/// Normal test runs return immediately: the helper body only executes when
/// `spawn_helper` sets `LOCK_HELPER_ENV`. The helper lives inside the test
/// binary so it exercises the real `lock` implementation rather than a copy of
/// it.
#[test]
fn lock_helper_entry_point() -> Result<()> {
    if std::env::var_os(LOCK_HELPER_ENV).is_none() {
        return Ok(());
    }
    let root = std::env::var_os(LOCK_ROOT_ENV)
        .map(PathBuf::from)
        .context("helper requires the artefact lock root")?;
    let mode = std::env::var(LOCK_MODE_ENV).context("helper requires a lock mode")?;
    match mode.as_str() {
        "hold" => hold(&root),
        "contend" => contend(&root),
        "publish" => publish(&root),
        "probe" => probe(&root),
        other => Err(anyhow::anyhow!("unknown helper mode {other}")),
    }
}

/// Acquires the lock, advertises ownership, then releases it.
fn hold(root: &Path) -> Result<()> {
    let guard = lock(root)?;
    fs::write(root.join(SENTINEL_FILE), SENTINEL_ACQUIRED).context("record lock ownership")?;
    std::thread::sleep(HOLD_PERIOD);
    // Write the release marker while the guard is still held, so every later
    // acquirer is guaranteed to observe it.
    fs::write(root.join(SENTINEL_FILE), SENTINEL_RELEASED).context("record lock release")?;
    drop(guard);
    Ok(())
}

/// Acquires the lock and fails when a holder was still advertising ownership.
fn contend(root: &Path) -> Result<()> {
    let _guard = lock(root)?;
    let observed = fs::read_to_string(root.join(SENTINEL_FILE))
        .context("contender requires the holder's sentinel")?;
    ensure!(
        observed == SENTINEL_RELEASED,
        "contender acquired the lock while the holder still reported it owned it; \
         observed {observed:?}, which shows no inter-process exclusion held",
    );
    Ok(())
}

/// Fails unless the lock is free, without blocking on a stuck holder.
fn probe(root: &Path) -> Result<()> {
    let free = is_free(root).context("probe the artefact lock")?;
    ensure!(
        free,
        "lock was still held after the publisher returned, so a later compiler \
         process would block indefinitely",
    );
    Ok(())
}

/// Runs a full fragment publication so the lock wraps real emission work.
fn publish(root: &Path) -> Result<()> {
    let entry = Entry {
        id: String::from("lock-helper-about"),
        kind: String::from("about"),
        type_name: String::from("fixture::Config"),
        field: None,
        path_scope: String::from("standalone"),
        source: Source {
            file: String::from("fixture.rs"),
            line: 1,
            column: 0,
        },
        embedded_default: None,
    };
    let fragment = Fragment {
        source_file: String::from("fixture.rs"),
        entries: vec![entry],
    };
    let _guard = lock(root)?;
    fs::create_dir_all(root.join(FRAGMENT_DIR)).context("create fragment directory")?;
    fs::write(
        root.join(FRAGMENT_DIR).join("helper.json"),
        json(&fragment)?,
    )
    .context("write helper fragment")?;
    for file in render(merge_fragments(root).map_err(anyhow::Error::msg)?)? {
        atomic_write(&root.join(file.name), &file.contents).context("publish rendered artefact")?;
    }
    Ok(())
}
