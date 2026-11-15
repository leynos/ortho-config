//! Removing rendered artefact files left over from a previous layout.
//!
//! `render` returns only the files the current entry set needs, so switching
//! between the single-file and index-plus-parts layouts would otherwise leave
//! the superseded files in place. A reader cannot tell them from current
//! output, and a stale index would keep pointing at parts that no longer
//! describe the inventory. Pruning keeps the directory equal to exactly what
//! the latest render produced.
//!
//! This lives apart from `artefact` because "which files does this render
//! keep" is a self-contained question, answered without reference to the
//! publication sequence. The module also owns the only test-injection point in
//! the prune, so the failure a regression test provokes has one place to be
//! defined and cannot drift from the code that reads it.

use std::fs;
use std::path::Path;
// The injection record is the only thing that names a path by value, and only
// test builds carry it.
#[cfg(test)]
use std::path::PathBuf;

use super::schema::ArtefactFile;

/// Returns whether a directory entry is a rendered artefact file.
///
/// Rendered files are the single `cli-identifiers.json`, the
/// `cli-identifiers.index.json` manifest, and the numbered
/// `cli-identifiers.<n>.json` parts. Fragments live in
/// [`super::FRAGMENT_DIR`] and the lock file is not JSON, so neither is
/// matched.
pub(super) fn is_rendered_artefact(name: &str) -> bool {
    let Some(rest) = name.strip_prefix("cli-identifiers") else {
        return false;
    };
    // The unsuffixed file is the whole single-file layout.
    if rest == ".json" {
        return true;
    }
    let Some(tail) = rest.strip_prefix('.') else {
        return false;
    };
    tail.strip_suffix(".json")
        .is_some_and(|index| index == "index" || index.parse::<usize>().is_ok())
}

/// Names the message a test-injected directory-entry probe failure carries.
///
/// The prune returns the probe error, so a regression test can prove the
/// reported failure came from the injection rather than from a real
/// filesystem condition.
#[cfg(test)]
pub(super) const INJECTED_ENTRY_PROBE_MESSAGE: &str = "injected directory-entry probe failure";

// Records one metadata error kind to inject for one exact directory entry.
//
// This mirrors the injection `artefact_probe` documents: a failed probe that
// is not `NotFound` cannot be provoked portably through filesystem topology,
// so test builds name the failure they want. Only failures are injectable
// here; a test that wants the success path creates the file.
#[cfg(test)]
thread_local! {
    static INJECTED_ENTRY_PROBE: std::cell::RefCell<Option<(PathBuf, std::io::ErrorKind)>> =
        const { std::cell::RefCell::new(None) };
}

/// Runs `body` with `kind` injected as the probe outcome for `path`.
#[cfg(test)]
pub(super) fn with_entry_probe_error<T>(
    path: &Path,
    kind: std::io::ErrorKind,
    body: impl FnOnce() -> T,
) -> T {
    INJECTED_ENTRY_PROBE.with(|cell| *cell.borrow_mut() = Some((path.to_path_buf(), kind)));
    let result = body();
    INJECTED_ENTRY_PROBE.with(|cell| *cell.borrow_mut() = None);
    result
}

/// Reads one directory entry's metadata, honouring an injected failure.
///
/// This asks about the entry the prune would unlink, so it uses
/// `symlink_metadata`: a symlink must be classified as the link it is, not as
/// whatever its target happens to be.
fn probe_entry(path: &Path) -> std::io::Result<fs::Metadata> {
    #[cfg(test)]
    if let Some(kind) = INJECTED_ENTRY_PROBE.with(|cell| {
        cell.borrow()
            .as_ref()
            .filter(|(recorded, _)| recorded == path)
            .map(|(_, kind)| *kind)
    }) {
        return Err(std::io::Error::new(kind, INJECTED_ENTRY_PROBE_MESSAGE));
    }
    fs::symlink_metadata(path)
}

/// Returns whether a directory entry is a superseded rendered artefact.
///
/// Only a regular file that is rendered output and is absent from the current
/// render is superseded. Splitting the tests out of the prune loop keeps the
/// directory scan reading as "for each entry: drop it when superseded", rather
/// than as a single conjunct that a reader must unpack.
///
/// The probe reports failure rather than answering `false`. An unreadable
/// entry says nothing about whether the file is superseded, so reading the
/// failure as "keep it" would leave stale rendered output in the directory
/// while the declared [`std::io::Result`] promised the caller a diagnostic.
/// Only an entry that has genuinely vanished is answered `false`: it cannot be
/// a leftover the prune still needs to remove.
fn is_superseded(
    name: &str,
    keep: &std::collections::HashSet<&str>,
    path: &Path,
) -> std::io::Result<bool> {
    if !is_rendered_artefact(name) || keep.contains(name) {
        // Neither the fragments, nor the lock, nor any file the current
        // render keeps is a stale candidate, so none of them pays a metadata
        // call.
        return Ok(false);
    }
    match probe_entry(path) {
        Ok(metadata) => Ok(metadata.is_file()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error),
    }
}

/// Removes rendered artefact files left over from a previous layout.
///
/// The caller holds the artefact lock, so no other *writer* participates while
/// these files are removed. The lock excludes publishers only: it is
/// cooperative, so a reader that never takes it — a build script globbing the
/// directory, say — can still observe the transition between the prune and the
/// writes that follow it. Atomic per-file writes keep each file whole; the
/// directory as a whole is not swapped atomically.
///
/// A failure to classify an entry, or to unlink it, is returned rather than
/// skipped. Both would otherwise leave a stale file in place while the caller
/// reported success.
pub(super) fn prune_stale_rendered(root: &Path, current: &[ArtefactFile]) -> std::io::Result<()> {
    let keep = current
        .iter()
        .map(|file| file.name.as_str())
        .collect::<std::collections::HashSet<_>>();
    for entry in fs::read_dir(root)? {
        let candidate = entry?;
        let raw_name = candidate.file_name();
        let Some(name) = raw_name.to_str() else {
            continue;
        };
        let path = candidate.path();
        if is_superseded(name, &keep, &path)? {
            fs::remove_file(path)?;
        }
    }
    Ok(())
}
