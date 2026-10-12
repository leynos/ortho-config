//! Shared scaffolding for the artefact lock and publication regression suites.
//!
//! `lock_tests` exercises the lock primitive across real processes;
//! `publication_tests` exercises the production publication sequence. Both need
//! the same helper-process spawn, publication entry point, and observation
//! markers, so that scaffolding lives here rather than being duplicated per
//! suite and drifting apart — which is how the earlier lock tests came to
//! assert against `lock` directly while never observing `publish` at all.
//!
//! The isolated temporary directory is *not* duplicated here: `TempRoot` is
//! owned by `fragment_tests` and already reused by `prune_tests`, so the
//! helpers below take a root path rather than introducing a third copy.
//!
//! `lock_helper_entry_point` is the single re-entry point every helper mode
//! dispatches through, so a mode cannot be reachable from one suite and
//! silently missing from the other. The mode bodies themselves live in
//! [`lock_modes`], which this module re-enters through that point.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Child, Command};
use std::time::{Duration, Instant};

use anyhow::{Context, Result, ensure};

#[path = "artefact_lock_modes.rs"]
mod lock_modes;

/// Names the environment variable that turns this test binary into a helper.
const LOCK_HELPER_ENV: &str = "ORTHO_CONFIG_ARTEFACT_LOCK_HELPER";

/// Names the environment variable carrying the helper's mode.
const LOCK_MODE_ENV: &str = "ORTHO_CONFIG_ARTEFACT_LOCK_MODE";

/// Names the environment variable carrying the helper's artefact root.
const LOCK_ROOT_ENV: &str = "ORTHO_CONFIG_ARTEFACT_LOCK_ROOT";

/// Names the environment variable selecting a publishing helper's variant.
///
/// Separate publishers must differ in both fragment filename and recorded
/// crate, so the merge sees two distinct contributions rather than one
/// fragment rewritten twice.
const LOCK_VARIANT_ENV: &str = "ORTHO_CONFIG_ARTEFACT_LOCK_VARIANT";

/// Names the environment variable keying an announcing helper's files.
///
/// Concurrently running publishers must not share `PUBLISH_STARTED_FILE`, which
/// is a fixed name: the second would find the first's announcement already
/// there and report itself started before it was. The key travels through the
/// environment rather than argv for the reason [`spawn_variant`] records —
/// libtest reads an extra positional argument as another name filter.
const LOCK_KEY_ENV: &str = "ORTHO_CONFIG_ARTEFACT_LOCK_KEY";

/// Names the crate each variant helper records against its entries.
pub(super) const VARIANT_CRATES: [&str; 2] = ["alpha", "beta"];

/// Filters libtest down to the single helper entry point.
///
/// The sibling test modules under `artefact` also match a bare
/// `localization::artefact` filter, so the full path is required.
const LOCK_HELPER_FILTER: &str =
    "derive::generate::localization::artefact::lock_support::lock_helper_entry_point";

/// Names the wait that makes an observed publisher announce promptly.
///
/// Production announces after five seconds, which a test cannot spend
/// genuinely blocked on every run. This is short enough to be reached inside a
/// test's budget while a peer really holds the lock, so the announcement under
/// test is one the production acquisition path made for a real wait rather
/// than one the test wrote on its behalf.
pub(super) const SHORT_WAIT_THRESHOLD: Duration = Duration::from_millis(50);

/// Names the refresh gap the observed helper announces on.
///
/// Short for the same reason as [`SHORT_WAIT_THRESHOLD`], and short enough
/// relative to it that the parent can observe the marker being *rewritten*
/// with a larger wait rather than only written once.
pub(super) const SHORT_WAIT_REFRESH: Duration = Duration::from_millis(100);

/// Records the state a holder leaves in the artefacts directory.
const SENTINEL_FILE: &str = "lock-sentinel.txt";

/// Names the sentinel value written while the holder still owns the lock.
pub(super) const SENTINEL_ACQUIRED: &str = "acquired";

/// Names the sentinel value written just before the holder releases the lock.
pub(super) const SENTINEL_RELEASED: &str = "released";

/// Names the file a parent writes to permit a `hold-until-asked` release.
///
/// The hold window is bounded by the parent rather than by a duration, so a
/// publication test can assert the lock was provably still held at that moment.
pub(super) const RELEASE_REQUEST_FILE: &str = "lock-release-request.txt";

/// Names the marker an announcing publisher writes before it publishes.
pub(super) const PUBLISH_STARTED_FILE: &str = "publish-started.txt";

/// Names the marker an announcing publisher writes after it publishes.
///
/// A lock-honouring publisher cannot write this while a peer holds the lock, so
/// its appearance during a peer's hold is direct evidence that the production
/// sequence never acquired the lock.
pub(super) const PUBLISH_FINISHED_FILE: &str = "publish-finished.txt";

/// Bounds how long a helper holds the lock before releasing it.
const HOLD_PERIOD: Duration = Duration::from_millis(900);

/// Bounds how long a parent waits for a peer to reach a known state.
pub(super) const ACQUIRE_TIMEOUT: Duration = Duration::from_secs(20);

/// Bounds how long a blocked publisher is watched before it is released.
///
/// The publisher has already announced reaching the publication call, so a
/// lock-honouring run cannot finish inside this window however slow the machine
/// is. Only a publisher whose lock is missing completes here — the defect.
pub(super) const BLOCKED_WINDOW: Duration = Duration::from_secs(2);

