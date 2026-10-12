//! Opt-in build-time observation of identifier artefact publication.
//!
//! A procedural macro runs inside the compiler process, which owns no
//! application subscriber and no logger a build maintainer can configure, so
//! `tracing` instrumentation here would be unobservable by construction: the
//! crate is `proc-macro = true` and links no runtime. This channel writes its
//! observations where a maintainer is already looking — the artefact directory
//! below `OUT_DIR` — and only when the build explicitly opts in.
//!
//! Two files carry the channel, and they answer different questions:
//!
//! * `cli-identifiers.lock-wait.<pid>.json` exists only *while* a publication
//!   is blocked on the artefact lock. A build that appears hung can therefore
//!   be diagnosed as waiting rather than deadlocked, and the wait is observable
//!   before the lock is ever granted. A reader looking for a stall globs
//!   `cli-identifiers.lock-wait.*.json`.
//! * `cli-identifiers.build-report.json` records one completed publication:
//!   the stage it reached, the lock-wait and total durations, and a bounded
//!   outcome category.
//!
//! Neither file is part of the identifier inventory and neither is matched by
//! the renderer's artefact-name matcher, so pruning leaves both alone. Both are
//! best effort: the channel exists to explain builds, so a report that cannot
//! be written leaves the publication's own result untouched and the absent file
//! is itself the observation.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::Serialize;

use super::{atomic_write, requested_value};

/// Names the environment variable that opts a build into this channel.
pub(super) const REPORT_ENV: &str = "ORTHO_CONFIG_ARTEFACT_REPORT";

/// Names the file recording one completed publication.
pub(super) const REPORT_FILE: &str = "cli-identifiers.build-report.json";

/// Prefixes the per-process file that exists only while a publication waits.
///
/// The process id is appended to the stem rather than recorded in the body
/// alone. Two publishers that are both blocked write this marker while neither
/// holds the lock, so a single shared name could only be written through a
/// shared temporary — the very collision a unique filename exists to avoid.
pub(super) const WAIT_STEM: &str = "cli-identifiers.lock-wait";

/// Names the wait after which a blocked acquisition announces itself.
///
/// Several expansions serialise well inside this window on an ordinary machine,
/// so a contended build stays quiet; a maintainer watching a stalled build sees
/// the cause long before reaching for a profiler.
pub(super) const WAIT_WARN_THRESHOLD: Duration = Duration::from_secs(5);

/// Names how often a still-blocked waiter refreshes its announcement.
///
/// The marker is rewritten rather than written once so a long stall keeps
/// showing a growing wait, and on an interval rather than every poll so a
/// blocked build does not spin on the marker file.
pub(super) const WAIT_REANNOUNCE: Duration = Duration::from_secs(1);

/// Classifies how far a publication reached, so a failure names its stage.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum Stage {
    /// Waiting for the artefact lock.
    LockWait,
    /// Writing this expansion's fragment.
    FragmentWrite,
    /// Reading every current fragment into one entry set.
    Merge,
    /// Rendering the merged entry set into named files.
    Render,
    /// Removing rendered files the current render does not produce.
    Prune,
    /// Writing the rendered files.
    Publish,
}

/// Classifies a finished publication with a bounded, machine-readable outcome.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum Outcome {
    /// Every step ran and the inventory names the current entry set.
    Completed,
    /// A step failed; the report's `stage` names where.
    Failed,
}

/// Reports whether a build opted into the observation channel.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Reporting {
    /// The default: nothing is written and no marker is ever created.
    Off,
    /// The build set [`REPORT_ENV`] to the literal `1`.
    On,
}

impl Reporting {
    /// Resolves the opt-in from an environment value.
    pub(super) fn requested(value: Option<&str>) -> Self {
        if requested_value(value) {
            Self::On
        } else {
            Self::Off
        }
    }

    /// Reports whether this build writes observations.
    pub(super) const fn is_on(self) -> bool {
        matches!(self, Self::On)
    }
}

/// Carries the observation inputs `emit` resolved at the entry boundary.
///
/// Grouping the two keeps the publication boundary's parameter list honest and
/// lets a test enable the channel, or shorten its warning threshold, without
/// mutating the process environment.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct ReportConfig {
    /// Whether this build opted into the channel.
    pub(super) reporting: Reporting,
    /// Wait after which a blocked acquisition announces itself.
    pub(super) threshold: Duration,
    /// Minimum gap between subsequent announcements of one continuing wait.
    pub(super) refresh: Duration,
}

impl ReportConfig {
    /// Describes a build that writes no observations.
    ///
    /// Production never constructs this directly: `emit` builds the real
    /// configuration from the environment, and a quiet build is simply one
    /// whose opt-in resolved to [`Reporting::Off`]. The constructor exists so
    /// a test can drive the production boundary with reporting explicitly
    /// disabled, which is what pins the "a normal build stays quiet" promise.
    #[cfg(test)]
    pub(super) const fn quiet() -> Self {
        Self::observed(Reporting::Off)
    }

