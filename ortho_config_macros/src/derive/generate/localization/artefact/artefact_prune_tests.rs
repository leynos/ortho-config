//! Regression tests for pruning superseded rendered artefact layouts.
//!
//! These share the filesystem fixture helpers with `fragment_tests` via
//! `super`, so the temp-root discipline is defined in one place.

use std::fs;

use anyhow::{Context, Result, ensure};

use super::fragment_tests::TempRoot;
use super::*;

/// Builds a rendered file with the given name for prune fixtures.
fn rendered(name: &str) -> ArtefactFile {
    ArtefactFile {
        name: name.to_owned(),
        contents: Vec::new(),
    }
}

/// Verifies the rendered-file predicate matches the three published shapes.
#[test]
fn rendered_artefact_predicate_matches_published_names() -> Result<()> {
    for name in [
        "cli-identifiers.json",
        "cli-identifiers.index.json",
        "cli-identifiers.0.json",
        "cli-identifiers.12.json",
    ] {
        ensure!(is_rendered_artefact(name), "{name} must be rendered output");
    }
    // Fragments live in a subdirectory and the lock is not JSON, so neither is
    // rendered output. A near-miss name must not be pruned either.
    for name in [
        "cli-identifiers.lock",
        "cli-identifiers",
        "cli-identifiers.0.txt",
        "cli-identifiers.abc.json",
        "cli-identifiers.extra.json",
        "other.json",
    ] {
        ensure!(
            !is_rendered_artefact(name),
            "{name} must not be treated as rendered output"
        );
    }
    Ok(())
}

/// Verifies switching from the split layout to single-file prunes the parts.
///
/// `render` emits only the files the current entry set needs, so without
/// pruning the orphaned parts and index stay behind. A reader cannot tell them
/// from current output, and the stale index still names parts that no longer
/// describe the inventory.
#[test]
fn prune_removes_parts_superseded_by_single_file() -> Result<()> {
    let root = TempRoot::new()?;
    for name in [
        "cli-identifiers.json",
        "cli-identifiers.index.json",
        "cli-identifiers.0.json",
        "cli-identifiers.1.json",
    ] {
        fs::write(root.path().join(name), b"{}").context("write rendered fixture")?;
    }
    fs::create_dir_all(root.path().join(FRAGMENT_DIR)).context("create fragment directory")?;
    fs::write(root.path().join(FRAGMENT_DIR).join("kept.json"), b"{}")
        .context("write fragment fixture")?;
    fs::write(root.path().join("cli-identifiers.lock"), b"").context("write lock fixture")?;

    prune_stale_rendered(root.path(), &[rendered("cli-identifiers.json")])
        .context("prune superseded renders")?;

    ensure!(
        root.path().join("cli-identifiers.json").is_file(),
        "the current render must survive"
    );
    for name in [
        "cli-identifiers.index.json",
        "cli-identifiers.0.json",
        "cli-identifiers.1.json",
    ] {
        ensure!(
            !root.path().join(name).exists(),
            "superseded {name} must be pruned"
        );
    }
    ensure!(
        root.path().join(FRAGMENT_DIR).join("kept.json").is_file(),
        "fragments must survive pruning"
    );
    ensure!(
        root.path().join("cli-identifiers.lock").is_file(),
        "the lock file must survive pruning"
    );
    Ok(())
}

/// Verifies switching from single-file to the split layout prunes the whole file.
#[test]
fn prune_removes_single_file_superseded_by_parts() -> Result<()> {
    let root = TempRoot::new()?;
    fs::write(root.path().join("cli-identifiers.json"), b"{}").context("write rendered fixture")?;

    prune_stale_rendered(
        root.path(),
        &[
            rendered("cli-identifiers.index.json"),
            rendered("cli-identifiers.0.json"),
        ],
    )
    .context("prune superseded renders")?;

    ensure!(
        !root.path().join("cli-identifiers.json").exists(),
        "the superseded single file must be pruned"
    );
    Ok(())
}
