//! Opt-in build-time export of derived CLI localization identifiers.
//!
//! The pure schema and renderer live in `schema` so ordering and splitting
//! behaviour stay testable without filesystem state; this module owns the
//! `OUT_DIR` layout and the merge of per-expansion fragments.
//!
//! `OUT_DIR` is package-scoped rather than target-scoped, so a package with
//! more than one compilation target can have several compiler processes
//! writing here concurrently. `emit` serialises the complete fragment-write,
//! merge, and publication sequence behind an exclusive file lock in the
//! artefact directory; see `publish` for why a unique temporary filename alone
//! is not sufficient.

mod artefact_channel_log;
mod artefact_lock;
mod artefact_observation;
mod artefact_probe;
mod artefact_report;
mod artefact_wait_marker;
mod publish;
mod schema;

use std::collections::hash_map::DefaultHasher;
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};

use proc_macro2::Span;
use syn::Ident;

use self::artefact_probe::source_still_exists;
#[cfg(test)]
use self::artefact_probe::{INJECTED_PROBE_MESSAGE, with_metadata_outcome};
// Named re-exports keep the schema items reachable from `publish` and the test
// modules as `super::Name`, exactly as they were before the split. The
// test-only names are listed under `cfg(test)` so a release build does not
// carry an unused import.
use self::schema::{ArtefactFile, Entry, Fragment, Source, entries, json, render, source};
#[cfg(test)]
use self::schema::{
    CAP_BYTES, Document, EntryContext, Index, SCHEMA_VERSION, argument_entries, command_entries,
    ordered,
};
use super::LocalizationIds;
#[cfg(test)]
use super::suffix::MessageSuffix;

/// Names the directory below `OUT_DIR` holding every artefact and channel file.
const ARTEFACT_DIR: &str = "ortho-config";

/// Names the subdirectory holding one JSON fragment per derive expansion.
///
/// A subdirectory rather than a filename prefix, so the merge can enumerate
/// exactly the fragments and the renderer's artefact matcher never sees them.
const FRAGMENT_DIR: &str = "cli-identifiers.d";

/// Returns whether an artefact-emission environment value explicitly opts in.
fn requested_value(value: Option<&str>) -> bool {
    value == Some("1")
}

/// Returns whether the consuming build explicitly requested artefact output.
fn requested() -> bool {
    requested_value(
        std::env::var("ORTHO_CONFIG_EMIT_IDENTIFIERS")
            .ok()
            .as_deref(),
    )
}

/// Replaces a generated file atomically through a sibling temporary file.
fn atomic_write(path: &Path, contents: &[u8]) -> std::io::Result<()> {
    let temporary = path.with_extension("tmp");
    fs::write(&temporary, contents)?;
    fs::rename(temporary, path)
}

