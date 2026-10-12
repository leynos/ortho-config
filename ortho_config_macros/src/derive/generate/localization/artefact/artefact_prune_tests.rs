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

/// Verifies an unreadable entry fails the prune instead of being kept.
///
/// The classification asks a question — "is this file superseded?" — and an
/// unreadable entry does not answer it. Reporting `false` would leave the
/// stale file in place while `prune_stale_rendered` returned `Ok`, so the
/// caller would publish a directory that still holds a layout it believed it
/// had removed. The probe error has to reach the caller.
///
/// The failure is injected rather than provoked through filesystem topology:
/// no portable layout yields the same error kind on every platform, which is
/// the constraint `artefact_probe` documents for the same reason. A
/// `PermissionDenied` probe stands in for any failure that is not `NotFound`.
#[test]
fn an_unreadable_entry_fails_the_prune_rather_than_being_kept() -> Result<()> {
    let root = TempRoot::new()?;
    let superseded = root.path().join("cli-identifiers.0.json");
    fs::write(&superseded, b"{}").context("write superseded fixture")?;

    let outcome = with_entry_probe_error(&superseded, std::io::ErrorKind::PermissionDenied, || {
        prune_stale_rendered(root.path(), &[rendered("cli-identifiers.json")])
    });

    let error = outcome
        .err()
        .context("an unreadable entry must fail the prune, not be silently kept")?;
    ensure!(
        error.kind() == std::io::ErrorKind::PermissionDenied,
        "the probe's error must reach the caller unchanged; got {error}",
    );
    ensure!(
        superseded.is_file(),
        "the prune must not claim to have removed an entry it could not classify",
    );
    Ok(())
}

/// Verifies an entry that vanishes mid-prune is skipped, not failed.
///
/// The prune runs while the artefact lock is held, so no *publisher* races it.
/// The lock is cooperative, though, and a concurrent `cargo clean` or a
/// build-script glob with side effects can still remove a file between the
/// directory scan and the probe. That entry is not a leftover anyone needs
/// removed, so `NotFound` is answered `false` and the prune continues.
///
/// The disappearance is injected rather than staged by deleting the file:
/// deleting it before the prune means `read_dir` never yields it, so the
/// `NotFound` arm this test names is never reached and the test passes
/// whichever way that arm is written. Injecting the race for an entry that is
/// still present is what actually exercises it.
#[test]
fn an_entry_that_vanishes_mid_prune_is_skipped() -> Result<()> {
    let root = TempRoot::new()?;
    let racing = root.path().join("cli-identifiers.7.json");
    fs::write(&racing, b"{}").context("write the racing fixture")?;

    let outcome = with_entry_probe_error(&racing, std::io::ErrorKind::NotFound, || {
        prune_stale_rendered(root.path(), &[rendered("cli-identifiers.json")])
    });

    outcome.context("a vanished entry must not fail the prune")?;
    ensure!(
        racing.is_file(),
        "the injected race must be the only reason the entry was skipped, so \
         the prune must not have removed a file it could not classify",
    );
    Ok(())
}

/// Verifies a kept file is never probed, so the prune asks only about candidates.
///
/// The injection matches one exact path, so a prune that probed a file the
/// current render keeps would fail here. This pins the short-circuit: the
/// render's own output must not be classified, because an unreadable *current*
/// artefact is not a reason to fail a publication that is about to rewrite it.
#[test]
fn a_file_the_render_keeps_is_not_probed() -> Result<()> {
    let root = TempRoot::new()?;
    let kept = root.path().join("cli-identifiers.json");
    fs::write(&kept, b"{}").context("write current fixture")?;

    with_entry_probe_error(&kept, std::io::ErrorKind::PermissionDenied, || {
        prune_stale_rendered(root.path(), &[rendered("cli-identifiers.json")])
    })
    .context("a file the current render keeps is not a prune candidate")?;

    ensure!(kept.is_file(), "the current render must survive the prune");
    Ok(())
}

/// Verifies a non-rendered entry is never probed either.
///
/// Fragments, the lock, and the channel files all live in this directory and
/// none is a prune candidate. Probing them would make an unreadable lock file
/// fail a publication that does not need to read it.
#[test]
fn a_non_rendered_entry_is_not_probed() -> Result<()> {
    let root = TempRoot::new()?;
    let lock = root.path().join("cli-identifiers.lock");
    fs::write(&lock, b"").context("write lock fixture")?;

    with_entry_probe_error(&lock, std::io::ErrorKind::PermissionDenied, || {
        prune_stale_rendered(root.path(), &[rendered("cli-identifiers.json")])
    })
    .context("a non-rendered entry is not a prune candidate")?;

    ensure!(lock.is_file(), "the lock file must survive the prune");
    Ok(())
}
