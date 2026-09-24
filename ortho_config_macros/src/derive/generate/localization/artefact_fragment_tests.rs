//! Filesystem-focused regression tests for identifier artefact fragments.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

use anyhow::{Context, Result, ensure};
use proc_macro2::Span;
use syn::Ident;

use super::*;

static TEMP_ROOT_SEQUENCE: AtomicUsize = AtomicUsize::new(0);

/// Owns an isolated temporary directory used by one filesystem regression test.
struct TempRoot(PathBuf);

impl TempRoot {
    /// Creates a unique directory below the system temporary directory.
    fn new() -> Result<Self> {
        let sequence = TEMP_ROOT_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "ortho-config-artefact-tests-{}-{sequence}",
            std::process::id()
        ));
        fs::create_dir_all(&path).context("create isolated artefact test root")?;
        Ok(Self(path))
    }

    /// Returns the root directory available to the current test.
    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TempRoot {
    fn drop(&mut self) {
        drop(fs::remove_dir_all(&self.0));
    }
}

/// Builds a stable entry suitable for filesystem fragment fixtures.
fn fixture_entry(id: &str) -> Entry {
    Entry {
        id: id.to_owned(),
        kind: String::from("about"),
        type_name: String::from("fixture::Config"),
        field: None,
        path_scope: String::from("standalone"),
        source: Source {
            file: String::from("fixture.rs"),
            line: 1,
            column: 0,
        },
        embedded_default: None,
    }
}

/// Writes one serialised fragment below an isolated artefact root.
fn write_fragment(path: &Path, source_file: String, entries: Vec<Entry>) -> Result<()> {
    let parent = path.parent().context("fragment parent directory")?;
    fs::create_dir_all(parent).context("create fragment directory")?;
    fs::write(
        path,
        json(&Fragment {
            source_file,
            entries,
        })?,
    )
    .context("write fragment")?;
    Ok(())
}

/// Creates an existing source file that marks a fragment as current.
fn current_source(root: &Path) -> Result<PathBuf> {
    let source = root.join("current.rs");
    fs::write(&source, "// fixture source\n").context("write current source fixture")?;
    Ok(source)
}

/// Verifies exact environment opt-in accepts only the documented literal value.
#[test]
fn identifier_artefact_opt_in_requires_literal_one() -> Result<()> {
    ensure!(
        !requested_value(None),
        "missing opt-in must not emit an artefact"
    );
    ensure!(
        requested_value(Some("1")),
        "literal one must enable emission"
    );
    ensure!(!requested_value(Some("0")), "zero must not enable emission");
    ensure!(
        !requested_value(Some("true")),
        "boolean-like values must not enable emission"
    );
    Ok(())
}

/// Verifies fragments for removed source files are pruned from merged output.
#[test]
fn merge_fragments_prunes_removed_sources() -> Result<()> {
    let root = TempRoot::new()?;
    let source = current_source(root.path())?;
    let ident = Ident::new("Config", Span::call_site());
    let current = Source {
        file: source.to_string_lossy().into_owned(),
        line: 3,
        column: 2,
    };
    let stale = Source {
        file: root
            .path()
            .join("removed.rs")
            .to_string_lossy()
            .into_owned(),
        line: 5,
        column: 1,
    };
    write_fragment(
        &fragment_path(root.path(), &ident, &current),
        current.file.clone(),
        vec![fixture_entry("current-about")],
    )?;
    write_fragment(
        &fragment_path(root.path(), &ident, &stale),
        stale.file.clone(),
        vec![fixture_entry("stale-about")],
    )?;

    let merged = merge_fragments(root.path()).map_err(anyhow::Error::msg)?;
    ensure!(merged.len() == 1, "only current fragments should remain");
    ensure!(
        merged
            .first()
            .is_some_and(|entry| entry.id == "current-about"),
        "current entry must survive"
    );
    Ok(())
}

/// Verifies same-named derives at separate locations retain independent fragments.
#[test]
fn same_named_derives_at_distinct_locations_keep_both_fragments() -> Result<()> {
    let root = TempRoot::new()?;
    let source_file = current_source(root.path())?;
    let ident = Ident::new("Config", Span::call_site());
    let first = Source {
        file: source_file.to_string_lossy().into_owned(),
        line: 10,
        column: 4,
    };
    let second = Source {
        line: 20,
        column: 4,
        ..first.clone()
    };
    let first_path = fragment_path(root.path(), &ident, &first);
    let second_path = fragment_path(root.path(), &ident, &second);
    ensure!(
        first_path != second_path,
        "derive locations must produce distinct fragment names"
    );
    write_fragment(
        &first_path,
        first.file.clone(),
        vec![fixture_entry("first-about")],
    )?;
    write_fragment(
        &second_path,
        second.file.clone(),
        vec![fixture_entry("second-about")],
    )?;

    let mut identifiers = merge_fragments(root.path())
        .map_err(anyhow::Error::msg)?
        .into_iter()
        .map(|entry| entry.id)
        .collect::<Vec<_>>();
    identifiers.sort();
    ensure!(
        identifiers == ["first-about", "second-about"],
        "merged artefact must retain both same-named derives"
    );
    Ok(())
}

/// Verifies malformed fragment JSON returns an actionable merge error.
#[test]
fn merge_fragments_reports_malformed_json() -> Result<()> {
    let root = TempRoot::new()?;
    let fragments = root.path().join(FRAGMENT_DIR);
    fs::create_dir_all(&fragments).context("create fragment directory")?;
    fs::write(fragments.join("malformed.json"), "not json").context("write malformed fragment")?;

    let error = merge_fragments(root.path()).expect_err("malformed JSON must fail merging");
    ensure!(
        !error.is_empty(),
        "malformed JSON error must retain diagnostics"
    );
    Ok(())
}

/// Verifies directory-backed fragment paths report filesystem read failures.
#[test]
fn merge_fragments_reports_unreadable_fragment_path() -> Result<()> {
    let root = TempRoot::new()?;
    fs::create_dir_all(root.path().join(FRAGMENT_DIR).join("unreadable.json"))
        .context("create unreadable fragment directory")?;

    let error = merge_fragments(root.path()).expect_err("directory fragment must fail reading");
    ensure!(!error.is_empty(), "read failure must retain diagnostics");
    Ok(())
}

/// Verifies missing fragment directories report their filesystem error.
#[test]
fn merge_fragments_reports_missing_fragment_directory() -> Result<()> {
    let root = TempRoot::new()?;
    let error = merge_fragments(root.path()).expect_err("missing fragment directory must fail");
    ensure!(
        !error.is_empty(),
        "missing directory error must retain diagnostics"
    );
    Ok(())
}

/// Verifies atomic writer preserves filesystem errors for an invalid destination.
#[test]
fn atomic_write_reports_invalid_destination() -> Result<()> {
    let root = TempRoot::new()?;
    let destination = root.path().join("destination");
    fs::create_dir_all(&destination).context("create destination directory")?;

    let error = atomic_write(&destination, b"contents")
        .expect_err("directory destinations must fail atomically");
    ensure!(
        !error.to_string().is_empty(),
        "write error must retain diagnostics"
    );
    Ok(())
}
