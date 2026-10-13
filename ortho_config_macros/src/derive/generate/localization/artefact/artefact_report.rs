//! Opt-in build-time observation of identifier artefact publication.
//!
//! A procedural macro runs inside the compiler process, which owns no
//! application subscriber and no logger a build maintainer can configure, so
//! `tracing` instrumentation here would be unobservable by construction: the
//! crate is `proc-macro = true` and links no runtime. This channel writes its
//! observations where a maintainer is already looking — the artefact directory
//! below `OUT_DIR` — and only when the build explicitly opts in.
//!
//! Three files carry the channel, each answering a different question in its
//! own module:
//!
//! * [`artefact_wait_marker`](super::artefact_wait_marker) answers "is this
//!   publication blocked on the lock?" with a marker that exists while it
//!   waits.
//! * This module answers "what did this process's publication do?" with one
//!   dated report per compiler process.
//! * [`artefact_channel_log`](super::artefact_channel_log) answers "did the
//!   channel itself lose an observation?" with one record per lost write.
//!
//! None of the three is part of the identifier inventory and none is matched by
//! the renderer's artefact-name matcher, so pruning leaves all three alone.
//!
//! # Report identity and freshness
//!
//! A report is named for the process that wrote it, not for the publication it
//! describes, and each publication **replaces** what that process reported
//! before. The last publication a compiler process completes is therefore the
//! one a reader finds. Three consequences follow, and each is a contract rather
//! than an accident:
//!
//! * A reader cannot assume the file describes the build it is diagnosing. The
//!   report dates itself in `written_at_ms` so its age is stated rather than
//!   inferred from the file's existence, and it names the crate, deriving type,
//!   and source file of the expansion it does describe.
//! * A replacement that fails leaves the *earlier* report in place: the write
//!   is atomic, so a failed rename never truncates the destination. The
//!   timestamp is what separates "the last publication could not report" from
//!   "the report is current".
//! * Process ids are reused, so a stale report can carry the id of a later
//!   build's process. Its timestamp and expansion are then the only evidence
//!   that it describes the earlier one, which is exactly what lets a reader set
//!   it aside.
//!
//! The channel is best effort: it exists to explain builds and must never fail
//! one, so a report that cannot be written leaves the publication's own result
//! untouched. That is a promise about the *publication*, not licence to drop a
//! failure silently. A writer that quietly discards its error turns a broken
//! channel into a channel that merely looks quiet, and the two are
//! indistinguishable to the maintainer who opted in. Each discarded failure is
//! therefore recorded in the bounded channel log — the last observation to be
//! written, and the one that says the rest cannot be trusted.

use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::Serialize;

use super::artefact_channel_log::write_failure;
use super::atomic_write;

/// Names the environment variable that opts a build into this channel.
pub(super) const REPORT_ENV: &str = "ORTHO_CONFIG_ARTEFACT_REPORT";

/// Prefixes the file recording one publication's observation.
///
/// The publishing process's id is appended, so concurrent publications of one
/// package write to distinct files instead of overwriting one another. The id
/// is in the filename rather than only in the body because the collision is on
/// the path: two writers told to produce one name still race, whatever their
/// reports say about themselves.
pub(super) const REPORT_STEM: &str = "cli-identifiers.build-report";

/// Names the wait after which a blocked acquisition announces itself.
///
/// Re-exported from [`artefact_wait_marker`](super::artefact_wait_marker),
/// which owns the marker and therefore owns its timing.
pub(super) use super::artefact_wait_marker::{WAIT_REANNOUNCE, WAIT_WARN_THRESHOLD};

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

/// Returns whether an artefact-emission environment value explicitly opts in.
fn requested_value(value: Option<&str>) -> bool {
    value == Some("1")
}

/// Carries the observation inputs `emit` resolved at the entry boundary.
///
/// Grouping the values keeps the publication boundary's parameter list honest
/// and lets a test enable the channel, or shorten its warning threshold, without
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
        Self::with_timing(reporting, WAIT_WARN_THRESHOLD, WAIT_REANNOUNCE)
    }

    /// Describes a build with an explicit announcement threshold and refresh.
    ///
    /// Production always uses [`Self::observed`]; this exists so a test can
    /// reach the announcement and refresh paths inside a test's budget instead
    /// of spending the production five seconds genuinely blocked, and so the
    /// helper process that proves a *real* contention announces itself can do
    /// the same while a peer really holds the lock.
    pub(super) const fn with_timing(
        reporting: Reporting,
        threshold: Duration,
        refresh: Duration,
    ) -> Self {
        Self {
            reporting,
            threshold,
            refresh,
        }
    }
}

