//! The wait marker: which publication is blocked on the artefact lock.
//!
//! A blocked publication writes one marker naming itself and how long it has
//! waited, and refreshes it while the wait continues. The marker answers the
//! question a maintainer watching a stalled build actually has — *is this
//! build waiting for a peer, or is it stuck on something else?* — at the
//! moment it is asked, without a profiler and without reproducing the stall.
//!
//! # What the marker does and does not promise
//!
//! A marker is written only by an opted-in build, and only once the wait has
//! passed [`WAIT_WARN_THRESHOLD`], so its **presence is evidence of a wait that
//! was still running when it was written**. It is not evidence of a wait
//! running *now*. Three ways it can outlive the wait it describes:
//!
//! * **Termination.** A process killed mid-wait never reaches the code that
//!   retires its marker. Nothing sweeps another process's marker, deliberately:
//!   a live peer's marker is information, and a sweep would have to guess which
//!   peers are alive.
//! * **Failed cleanup.** Retiring the marker is a filesystem operation that can
//!   itself fail, or can be prevented by an `OUT_DIR` that has since become
//!   unwritable. The failure is recorded in the channel log; the marker stays.
//! * **A failed acquisition.** The acquisition-failure path retires the marker
//!   before reporting, so this one is closed — but only since it was fixed, and
//!   a marker written by an older build of this crate can still be lying
//!   around.
//!
//! This is why the filename carries the writing process's id and the body
//! carries `written_at_ms`. **A live process id is not proof of contention** —
//! ids are reused, and a marker whose process is long gone can share its id
//! with an unrelated process. Read the marker's timestamp next to the build's
//! own timeline, and treat a marker older than the build under investigation as
//! a leftover rather than a clue. What a reader may safely conclude is the
//! converse: a marker that is both recent and freshly refreshed describes a
//! wait that had not yet ended.
//!
//! # Why this is not `atomic_write`
//!
//! The marker is written precisely while the artefact lock is *not* held, so
//! two blocked publishers can be writing at the same moment. `atomic_write`
//! derives its temporary from the destination alone, which those two would
//! share; deriving the temporary from the already process-private destination
//! keeps both writes independent.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::Serialize;

use super::artefact_channel_log::write_failure;
use super::artefact_report::{ErrorCategory, millis};

/// Names the file a blocked publication uses to announce its wait.
///
/// The publishing process's id is appended, so a reader can attribute one
/// marker to one waiting process — and so a marker left by a terminated
/// process does not masquerade as this one's.
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

/// Records the process and elapsed wait a blocked publication reached.
#[derive(Debug, Serialize)]
struct WaitMarker {
    /// The waiting process, so a marker left by a dead one can be told apart.
    pid: u32,
    /// Time already spent waiting when the marker was written.
    waited_ms: u64,
    /// Wall-clock write time in whole milliseconds since the Unix epoch.
    ///
    /// The filename's process id cannot establish freshness — ids are reused —
    /// so this is the field that dates the wait.
    written_at_ms: u64,
}

/// Returns this process's wait-marker path below one artefact root.
pub(super) fn wait_marker(root: &Path) -> PathBuf {
    root.join(format!("{WAIT_STEM}.{}.json", std::process::id()))
}

/// Returns the wait-marker path one process writes below an artefact root.
///
/// The naming rule itself is what keeps two waiting processes apart, which is
/// a property worth checking without spawning a process to check it. Tests
/// holding a helper's id rather than their own use this.
#[cfg(test)]
pub(super) fn wait_marker_for(root: &Path, pid: u32) -> PathBuf {
    root.join(format!("{WAIT_STEM}.{pid}.json"))
}

/// Publishes bytes atomically through a temporary beside the destination.
///
/// Deliberately not `atomic_write`: that helper derives its temporary from the
/// destination alone, and the marker is written while the lock is *not* held,
/// so two blocked publishers would collide on one shared temporary name. See
/// the module documentation.
fn write_private_atomic(path: &Path, contents: &[u8]) -> std::io::Result<()> {
    let temporary = path.with_extension("json.tmp");
    fs::write(&temporary, contents)?;
    fs::rename(temporary, path)
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
        written_at_ms: super::artefact_report::epoch_millis(),
    };
    match serde_json::to_vec(&marker) {
        Ok(contents) => {
            if let Err(error) = write_private_atomic(&wait_marker(root), &contents) {
                write_failure(
                    root,
                    "cannot write the lock-wait marker",
                    ErrorCategory::Io,
                    &error,
                );
            }
        }
        Err(error) => write_failure(
            root,
            "cannot serialize the lock-wait marker",
            ErrorCategory::Serialization,
            &error,
        ),
    }
}

/// Removes this process's wait marker once the wait has ended.
///
/// Called both when the lock is granted and when the acquisition fails: those
/// are the two terminal paths a wait can take, and on both the marker has
/// stopped describing anything true.
///
/// Absence is the normal case for a build that never waited, so a missing file
/// is **not** reported: only a removal that fails for another reason means a
/// stale marker can outlive the wait it described. That failure is recorded,
/// because it is precisely the "failed cleanup" case a reader needs to know
/// about before trusting any marker they find. See the module documentation
/// for the freshness rule that follows from it.
///
/// This deliberately does not sweep other processes' markers. A live peer's
/// marker is information, and a terminated peer's has no live handle to
/// distinguish it from a slow one.
pub(super) fn clear_wait(root: &Path) {
    match fs::remove_file(wait_marker(root)) {
        Err(error) if error.kind() != std::io::ErrorKind::NotFound => write_failure(
            root,
            "cannot remove the lock-wait marker",
            ErrorCategory::Io,
            &error,
        ),
        Ok(()) | Err(_) => {}
    }
}
