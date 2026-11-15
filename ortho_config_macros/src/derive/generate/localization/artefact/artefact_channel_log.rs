//! The channel's own failure record: what the observer could not observe.
//!
//! The observation channel is best effort by contract — it explains builds and
//! must never fail one — so every writer in it discards a failure it cannot
//! turn into a compiler diagnostic. Discarding *silently* is the part that
//! cannot stand: an opted-in build with a broken channel would leave an
//! artefact directory that looks exactly like one where nothing happened, and
//! the maintainer who set the variable would read "no report" as "no problem".
//!
//! This module is where those discarded failures go. One file per process, one
//! **JSON Lines** record per lost observation:
//!
//! ```json
//! {"pid":4242,"operation":"cannot write the build report","error":"io",
//!  "detail":"No space left on device (os error 28)",
//!  "written_at_ms":1770000000000}
//! ```
//!
//! # Why JSON Lines, and why `.jsonl`
//!
//! The file is append-only and may be read while the build that writes it is
//! still running, so a reader must be able to consume every complete record
//! without parsing a partially written one. One JSON value per line is the
//! format that allows exactly that, and its extension says so. The extension
//! was `.json` before, which was a claim a reader could only disprove by
//! opening the file: a `.json` document is one value, and this file is a
//! sequence of them.
//!
//! `error` is the same bounded [`ErrorCategory`] the build report uses, so a
//! harness can group failures across both files with one vocabulary. `detail`
//! carries the underlying error's own text — the part a human needs, and the
//! part no harness should group on.
//!
//! # A failure to record the failure
//!
//! A write here that itself fails is discarded, and this is the one place the
//! contract is unconditional. If the directory cannot be written at all — a
//! full disk, a read-only `OUT_DIR` — there is nowhere left to record the
//! failure of recording the failure, and the compilation must still succeed.
//! The recursion has to stop somewhere; it stops here.
//!
//! # Not an inventory artefact
//!
//! The file sits at the artefact root beside the render output, so it is worth
//! stating that the prune leaves it alone. The renderer's matcher accepts only
//! `cli-identifiers.json`, `cli-identifiers.index.json`, and
//! `cli-identifiers.<n>.json`; this name carries a second stem before the
//! process id and ends `.jsonl`, so it matches none of them. A prune that
//! removed it would delete the evidence that the channel was broken in the
//! same pass that needed it.

use std::fs;
use std::io::Write as _;
use std::path::{Path, PathBuf};

use serde::Serialize;

use super::artefact_report::{ErrorCategory, epoch_millis};

/// Names the process-scoped file recording the channel's lost observations.
///
/// The extension is `.jsonl` rather than `.json` because the file holds one
/// JSON value per line rather than a single document; see the module
/// documentation for why that distinction is worth an extension of its own.
pub(super) const ERRORS_STEM: &str = "cli-identifiers.observation-errors";

/// Records one observation the channel could not make.
#[derive(Debug, Serialize)]
struct ChannelRecord<'a> {
    /// Process that lost the observation, matching the filename's suffix.
    pid: u32,
    /// What the channel was attempting when it failed.
    operation: &'a str,
    /// Bounded category of the failure, shared with the build report.
    error: ErrorCategory,
    /// The underlying error's own text, for a human reading the file.
    detail: String,
    /// Wall-clock write time in whole milliseconds since the Unix epoch.
    ///
    /// Records are appended and outlive the run that wrote them, so each one
    /// dates itself for the same reason the build report does.
    written_at_ms: u64,
}

/// Returns this process's observation-error path below one artefact root.
pub(super) fn errors_path(root: &Path) -> PathBuf {
    root.join(format!("{ERRORS_STEM}.{}.jsonl", std::process::id()))
}

/// Appends one record of an observation the channel could not make.
///
/// `operation` names what was attempted and `category` classifies why it
/// failed, both as the caller already knows them; `error` supplies the
/// underlying text. A failure to append is discarded, for the reason the
/// module documentation gives: there is nowhere left to report it, and the
/// compilation must still succeed.
pub(super) fn write_failure(
    root: &Path,
    operation: &str,
    category: ErrorCategory,
    error: &dyn std::fmt::Display,
) {
    let record = ChannelRecord {
        pid: std::process::id(),
        operation,
        error: category,
        detail: format!("{error}"),
        written_at_ms: epoch_millis(),
    };
    let Ok(mut line) = serde_json::to_string(&record) else {
        return;
    };
    line.push('\n');
    drop(
        fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(errors_path(root))
            .and_then(|mut file| file.write_all(line.as_bytes())),
    );
}
