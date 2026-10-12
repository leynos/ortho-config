//! Publication of one derive expansion's fragment into the merged inventory.
//!
//! This module owns the whole write path: take the artefact lock, write the
//! fragment, merge every current fragment, render, prune the superseded
//! layout, and publish. `emit` resolves the ambient environment (`OUT_DIR`,
//! `CARGO_CRATE_NAME`, the opt-in flags) and then hands this boundary a
//! [`Publication`] of explicit values, so the filesystem work can be driven
//! from tests without mutating the process environment or guessing `OUT_DIR`.
//!
//! The lock spans the complete sequence rather than just the write. A peer
//! process that merged before this fragment landed would otherwise replace the
//! combined inventory with a stale rendering; a unique temporary filename
//! fixes the collision on the shared path but leaves that read-modify-write
//! unsynchronised. See `artefact_lock` for the full argument.
//!
//! When the build opts in, the same sequence is also observed: a blocked
//! acquisition announces itself while it waits, and the finished publication
//! records its stage, durations, and outcome. See `artefact_report`.

use std::fs;
use std::path::Path;
use std::time::{Duration, Instant};

use proc_macro2::Span;
use syn::Ident;

use super::artefact_lock::{LockGuard, lock, lock_while};
use super::artefact_observation::{Observation, Subject, observe};
use super::artefact_report::{ErrorCategory, Outcome, ReportConfig, Stage, note_wait};
use super::{
    ARTEFACT_DIR, FRAGMENT_DIR, Fragment, Source, atomic_write, fragment_path, json,
    merge_fragments, prune_stale_rendered, render,
};

/// Carries the explicit inputs for one fragment publication.
///
/// Every ambient value is resolved by the caller and passed in, so a test can
/// publish into a temporary directory under its own control rather than
/// reproducing the sequence with its own copies of the steps.
pub(super) struct Publication<'a> {
    /// Artefact root, already created, holding fragments and rendered output.
    pub(super) root: &'a Path,
    /// Deriving type, which keys the fragment filename.
    pub(super) ident: &'a Ident,
    /// Source location of the expansion, recorded in the fragment.
    pub(super) source: &'a Source,
    /// Crate name recorded against every entry.
    pub(super) crate_name: &'a str,
    /// Entry id, kind, field, and type for each declared identifier.
    pub(super) model: &'a super::LocalizationIds,
    /// Opt-in and threshold for the build-time observation channel.
    pub(super) report: ReportConfig,
}

/// Reports the artefact directory for one `OUT_DIR`, creating it if needed.
///
/// The lock file lives inside the artefact directory, so the directory has to
/// exist before the lock can be taken.
pub(super) fn artefact_root(out_dir: &Path, span: Span) -> syn::Result<std::path::PathBuf> {
    let root = out_dir.join(ARTEFACT_DIR);
    fs::create_dir_all(&root).map_err(|error| {
        syn::Error::new(
            span,
            format!(
                "cannot create identifier artefact directory {}: {error}; unset ORTHO_CONFIG_EMIT_IDENTIFIERS or fix permissions",
                root.display()
            ),
        )
    })?;
    Ok(root)
}

/// Writes one expansion's fragment and republishes the merged inventory.
///
/// The caller must have created the artefact root already; `emit` does that
/// through [`artefact_root`] before calling here.
///
/// The guard is threaded through the steps rather than merely bound here. It
/// is never read, so a plain local would let a later edit drop it early — the
/// release-before-write defect — while every test still passed, because a
/// publisher that acquires and immediately releases still *blocks* on
/// acquisition and so is indistinguishable to an outside observer. Passing the
/// guard by reference makes the borrow checker enforce the ordering instead:
/// no step can run once the guard has been released, and releasing it early no
/// longer compiles.
pub(super) fn publish(publication: &Publication<'_>, span: Span) -> syn::Result<()> {
    let root = publication.root;
    // The deriving type is recorded by its source spelling, which is what the
    // fragment filename and the compiler diagnostic already call it.
    let ident = publication.ident.to_string();
    let observation = Observation::begin(
        Subject {
            root,
            crate_name: publication.crate_name,
            ident: &ident,
            source_file: &publication.source.file,
        },
        publication.report,
    );
    let held = match acquire(&observation) {
        Ok(held) => held,
        Err(error) => {
            // A failed acquisition is one of the two terminal paths that never
            // reaches `Observation::granted`, so the wait marker this
            // publication wrote is retired here. Leaving it behind would make a
            // later build look blocked on a lock nobody holds.
            observation.abandon_wait();
            observation.failed(ErrorCategory::Io);
            observation.finish(Outcome::Failed);
            return Err(error);
        }
    };
    observation.granted(Instant::now());
    let outcome = observe(&observation, ErrorCategory::Io, || {
        write_all(&held, &observation, publication, span)
    });
    if outcome.is_ok() {
        observation.enter(Stage::Publish);
        observation.finish(Outcome::Completed);
    }
    outcome
}