/// Returns whether a directory entry is a rendered artefact file.
///
/// Rendered files are the single `cli-identifiers.json`, the
/// `cli-identifiers.index.json` manifest, and the numbered
/// `cli-identifiers.<n>.json` parts. Fragments live in `FRAGMENT_DIR` and the
/// lock file is not JSON, so neither is matched.
fn is_rendered_artefact(name: &str) -> bool {
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
/// `render` returns only the files the current entry set needs, so switching
/// between the single-file and index-plus-parts layouts would otherwise leave
/// the superseded files in place. A reader cannot tell them from current
/// output, and a stale index would keep pointing at parts that no longer
/// describe the inventory. Pruning keeps the directory equal to exactly what
/// the latest render produced.
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
fn prune_stale_rendered(root: &Path, current: &[ArtefactFile]) -> std::io::Result<()> {
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

/// Hashes a source path for a stable per-expansion fragment name.
fn hash(value: &str) -> u64 {
    let mut hasher = DefaultHasher::new();
    value.hash(&mut hasher);
    hasher.finish()
}

/// Computes the fragment path for one deriving type and source location.
fn fragment_path(root: &Path, ident: &Ident, source: &Source) -> PathBuf {
    root.join(FRAGMENT_DIR).join(format!(
        "{ident}-{}-{}-{:016x}.json",
        source.line,
        source.column,
        hash(&source.file)
    ))
}

/// Reads JSON fragments and drops fragments for removed source files.
///
/// Every failure names both the operation and the path it was attempted on.
/// The bare `io::Error` text says only what went wrong, and the merge runs
/// inside a procedural macro, so a message without the fragment path leaves
/// the build maintainer to guess which of several expansions or targets wrote
/// the file that could not be read. The returned text reaches the compiler
/// diagnostic verbatim through `publish::republish`, so it is the whole of what
/// a failing build shows.
fn merge_fragments(root: &Path) -> Result<Vec<Entry>, String> {
    let fragments = root.join(FRAGMENT_DIR);
    let mut output = Vec::new();
    let entries = fs::read_dir(&fragments).map_err(|error| {
        format!(
            "cannot list identifier artefact fragments in {}: {error}",
            fragments.display()
        )
    })?;
    for item in entries {
        let path = item
            .map_err(|error| {
                format!(
                    "cannot read an entry of the fragment directory {}: {error}",
                    fragments.display()
                )
            })?
            .path();
        if path.extension().is_none_or(|extension| extension != "json") {
            continue;
        }
        let bytes = fs::read(&path).map_err(|error| {
            format!(
                "cannot read identifier artefact fragment {}: {error}",
                path.display()
            )
        })?;
        let fragment: Fragment = serde_json::from_slice(&bytes).map_err(|error| {
            format!(
                "cannot parse identifier artefact fragment {}: {error}; delete it and rebuild",
                path.display()
            )
        })?;
        if source_still_exists(Path::new(&fragment.source_file)).map_err(|error| {
            format!(
                "cannot probe the source {} recorded by fragment {}: {error}",
                fragment.source_file,
                path.display()
            )
        })? {
            output.extend(fragment.entries);
        }
    }
    Ok(output)
}

/// Writes an artefact only when explicitly requested by the consuming build.
///
/// This is the only place the artefact path reads the process environment.
/// Everything below it takes its inputs explicitly, so the publication
/// sequence can be driven from tests without ambient state.
pub(super) fn emit(model: &LocalizationIds, ident: &Ident, span: Span) -> syn::Result<()> {
    if !requested() {
        return Ok(());
    }
    let out_dir = std::env::var("OUT_DIR").map_err(|_| syn::Error::new(span, "identifier artefact emission requires OUT_DIR; unset ORTHO_CONFIG_EMIT_IDENTIFIERS or add a build.rs"))?;
    let root = publish::artefact_root(Path::new(&out_dir), span)?;
    let source = source(span);
    let crate_name = std::env::var("CARGO_CRATE_NAME").unwrap_or_else(|_| String::from("unknown"));
    let report = artefact_report::ReportConfig::observed(artefact_report::Reporting::requested(
        std::env::var(artefact_report::REPORT_ENV).ok().as_deref(),
    ));
    publish::publish(
        &publish::Publication {
            root: &root,
            ident,
            source: &source,
            crate_name: &crate_name,
            model,
            report,
        },
        span,
    )
}

#[cfg(test)]
#[path = "artefact_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "artefact_fragment_tests.rs"]
mod fragment_tests;

#[cfg(test)]
#[path = "artefact_prune_tests.rs"]
mod prune_tests;

#[cfg(test)]
#[path = "artefact_report_tests.rs"]
mod report_tests;

// What a channel file's *name* lets a reader conclude, split from
// `report_tests` so neither suite outgrows the file-length cap; the two share
// `report_tests::report`.
#[cfg(test)]
#[path = "artefact_report_identity_tests.rs"]
mod report_identity_tests;

// The channel's own failure record, split from `report_tests` so neither suite
// outgrows the file-length cap; `report_tests::report` is shared between them.
#[cfg(test)]
#[path = "artefact_report_failure_tests.rs"]
mod report_failure_tests;

// Shared by `lock_tests` and `publication_tests`, so it is neither suite's own.
#[cfg(test)]
#[path = "artefact_lock_support.rs"]
mod lock_support;

// The fixture publications the lock suites drive the production boundary with,
// split out so neither support module outgrows the file-length cap.
#[cfg(test)]
#[path = "artefact_lock_fixture.rs"]
mod lock_fixture;

#[cfg(test)]
#[path = "artefact_lock_tests.rs"]
mod lock_tests;

#[cfg(test)]
#[path = "artefact_publication_tests.rs"]
mod publication_tests;

// The channel observed against real publications, split from `report_tests` so
// neither suite outgrows the file-length cap.
#[cfg(test)]
#[path = "artefact_channel_tests.rs"]
mod channel_tests;
