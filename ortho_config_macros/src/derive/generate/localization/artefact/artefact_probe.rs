//! Source-presence probing for identifier artefact fragments.
//!
//! A merged fragment is kept only while its recorded source file still
//! exists; a file that was deleted or moved leaves a stale fragment whose
//! entries must not survive into the published inventory. This module owns
//! that decision: `source_still_exists` is its only entry point, and the
//! probe it calls is the only `std::fs::metadata` call site for it.
//!
//! Test builds can inject metadata outcomes through `with_metadata_outcome`
//! instead of provoking failures through filesystem topology, because no
//! layout yields the same error kind on every platform. The injection is a
//! thread-local installed by the calling test and matches one exact probe
//! path; it is compiled out of non-test builds. This is not a
//! general-purpose filesystem abstraction; its scope, ownership, and call
//! sites are recorded in the developers' guide.

use std::fs;
use std::path::Path;

#[cfg(test)]
use std::cell::RefCell;
#[cfg(test)]
use std::io::ErrorKind;
#[cfg(test)]
use std::path::PathBuf;

/// Names the message an injected metadata failure carries.
///
/// The merge diagnostic interpolates the probe error, so a regression test
/// can prove the reported error came from the injection rather than from a
/// real filesystem condition.
#[cfg(test)]
pub(super) const INJECTED_PROBE_MESSAGE: &str = "injected metadata failure";

/// Remembers the metadata outcome to inject for one probe path.
#[cfg(test)]
enum Injected {
    /// A metadata success for the recorded path.
    Exists(PathBuf),
    /// A metadata error kind for the recorded path.
    Error(PathBuf, ErrorKind),
}

#[cfg(test)]
thread_local! {
    static INJECTED: RefCell<Option<Injected>> = const { RefCell::new(None) };
}

/// Runs `body` with a metadata outcome injected for `path`.
///
/// The outcome is confined to the calling thread and cleared when `body`
/// returns, so an injection cannot leak into a later test on the same thread.
/// A nested call replaces the outer outcome; the tests here do not nest.
#[cfg(test)]
pub(super) fn with_metadata_outcome<T>(
    path: &Path,
    outcome: Result<(), ErrorKind>,
    body: impl FnOnce() -> T,
) -> T {
    let injected = match outcome {
        Ok(()) => Injected::Exists(path.to_path_buf()),
        Err(kind) => Injected::Error(path.to_path_buf(), kind),
    };
    INJECTED.with(|cell| *cell.borrow_mut() = Some(injected));
    let result = body();
    INJECTED.with(|cell| *cell.borrow_mut() = None);
    result
}

/// Returns the outcome to inject for `path`, if one is installed for it.
#[cfg(test)]
fn injected(path: &Path) -> Option<Result<(), ErrorKind>> {
    INJECTED.with(|cell| match cell.borrow().as_ref() {
        Some(Injected::Exists(recorded)) if recorded == path => Some(Ok(())),
        Some(Injected::Error(recorded, kind)) if recorded == path => Some(Err(*kind)),
        _ => None,
    })
}

/// Reads the metadata for a probe path, honouring an injected outcome.
///
/// A successful injection cannot synthesize a metadata value, so it records
/// the test's own fixture path and re-issues the real probe against it. That
/// keeps the production wrapper, not a stand-in, supplying the metadata the
/// classification reads.
#[cfg(test)]
fn injected_metadata(path: &Path) -> Option<std::io::Result<fs::Metadata>> {
    match injected(path)? {
        Ok(()) => Some(fs::metadata(path)),
        Err(kind) => Some(Err(std::io::Error::new(kind, INJECTED_PROBE_MESSAGE))),
    }
}

/// Reads metadata for a fragment source, honouring injected outcomes under
/// test.
///
/// Production always reaches `fs::metadata` here; the injected path is
/// test-only and must stay that way, so the production wrapper is the same
/// call every policy case is exercised through.
fn probe_metadata(path: &Path) -> std::io::Result<fs::Metadata> {
    #[cfg(test)]
    if let Some(outcome) = injected_metadata(path) {
        return outcome;
    }
    fs::metadata(path)
}

/// Classifies one metadata outcome for a fragment source.
///
/// Only `NotFound` means the source was deleted or moved, which is the one
/// case that makes the fragment stale. Any other metadata failure says
/// nothing about presence, so it is propagated: silently dropping the
/// fragment would publish an inventory that is missing entries with no
/// diagnostic to explain the gap.
fn classify_metadata(outcome: std::io::Result<fs::Metadata>) -> std::io::Result<bool> {
    match outcome {
        Ok(_) => Ok(true),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error),
    }
}

/// Reports whether a fragment's recorded source file is still present.
///
/// The probe and the classification are shared between production and tests:
/// the probe is the only `fs::metadata` call and this classification is the
/// only copy of the stale-fragment policy, so injected outcomes exercise the
/// production decision path rather than a test-only copy of it.
pub(super) fn source_still_exists(path: &Path) -> std::io::Result<bool> {
    classify_metadata(probe_metadata(path))
}

#[cfg(test)]
#[path = "artefact_probe_tests.rs"]
mod tests;