/// Waits for the artefact lock, announcing a prolonged wait while it blocks.
///
/// `lock_while` reports the wait from inside the acquisition loop, so the
/// marker exists *during* the stall rather than only in a duration recorded
/// once the lock is already granted — the difference between explaining a hung
/// build and describing a completed one.
///
/// A build that did not opt in takes the blocking path instead. Nothing would
/// read the marker, and polling would make the common case pay for a channel
/// it never enabled.
fn acquire(observation: &Observation<'_>) -> syn::Result<LockGuard> {
    if !observation.is_on() {
        return lock(observation.root());
    }
    let refresh = observation.refresh();
    let mut announced: Option<Duration> = None;
    lock_while(observation.root(), |waited| {
        if waited < observation.threshold() {
            return;
        }
        // Announce once the threshold is crossed, then refresh on the slow
        // interval so the marker keeps showing a growing wait without a write
        // per poll.
        if announced.is_none_or(|last| waited.saturating_sub(last) >= refresh) {
            announced = Some(waited);
            note_wait(observation.root(), waited);
        }
    })
}

/// Runs the locked part of one publication, recording the stage it reaches.
///
/// The stage is entered *before* the fragment directory is created. Creating
/// it is part of writing the fragment, not part of waiting for the lock, and
/// the lock has already been granted by the time this runs. Recording
/// [`Stage::LockWait`] for a directory failure would tell a maintainer the
/// build was blocked on a peer when it was not.
fn write_all(
    held: &LockGuard,
    observation: &Observation<'_>,
    publication: &Publication<'_>,
    span: Span,
) -> syn::Result<()> {
    let root = publication.root;
    observation.enter(Stage::FragmentWrite);
    fs::create_dir_all(root.join(FRAGMENT_DIR)).map_err(|error| {
        syn::Error::new(
            span,
            format!(
                "cannot create identifier artefact directory {}: {error}; unset ORTHO_CONFIG_EMIT_IDENTIFIERS or fix permissions",
                root.display()
            ),
        )
    })?;
    write_fragment(held, publication, span)?;
    republish(held, observation, root, span)
}

/// Serialises and atomically writes the fragment for one expansion.
///
/// `_held` is a proof token, not an input: it is never read, and exists so the
/// borrow checker rejects a caller that released the lock first.
fn write_fragment(_held: &LockGuard, publication: &Publication<'_>, span: Span) -> syn::Result<()> {
    let fragment = Fragment {
        source_file: publication.source.file.clone(),
        entries: super::entries(
            publication.model,
            publication.ident,
            span,
            publication.crate_name,
        ),
    };
    let path = fragment_path(publication.root, publication.ident, publication.source);
    let contents = json(&fragment).map_err(|error| {
        syn::Error::new(
            span,
            format!("cannot serialize identifier artefact: {error}"),
        )
    })?;
    atomic_write(&path, &contents).map_err(|error| {
        syn::Error::new(
            span,
            format!(
                "cannot write identifier artefact {}: {error}; unset ORTHO_CONFIG_EMIT_IDENTIFIERS or fix permissions",
                path.display()
            ),
        )
    })
}

/// Renders the merged inventory and makes the directory match it exactly.
///
/// `_held` carries the same proof obligation as in [`write_fragment`]: the
/// merge, the prune, and the rendered writes must not run without the lock.
fn republish(
    _held: &LockGuard,
    observation: &Observation<'_>,
    root: &Path,
    span: Span,
) -> syn::Result<()> {
    observation.enter(Stage::Merge);
    let merged = observe(observation, ErrorCategory::Serialization, || {
        merge_fragments(root).map_err(|error| syn::Error::new(span, error))
    })?;
    observation.enter(Stage::Render);
    let rendered = observe(observation, ErrorCategory::Serialization, || {
        render(merged).map_err(|error| {
            syn::Error::new(
                span,
                format!(
                    "cannot render identifier artefacts in {}: {error}",
                    root.display()
                ),
            )
        })
    })?;
    // Prune before publishing so a reader never observes a directory holding
    // both the new layout's files and unpruned leftovers from the old one.
    observation.enter(Stage::Prune);
    observe(observation, ErrorCategory::Io, || {
        prune_stale_rendered(root, &rendered).map_err(|error| {
            syn::Error::new(
                span,
                format!(
                    "cannot prune superseded identifier artefacts in {}: {error}",
                    root.display()
                ),
            )
        })
    })?;
    observation.enter(Stage::Publish);
    observe(observation, ErrorCategory::Io, || {
        for file in rendered {
            let path = root.join(&file.name);
            atomic_write(&path, &file.contents).map_err(|error| {
                syn::Error::new(
                    span,
                    format!(
                        "cannot write identifier artefact {}: {error}; unset ORTHO_CONFIG_EMIT_IDENTIFIERS or fix permissions",
                        path.display()
                    ),
                )
            })?;
        }
        Ok(())
    })
}
