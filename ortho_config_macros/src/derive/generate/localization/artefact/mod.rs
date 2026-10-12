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

mod artefact_lock;
mod artefact_probe;
mod publish;
mod schema;

use std::collections::hash_map::DefaultHasher;
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};

use proc_macro2::Span;
use syn::Ident;

#[cfg(test)]
use self::artefact_lock::is_free;
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

const ARTEFACT_DIR: &str = "ortho-config";
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

/// Removes rendered artefact files left over from a previous layout.
///
/// `render` returns only the files the current entry set needs, so switching
/// between the single-file and index-plus-parts layouts would otherwise leave
/// the superseded files in place. A reader cannot tell them from current
/// output, and a stale index would keep pointing at parts that no longer
/// describe the inventory. Pruning keeps the directory equal to exactly what
/// the latest render produced.
///
/// The caller holds the artefact lock, so no concurrent process can be reading
/// or rewriting these files while they are removed.
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
        if is_rendered_artefact(name) && !keep.contains(name) && path.is_file() {
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
fn merge_fragments(root: &Path) -> Result<Vec<Entry>, String> {
    let fragments = root.join(FRAGMENT_DIR);
    let mut output = Vec::new();
    for item in fs::read_dir(&fragments).map_err(|error| error.to_string())? {
        let path = item.map_err(|error| error.to_string())?.path();
        if path.extension().is_none_or(|extension| extension != "json") {
            continue;
        }
        let bytes = fs::read(&path).map_err(|error| error.to_string())?;
        let fragment: Fragment =
            serde_json::from_slice(&bytes).map_err(|error| error.to_string())?;
        if source_still_exists(Path::new(&fragment.source_file)).map_err(|error| {
            format!(
                "cannot probe fragment source {}: {error}",
                fragment.source_file
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
    publish::publish(
        &publish::Publication {
            root: &root,
            ident,
            source: &source,
            crate_name: &crate_name,
            model,
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
#[path = "artefact_lock_tests.rs"]
mod lock_tests;