/// Records one publication's stage, durations, and bounded outcome.
#[derive(Debug, Serialize)]
pub(super) struct PublicationReport<'a> {
    /// Publishing process, so a reader can tell whose report it is holding.
    pub(super) pid: u32,
    /// Crate whose expansion published, matching the inventory's own records.
    pub(super) crate_name: &'a str,
    /// Deriving type, naming the expansion among the crate's several.
    pub(super) ident: &'a str,
    /// Recorded source file of the expansion, as the fragment names it.
    pub(super) source_file: &'a str,
    /// Stage the publication reached; `publish` means it completed.
    pub(super) stage: Stage,
    /// Bounded classification of how the publication ended.
    pub(super) outcome: Outcome,
    /// Bounded category of the stage's failure, when it failed.
    ///
    /// Present only on a failed publication: a success has no error to
    /// classify, and an always-present null would invite a reader to treat the
    /// field as meaningful on every row.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) error: Option<ErrorCategory>,
    /// Time spent waiting for the artefact lock, in whole milliseconds.
    #[serde(rename = "lock_wait_ms", serialize_with = "serialize_millis")]
    pub(super) lock_wait: Duration,
    /// Time from the first lock attempt to the publication's return, in whole
    /// milliseconds.
    #[serde(rename = "elapsed_ms", serialize_with = "serialize_millis")]
    pub(super) elapsed: Duration,
    /// Wall-clock write time, in whole milliseconds since the Unix epoch.
    ///
    /// A report is replaced by each publication its process completes, and a
    /// failed replacement leaves the earlier one in place, so presence says
    /// nothing about age. This is the field that does.
    pub(super) written_at_ms: u64,
}

/// Classifies how a publication's failing stage failed.
///
/// A bounded category rather than the error's text. The text is a spanned
/// `syn::Error` for the compiler's benefit; a harness reading many reports
/// needs something it can group and count across builds. Each step names the
/// category it can fail with as it is observed, so the value describes the
/// operation that failed rather than being reconstructed from its message.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum ErrorCategory {
    /// The filesystem refused an operation: a missing path, permissions, space.
    Io,
    /// A value could not be serialized into, or parsed out of, a file.
    Serialization,
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
pub(super) fn millis(value: Duration) -> u64 {
    u64::try_from(value.as_millis()).unwrap_or(u64::MAX)
}

/// Returns the wall clock as whole milliseconds since the Unix epoch.
///
/// A clock before the epoch cannot occur on a supported platform, and a report
/// that cannot be dated is still worth writing, so an unreadable clock records
/// the epoch rather than failing a build the channel exists only to explain.
pub(super) fn epoch_millis() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, millis)
}

/// Names one process-scoped channel file below an artefact root.
///
/// The report and the wait marker share this rule, so the spelling of the
/// process suffix lives here rather than in each of their constructors. The
/// channel log takes the same suffix under a different extension, because it
/// appends records rather than holding one document.
fn process_file(root: &Path, stem: &str, pid: u32) -> PathBuf {
    root.join(format!("{stem}.{pid}.json"))
}

/// Returns this process's build-report path below one artefact root.
pub(super) fn report_path(root: &Path) -> PathBuf {
    process_file(root, REPORT_STEM, std::process::id())
}

/// Returns the build-report path one process writes below an artefact root.
///
/// Tests hold a helper process's id rather than its `std::process::id()`, so
/// they need the naming rule without the ambient answer. The rule itself is
/// what keeps two compiler processes apart, which is a property worth checking
/// without spawning a process to check it.
#[cfg(test)]
pub(super) fn report_path_for(root: &Path, pid: u32) -> PathBuf {
    process_file(root, REPORT_STEM, pid)
}

/// Writes one publication's observation, replacing this process's last one.
///
/// Failure behaviour is deliberate and is the contract for this channel: it
/// explains builds and must not break them. An unwritable directory or a full
/// disk leaves the publication's own result untouched. The failure is recorded
/// through [`write_failure`] so the absent report is accompanied by the reason
/// it is absent.
///
/// The report path is process-scoped rather than lock-scoped, and that is what
/// makes its temporary safe. `atomic_write` derives the temporary from the
/// destination, so two writers of one path would collide on it — and the
/// acquisition-failure path reaches here without ever holding the artefact
/// lock. Neither can happen: the destination already names exactly one process,
/// and one process runs one publication at a time. The lock is irrelevant to
/// this write for the same reason.
///
/// A publication replaces whatever this process last reported, so a build with
/// several expansions leaves the report of its last one. See the module
/// documentation for what a reader may and may not conclude from that.
pub(super) fn write(root: &Path, report: &PublicationReport<'_>) {
    match serde_json::to_vec(report) {
        Ok(contents) => {
            if let Err(error) = atomic_write(&report_path(root), &contents) {
                write_failure(
                    root,
                    "cannot write the build report",
                    ErrorCategory::Io,
                    &error,
                );
            }
        }
        Err(error) => write_failure(
            root,
            "cannot serialize the build report",
            ErrorCategory::Serialization,
            &error,
        ),
    }
}
