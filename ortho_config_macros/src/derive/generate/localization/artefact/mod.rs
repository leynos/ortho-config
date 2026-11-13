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
mod artefact_merge_error;
mod artefact_observation;
mod artefact_probe;
mod artefact_prune;
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

use self::artefact_merge_error::MergeError;
use self::artefact_probe::source_still_exists;
#[cfg(test)]
use self::artefact_probe::{INJECTED_PROBE_MESSAGE, with_metadata_outcome};
// Imported rather than called from here: `publish` prunes between its render
// and its writes, and the prune's own suite reaches the predicate and the
// injection hook through `super::*`. Making the names visible from `artefact`
// is what lets both do so without knowing which file holds them.
use self::artefact_prune::prune_stale_rendered;
#[cfg(test)]
use self::artefact_prune::{is_rendered_artefact, with_entry_probe_error};
#[cfg(test)]
use self::artefact_report::ErrorCategory;
// Named re-exports keep the schema items reachable from `publish` and the test
// modules as `super::Name`, exactly as they were before the split. The
// test-only names are listed under `cfg(test)` so a release build does not
// carry an unused import.
#[cfg(test)]
use self::schema::{
    ArtefactFile, CAP_BYTES, Document, EntryContext, Index, SCHEMA_VERSION, argument_entries,
    command_entries, ordered,
};
use self::schema::{Entry, Fragment, Source, entries, json, render, source};
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
fn merge_fragments(root: &Path) -> Result<Vec<Entry>, MergeError> {
    let fragments = root.join(FRAGMENT_DIR);
    let mut output = Vec::new();
    let entries = fs::read_dir(&fragments).map_err(|error| {
        MergeError::io(format!(
            "cannot list identifier artefact fragments in {}: {error}",
            fragments.display()
        ))
    })?;
    for item in entries {
        let path = item
            .map_err(|error| {
                MergeError::io(format!(
                    "cannot read an entry of the fragment directory {}: {error}",
                    fragments.display()
                ))
            })?
            .path();
        if path.extension().is_none_or(|extension| extension != "json") {
            continue;
        }
        let bytes = fs::read(&path).map_err(|error| {
            MergeError::io(format!(
                "cannot read identifier artefact fragment {}: {error}",
                path.display()
            ))
        })?;
        let fragment: Fragment = serde_json::from_slice(&bytes).map_err(|error| {
            MergeError::serialization(format!(
                "cannot parse identifier artefact fragment {}: {error}; delete it and rebuild",
                path.display()
            ))
        })?;
        if source_still_exists(Path::new(&fragment.source_file)).map_err(|error| {
            MergeError::io(format!(
                "cannot probe the source {} recorded by fragment {}: {error}",
                fragment.source_file,
                path.display()
            ))
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

// The test suites are declared without blank lines between them: each costs
// three lines of a file held to Whitaker's 400-line `module_max_lines` cap, and
// the cap is reached by declarations alone. The comments below are what a
// reader needs; the spacing is not.
#[cfg(test)]
#[path = "artefact_fragment_tests.rs"]
mod fragment_tests;
#[cfg(test)]
#[path = "artefact_prune_tests.rs"]
mod prune_tests;
#[cfg(test)]
#[path = "artefact_report_tests.rs"]
mod report_tests;
#[cfg(test)]
#[path = "artefact_tests.rs"]
mod tests;
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
// The categories a *failing publication* produces for the two merge faults,
// which `report_failure_tests` cannot show: it hands the channel a category it
// built itself rather than one a real merge classified.
#[cfg(test)]
#[path = "artefact_merge_report_tests.rs"]
mod merge_report_tests;
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
// Turns a still-held lock into a report naming the process and descriptor that
// hold it, split from the pure evidence vocabulary it prints so neither half
// outgrows the file-length cap; `lock_report_tests` proves it against a hold
// this process really owns.
#[cfg(test)]
#[path = "artefact_lock_diagnostics.rs"]
mod lock_diagnostics;
#[cfg(test)]
#[path = "artefact_lock_evidence.rs"]
mod lock_evidence;
// The procfs half of the report, split from the deciding half so each stays
// inside the file-length cap and so "no descriptor" and "could not read" have
// one place to be kept apart.
#[cfg(test)]
#[path = "artefact_lock_holder.rs"]
mod lock_holder;
// Test support for the holder report: gated on `test`, not the platform, so
// every test build compiles the module and its `lock_evidence` import alike.
#[cfg(test)]
#[path = "artefact_lock_process.rs"]
mod lock_process;
#[cfg(test)]
#[path = "artefact_lock_report_tests.rs"]
mod lock_report_tests;
// Bounded contention whose every free assertion routes through
// `lock_diagnostics`, so a post-release hold fails with the report that names
// its holder instead of with a bare boolean.
#[cfg(test)]
#[path = "artefact_lock_stress_tests.rs"]
mod lock_stress_tests;
#[cfg(test)]
#[path = "artefact_publication_tests.rs"]
mod publication_tests;
// The channel observed against real publications, split from `report_tests` so
// neither suite outgrows the file-length cap.
#[cfg(test)]
#[path = "artefact_channel_tests.rs"]
mod channel_tests;