/// How long a parent sleeps between marker polls.
const POLL_INTERVAL: Duration = Duration::from_millis(25);

/// Reads the holder's sentinel, treating absence as "not yet written".
pub(super) fn sentinel(root: &Path) -> Option<String> {
    fs::read_to_string(root.join(SENTINEL_FILE)).ok()
}

/// Waits for a marker file to appear, reporting whether it did in time.
///
/// Absence is a normal outcome the caller asserts on, not an error.
pub(super) fn wait_for_file(root: &Path, name: &str, timeout: Duration) -> bool {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if root.join(name).exists() {
            return true;
        }
        std::thread::sleep(POLL_INTERVAL);
    }
    false
}

/// Waits until the holder records that it owns the lock.
///
/// Ordering the children this way is what makes the exclusivity assertion
/// meaningful: the contender must arrive while the holder is still inside its
/// hold window, not before the holder has started.
pub(super) fn wait_for_holder(root: &Path) -> Result<()> {
    ensure!(
        wait_for_file(root, SENTINEL_FILE, ACQUIRE_TIMEOUT),
        "holder did not acquire the lock within {ACQUIRE_TIMEOUT:?}",
    );
    Ok(())
}

/// Permits a `hold-until-asked` helper to release the lock.
pub(super) fn request_release(root: &Path) -> Result<()> {
    fs::write(root.join(RELEASE_REQUEST_FILE), "release")
        .context("request the holder's release")?;
    Ok(())
}

/// Returns the completion file one keyed helper writes under a root.
///
/// The key is what keeps two concurrently announced publishers apart: a shared
/// name would let one parent's poll be satisfied by the other child's file.
pub(super) fn finished_file(root: &Path, key: &str) -> PathBuf {
    root.join(format!("{PUBLISH_FINISHED_FILE}.{key}"))
}

/// Spawns this test binary in helper mode against one artefact root.
pub(super) fn spawn_helper(root: &Path, mode: &str) -> Result<Child> {
    spawn_variant(root, mode, None)
}

/// Spawns a helper, optionally recording a variant crate against its entries.
///
/// A one-shot helper needs no announcement key: only one runs at a time, so the
/// shared name cannot collide with a peer.
pub(super) fn spawn_variant(root: &Path, mode: &str, label: Option<&str>) -> Result<Child> {
    spawn_keyed_variant(root, mode, label, None)
}

/// Spawns an observed publisher announcing under a key of its own.
///
/// Two of these run against one artefact root in the report-isolation test, so
/// each needs both a distinct fragment identity — the label — and a distinct
/// announcement name. Without the key the second parent poll would be satisfied
/// by the first child's file.
pub(super) fn spawn_observed_variant(root: &Path, label: &str) -> Result<Child> {
    spawn_keyed_variant(root, "publish-observed", Some(label), Some(label))
}

/// Spawns the observed publisher that announces under no key.
///
/// One real publisher is enough for a test that reads the report it wrote, so
/// the shared announcement name is unambiguous there.
pub(super) fn spawn_observed_publisher(root: &Path) -> Result<Child> {
    spawn_keyed_variant(root, "publish-observed", None, None)
}

/// Spawns a helper with its optional variant label and announcement key.
///
/// The child re-enters through the helper's own test name, so it runs in a
/// genuinely separate process and therefore contends on a genuinely separate
/// lock handle. The mode, label, and key travel through the environment rather
/// than argv: libtest consumes any extra positional argument as an additional
/// name filter, which silently selects zero tests and still exits successfully.
fn spawn_keyed_variant(
    root: &Path,
    mode: &str,
    label: Option<&str>,
    key: Option<&str>,
) -> Result<Child> {
    let executable = std::env::current_exe().context("locate the test executable")?;
    let mut command = Command::new(executable);
    command
        .args(["--exact", "--nocapture", LOCK_HELPER_FILTER])
        .env(LOCK_HELPER_ENV, "1")
        .env(LOCK_MODE_ENV, mode)
        .env(LOCK_ROOT_ENV, root)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());
    if let Some(variant) = label {
        command.env(LOCK_VARIANT_ENV, variant);
    }
    if let Some(announcement) = key {
        command.env(LOCK_KEY_ENV, announcement);
    }
    command
        .spawn()
        .context("spawn artefact lock helper process")
}

/// Waits for a helper child and reports the exit status with its diagnostics.
pub(super) fn wait_for(child: Child, label: &str) -> Result<()> {
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

/// Re-enters the test binary as a lock helper in a separate process.
///
/// Normal test runs return immediately: the helper body only executes when
/// `spawn_helper` sets `LOCK_HELPER_ENV`. The helper lives inside the test
/// binary so it exercises the real `lock` and `publish` implementations rather
/// than copies of them.
#[test]
fn lock_helper_entry_point() -> Result<()> {
    if std::env::var_os(LOCK_HELPER_ENV).is_none() {
        return Ok(());
    }
    let root = std::env::var_os(LOCK_ROOT_ENV)
        .map(PathBuf::from)
        .context("helper requires the artefact lock root")?;
    let mode = std::env::var(LOCK_MODE_ENV).context("helper requires a lock mode")?;
    lock_modes::run(&root, &mode)
}

// The fixture publication lives in `lock_fixture`; re-exported so a suite that
// already imports this module's markers reaches the publisher here too, keeping
// one import per suite. The mode bodies in `lock_modes` and the observed
// variants take it from `lock_fixture` directly.
pub(super) use super::lock_fixture::publish_fixture;
