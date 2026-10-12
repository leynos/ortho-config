//! Opt-in build-time observation of identifier artefact publication.
//!
//! A procedural macro runs inside the compiler process, which owns no
//! application subscriber and no logger a build maintainer can configure, so
//! `tracing` instrumentation here would be unobservable by construction: the
//! crate is `proc-macro = true` and links no runtime. This channel writes its
//! observations where a maintainer is already looking — the artefact directory
//! below `OUT_DIR` — and only when the build explicitly opts in.
//!
//! Three files carry the channel, and they answer different questions:
//!
//! * `cli-identifiers.lock-wait.<pid>.json` exists only *while* a publication
//!   is blocked on the artefact lock. A build that appears hung can therefore
//!   be diagnosed as waiting rather than deadlocked, and the wait is observable
//!   before the lock is ever granted. A reader looking for a stall globs
//!   `cli-identifiers.lock-wait.*.json`.
//! * `cli-identifiers.build-report.<pid>.json` records one publication — its
//!   process, crate, stage, lock-wait and total durations, and a bounded
//!   outcome category. One file per process rather than one shared file,
//!   because `OUT_DIR` is package-scoped: a package with several targets has
//!   several compiler processes publishing concurrently, and a single name
//!   would let each overwrite the one before it, leaving a maintainer unable to
//!   tell which publication the surviving report describes. A reader globs
//!   `cli-identifiers.build-report.*.json`.
//! * `cli-identifiers.observation-errors.<pid>.json` appears only when the
//!   channel itself failed to write. See [`write_failure`] for why a
//!   best-effort channel still needs to say when it dropped something.
//!
//! None of the three is part of the identifier inventory and none is matched by
//! the renderer's artefact-name matcher, so pruning leaves all three alone.
//!
//! The channel is best effort: it exists to explain builds and must never fail
//! one, so a report that cannot be written leaves the publication's own result
//! untouched. That is a promise about the *publication*, not licence to drop a
//! failure silently. A writer that quietly discards its error turns a broken
//! channel into a channel that merely looks quiet, and the two are
//! indistinguishable to the maintainer who opted in. Each discarded failure is
//! therefore recorded in the bounded [`write_failure`] file — the last
//! observation to be written, and the one that says the rest cannot be trusted.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::Serialize;

use super::{atomic_write, requested_value};

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

/// Prefixes the file recording observation-channel write failures.
///
/// Process-scoped for the same reason as the report: only the publishing
/// process knows what its own channel dropped.
pub(super) const ERRORS_STEM: &str = "cli-identifiers.observation-errors";

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
fn millis(value: Duration) -> u64 {
    u64::try_from(value.as_millis()).unwrap_or(u64::MAX)
}

/// Joins a process-scoped channel filename onto an artefact root.
///
/// The three channel files share one naming rule, so the spelling of the
/// process suffix lives here rather than in each of the three constructors.
fn process_file(root: &Path, stem: &str) -> PathBuf {
    root.join(format!("{stem}.{}.json", std::process::id()))
}

/// Returns this process's wait-marker path below one artefact root.
pub(super) fn wait_marker(root: &Path) -> PathBuf {
    process_file(root, WAIT_STEM)
}

/// Returns this process's build-report path below one artefact root.
pub(super) fn report_path(root: &Path) -> PathBuf {
    process_file(root, REPORT_STEM)
}

/// Returns this process's observation-error path below one artefact root.
pub(super) fn errors_path(root: &Path) -> PathBuf {
    process_file(root, ERRORS_STEM)
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

/// Records that the observation channel itself could not write.
///
/// The channel's contract is that it never fails a build, so a write error
/// cannot become a `syn::Error` the compilation reports. Discarding it is not
/// the alternative: a channel that silently drops its observations looks
/// exactly like a build with nothing to observe, and a maintainer who opted in
/// would read the empty directory as "all quiet". The failure is therefore
/// appended to a bounded process-scoped file — one line per lost observation,
/// naming the operation and the error — so an opted-in build that could not
/// explain itself says so.
///
/// A failure to write *this* record is discarded: if the directory cannot be
/// written at all, there is nowhere left to record the failure of recording
/// it, and the compilation must still succeed.
pub(super) fn write_failure(root: &Path, operation: &str, error: &dyn std::fmt::Display) {
    let line = format!("{operation}: {error}\n");
    drop(
        fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(errors_path(root))
            .and_then(|mut file| std::io::Write::write_all(&mut file, line.as_bytes())),
    );
}

/// Announces that a publication has been waiting for the lock for too long.
///
/// Only an opted-in build reaches here. The write is best effort: the marker
/// explains a stalled build but must not be able to fail one, so a write error
/// leaves the publisher waiting and the wait still ends when the lock is
/// granted. The error is recorded through [`write_failure`] rather than
/// dropped, so a channel that cannot announce a stall is distinguishable from
/// a build that never stalled.
pub(super) fn note_wait(root: &Path, waited: Duration) {
    let marker = WaitMarker {
        pid: std::process::id(),
        waited_ms: millis(waited),
    };
    match serde_json::to_vec(&marker) {
        Ok(contents) => {
            if let Err(error) = write_private_atomic(&wait_marker(root), &contents) {
                write_failure(root, "cannot write the lock-wait marker", &error);
            }
        }
        Err(error) => write_failure(root, "cannot serialize the lock-wait marker", &error),
    }
}

/// Removes this process's wait marker once it holds the lock.
///
/// Absence is the normal case for a build that never waited, so a missing file
/// is **not** reported: only a removal that fails for another reason means a
/// stale marker can outlive the wait it described.
///
/// A marker left by a terminated process is the case the file naming already
/// answers. The pid is in the filename, so a reader globbing
/// `cli-identifiers.lock-wait.*.json` can tell a marker whose process is gone
/// from one whose process is alive and still blocked — the presence of a marker
/// is not by itself evidence of a stall, and this function deliberately does
/// not sweep other processes' markers, because a live peer's marker is
/// information rather than litter.
pub(super) fn clear_wait(root: &Path) {
    match fs::remove_file(wait_marker(root)) {
        Err(error) if error.kind() != std::io::ErrorKind::NotFound => {
            write_failure(root, "cannot remove the lock-wait marker", &error);
        }
        Ok(()) | Err(_) => {}
    }
}

/// Writes one publication's observation, reporting any failure it hits.
///
/// Failure behaviour is deliberate and is the contract for this channel: it
/// explains builds and must not break them. An unwritable directory or a full
/// disk leaves the publication's own result untouched. The failure is recorded
/// through [`write_failure`] so the absent report is accompanied by the reason
/// it is absent.
///
/// The caller holds the artefact lock, so the shared temporary `atomic_write`
/// derives from the destination cannot collide here. The report's own path is
/// process-scoped, so two concurrent publications of one package write to
/// distinct files rather than racing for one name.
pub(super) fn write(root: &Path, report: &PublicationReport<'_>) {
    match serde_json::to_vec(report) {
        Ok(contents) => {
            if let Err(error) = atomic_write(&report_path(root), &contents) {
                write_failure(root, "cannot write the build report", &error);
            }
        }
        Err(error) => write_failure(root, "cannot serialize the build report", &error),
    }
}
