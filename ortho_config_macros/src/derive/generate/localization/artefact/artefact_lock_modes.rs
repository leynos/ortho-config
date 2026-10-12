//! The helper-process modes a lock or publication test can ask for.
//!
//! `lock_support` owns the parent's side — spawning a helper, waiting for its
//! markers, and the constants both sides agree on — and this module owns what
//! the helper *does* once it is running. Splitting them keeps each readable:
//! the mode bodies exercise the production lock and publication, and they are
//! easier to follow beside one another than interleaved with the parent's
//! polling.
//!
//! This is a child of `lock_support` rather than a sibling under `artefact`,
//! so the mode bodies can use that module's private constants without widening
//! their visibility to every suite in the tree.
//!
//! `lock_support::lock_helper_entry_point` remains the single re-entry point:
//! it resolves the mode and calls [`run`] here, so a mode cannot be reachable
//! from one suite and silently missing from another.

use std::fs;
use std::io::ErrorKind;
use std::path::Path;
use std::time::Instant;

use anyhow::{Context, Result, ensure};

use super::super::artefact_lock::{create_lock, is_free, lock};
use super::super::artefact_probe::{INJECTED_PROBE_MESSAGE, with_metadata_outcome};
use super::super::artefact_report::{ReportConfig, Reporting};
use super::super::lock_fixture::{
    fixture_source_path, publish, publish_fixture, publish_variant, publish_variant_observed,
};
use super::{
    ACQUIRE_TIMEOUT, HOLD_PERIOD, LOCK_KEY_ENV, LOCK_VARIANT_ENV, POLL_INTERVAL,
    PUBLISH_FINISHED_FILE, PUBLISH_STARTED_FILE, RELEASE_REQUEST_FILE, SENTINEL_ACQUIRED,
    SENTINEL_FILE, SENTINEL_RELEASED, SHORT_WAIT_REFRESH, SHORT_WAIT_THRESHOLD, finished_file,
};

/// Runs the helper mode a parent selected through the environment.
pub(super) fn run(root: &Path, mode: &str) -> Result<()> {
    match mode {
        "hold" => hold(root),
        "hold-until-asked" => hold_until_asked(root),
        "contend" => contend(root),
        "publish" => publish(root),
        "publish-variant" => {
            let variant = std::env::var(LOCK_VARIANT_ENV)
                .context("variant publisher requires a variant name")?;
            publish_variant(root, &variant)
        }
        "publish-announced" => publish_announced(root),
        "publish-observed" => publish_observed(root),
        "publish-then-probe" => publish_then_probe(root),
        "publish-failing-then-probe" => publish_failing_then_probe(root),
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
///
/// The lock file is created first, as an explicit command. The probe opens the
/// file that exists rather than bringing one into being, so without this a
/// missing file would be indistinguishable from a released lock. Every probed
/// path has already published, so the file is normally there; creating it
/// keeps the check about the lock rather than about the directory's contents.
fn ensure_free(root: &Path, moment: &str) -> Result<()> {
    create_lock(root).context("create the artefact lock file before probing it")?;
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

/// Announces, then publishes with the channel on and a prompt threshold.
///
/// The parent needs a publisher that is *genuinely* blocked while a peer holds
/// the lock, so that the wait marker it observes was written by the production
/// acquisition path. The threshold is shortened through the explicit
/// configuration rather than the environment, which would change it for every
/// test running concurrently in this binary. The announcement is written before
/// the publication call, so a parent that sees it knows the child is about to
/// contend rather than about to start.
///
/// An optional key names both the announcement files and, through
/// [`publish_variant_observed`], the variant crate the fragment records, so two
/// observed publishers can run against one root without sharing an announcement
/// or overwriting each other's fragment.
fn publish_observed(root: &Path) -> Result<()> {
    let key = std::env::var(LOCK_KEY_ENV).ok();
    let announced = key.as_deref().unwrap_or(PUBLISH_STARTED_FILE);
    fs::write(root.join(announced), "started").context("announce the observed publication call")?;
    let report = ReportConfig::with_timing(Reporting::On, SHORT_WAIT_THRESHOLD, SHORT_WAIT_REFRESH);
    // The key doubles as the variant label, which is what gives each concurrent
    // observed publisher its own fragment filename and recorded crate: without
    // it the second would rewrite the first's fragment rather than adding its
    // own, and both reports would name one expansion.
    match key.as_deref() {
        Some(label) => publish_variant_observed(root, label, report)?,
        None => publish_fixture(root, report)?,
    }
    let finished = key.as_deref().map_or_else(
        || root.join(PUBLISH_FINISHED_FILE),
        |label| finished_file(root, label),
    );
    fs::write(finished, "finished").context("record observed publication completion")?;
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