    /// Describes a build with the given opt-in and default timing.
    pub(super) const fn observed(reporting: Reporting) -> Self {
        Self {
            reporting,
            threshold: WAIT_WARN_THRESHOLD,
            refresh: WAIT_REANNOUNCE,
        }
    }
}

/// Records the process and elapsed wait a blocked publication reached.
#[derive(Debug, Serialize)]
struct WaitMarker {
    /// The waiting process, so a marker left by a dead one can be told apart.
    pid: u32,
    /// Time already spent waiting when the marker was written.
    waited_ms: u64,
}

/// Records one publication's stage, durations, and bounded outcome.
#[derive(Debug, Serialize)]
pub(super) struct PublicationReport<'a> {
    /// Crate whose expansion published, matching the inventory's own records.
    pub(super) crate_name: &'a str,
    /// Stage the publication reached; `publish` means it completed.
    pub(super) stage: Stage,
    /// Bounded classification of how the publication ended.
    pub(super) outcome: Outcome,
    /// Time spent waiting for the artefact lock, in whole milliseconds.
    #[serde(rename = "lock_wait_ms", serialize_with = "serialize_millis")]
    pub(super) lock_wait: Duration,
    /// Time from the first lock attempt to the publication's return, in whole
    /// milliseconds.
    #[serde(rename = "elapsed_ms", serialize_with = "serialize_millis")]
    pub(super) elapsed: Duration,
}

/// Serializes a duration as whole milliseconds.
///
/// Serde's default `Duration` encoding is an object of seconds and
/// nanoseconds, which a build maintainer or a harness reading the report would
/// have to unpack. The wait marker already reports `waited_ms`, so both files
/// of the channel spell a duration the same way.
fn serialize_millis<S>(value: &Duration, serializer: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    serializer.serialize_u64(millis(*value))
}

/// Converts a duration to whole milliseconds, saturating rather than wrapping.
///
/// A wait beyond `u64::MAX` milliseconds cannot occur in a build, so saturation
/// records "very long" where a narrowing cast would report a wrapped small
/// number and understate the stall a maintainer is trying to explain.
fn millis(value: Duration) -> u64 {
    u64::try_from(value.as_millis()).unwrap_or(u64::MAX)
}

/// Returns this process's wait-marker path below one artefact root.
pub(super) fn wait_marker(root: &Path) -> PathBuf {
    root.join(format!("{WAIT_STEM}.{}.json", std::process::id()))
}

/// Publishes bytes atomically through a temporary beside the destination.
///
/// This is deliberately not `atomic_write`: that helper derives its temporary
/// from the destination alone, and the wait marker is written precisely while
/// the artefact lock is *not* held, so two blocked publishers would collide on
/// one shared temporary name. Deriving the temporary from the destination,
/// which is already process-private, keeps both writes independent.
fn write_private_atomic(path: &Path, contents: &[u8]) -> std::io::Result<()> {
    let temporary = path.with_extension("json.tmp");
    fs::write(&temporary, contents)?;
    fs::rename(temporary, path)
}

/// Announces that a publication has been waiting for the lock for too long.
///
/// Only an opted-in build reaches here. The write is best effort: the marker
/// explains a stalled build but must not be able to fail one, so a write error
/// leaves the publisher waiting silently and the wait still ends when the lock
/// is granted.
pub(super) fn note_wait(root: &Path, waited: Duration) {
    let marker = WaitMarker {
        pid: std::process::id(),
        waited_ms: millis(waited),
    };
    if let Ok(contents) = serde_json::to_vec(&marker) {
        drop(write_private_atomic(&wait_marker(root), &contents));
    }
}

/// Removes this process's wait marker once it holds the lock.
///
/// Absence is the normal case for a build that never waited, so a missing file
/// is discarded rather than reported.
pub(super) fn clear_wait(root: &Path) {
    drop(fs::remove_file(wait_marker(root)));
}

/// Writes one publication's observation, discarding any write failure.
///
/// Failure behaviour is deliberate and is the contract for this channel: it
/// explains builds and must not break them. An unwritable directory or a full
/// disk leaves the publication's own result untouched, and the absent report is
/// itself the observation a maintainer finds.
///
/// The caller holds the artefact lock, so the shared temporary `atomic_write`
/// derives from the destination cannot collide here.
pub(super) fn write(root: &Path, report: &PublicationReport<'_>) {
    if let Ok(contents) = serde_json::to_vec(report) {
        drop(atomic_write(&root.join(REPORT_FILE), &contents));
    }
}
