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
//! silently missing from the other.

use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::process::{Child, Command};
use std::time::{Duration, Instant};

use anyhow::{Context, Result, ensure};

use super::artefact_lock::{is_free, lock};
use super::artefact_probe::{INJECTED_PROBE_MESSAGE, with_metadata_outcome};
use super::lock_fixture::fixture_source_path;

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

/// Names the crate each variant helper records against its entries.
pub(super) const VARIANT_CRATES: [&str; 2] = ["alpha", "beta"];

/// Filters libtest down to the single helper entry point.
///
/// The sibling test modules under `artefact` also match a bare
/// `localization::artefact` filter, so the full path is required.
const LOCK_HELPER_FILTER: &str =
    "derive::generate::localization::artefact::lock_support::lock_helper_entry_point";

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

/// Spawns this test binary in helper mode against one artefact root.
pub(super) fn spawn_helper(root: &Path, mode: &str) -> Result<Child> {
    spawn_variant(root, mode, None)
}

/// Spawns a helper, optionally recording a variant crate against its entries.
///
/// The child re-enters through the helper's own test name, so it runs in a
/// genuinely separate process and therefore contends on a genuinely separate
/// lock handle. The mode travels through the environment rather than argv:
/// libtest consumes any extra positional argument as an additional name filter,
/// which silently selects zero tests and still exits successfully.
pub(super) fn spawn_variant(root: &Path, mode: &str, label: Option<&str>) -> Result<Child> {
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
    match mode.as_str() {
        "hold" => hold(&root),
        "hold-until-asked" => hold_until_asked(&root),
        "contend" => contend(&root),
        "publish" => publish(&root),
        "publish-variant" => {
            let variant = std::env::var(LOCK_VARIANT_ENV)
                .context("variant publisher requires a variant name")?;
            publish_variant(&root, &variant)
        }
        "publish-announced" => publish_announced(&root),
        "publish-then-probe" => publish_then_probe(&root),
        "publish-failing-then-probe" => publish_failing_then_probe(&root),
        "probe" => ensure_free(&root, "after the publisher returned"),
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

/// Acquires the lock and holds it until the parent permits its release.
fn hold_until_asked(root: &Path) -> Result<()> {
    let guard = lock(root)?;
    fs::write(root.join(SENTINEL_FILE), SENTINEL_ACQUIRED).context("record lock ownership")?;
    let deadline = Instant::now() + ACQUIRE_TIMEOUT;
    while Instant::now() < deadline {
        if root.join(RELEASE_REQUEST_FILE).exists() {
            fs::write(root.join(SENTINEL_FILE), SENTINEL_RELEASED)
                .context("record lock release")?;
            drop(guard);
            return Ok(());
        }
        std::thread::sleep(POLL_INTERVAL);
    }
    Err(anyhow::anyhow!(
        "parent never requested the release within {ACQUIRE_TIMEOUT:?}"
    ))
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

/// Fails unless the lock is free, naming the moment being checked.
fn ensure_free(root: &Path, moment: &str) -> Result<()> {
    let free = is_free(root).context("probe the artefact lock")?;
    ensure!(
        free,
        "the lock was still held {moment}, so a later compiler process would \
         block indefinitely",
    );
    Ok(())
}

/// Announces the publication call, publishes, then records completion.
///
/// The announcement precedes the lock acquisition, so a parent that sees it
/// knows the child is about to try the lock. That lets the parent bound the
/// child's opportunity deterministically instead of racing a sleep.
fn publish_announced(root: &Path) -> Result<()> {
    fs::write(root.join(PUBLISH_STARTED_FILE), "started")
        .context("announce the publication call")?;
    publish(root)?;
    fs::write(root.join(PUBLISH_FINISHED_FILE), "finished")
        .context("record publication completion")?;
    Ok(())
}

/// Publishes successfully and then probes, within one process.
///
/// The probe must share the publisher's process: one that waits for the child
/// to exit cannot distinguish a released guard from process exit closing the
/// handle, so it passes even if publication leaked its lock for its lifetime.
fn publish_then_probe(root: &Path) -> Result<()> {
    publish(root)?;
    ensure_free(root, "after a successful publication returned")
}

/// Fails a publication after the lock is taken, then probes in-process.
///
/// The metadata injection makes `merge_fragments` fail, which happens after the
/// fragment write and therefore after the lock is held, so this drives the
/// error path with the guard already acquired. The assertion on the injected
/// marker proves the failure came from the injection rather than from an
/// incidental filesystem condition that would make the fixture vacuous.
fn publish_failing_then_probe(root: &Path) -> Result<()> {
    let source = fixture_source_path(root, "fixture")?;
    let outcome =
        with_metadata_outcome(Path::new(&source), Err(ErrorKind::PermissionDenied), || {
            publish(root)
        });
    let reported = outcome
        .as_ref()
        .err()
        .map(ToString::to_string)
        .unwrap_or_default();
    ensure!(
        reported.contains(INJECTED_PROBE_MESSAGE),
        "the failing-publication fixture must fail because of the injected \
         metadata outcome, but it reported {reported:?}",
    );
    ensure_free(root, "after a failed publication returned")
}

// The fixture publications live in `lock_fixture`; re-exported so every suite
// reaches them as `lock_support::publish*`, keeping one import per suite.
pub(super) use super::lock_fixture::{publish, publish_fixture, publish_variant};
