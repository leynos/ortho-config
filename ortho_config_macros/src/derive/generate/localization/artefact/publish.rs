//! Publication of one derive expansion's fragment into the merged inventory.
//!
//! This module owns the whole write path: take the artefact lock, write the
//! fragment, merge every current fragment, render, prune the superseded
//! layout, and publish. `emit` resolves the ambient environment (`OUT_DIR`,
//! `CARGO_CRATE_NAME`, the opt-in flag) and then hands this boundary a
//! [`Publication`] of explicit values, so the filesystem work can be driven
//! from tests without mutating the process environment or guessing `OUT_DIR`.
//!
//! The lock spans the complete sequence rather than just the write. A peer
//! process that merged before this fragment landed would otherwise replace the
//! combined inventory with a stale rendering; a unique temporary filename
//! fixes the collision on the shared path but leaves that read-modify-write
//! unsynchronised. See `artefact_lock` for the full argument.

use std::fs;
use std::path::Path;

use proc_macro2::Span;
use syn::Ident;

use super::artefact_lock::lock;
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
pub(super) fn publish(publication: &Publication<'_>, span: Span) -> syn::Result<()> {
    let root = publication.root;
    // Hold the lock across the fragment write, the merge, the prune, and the
    // publication so no peer can observe or overwrite a partial inventory.
    let _lock = lock(root)?;
    fs::create_dir_all(root.join(FRAGMENT_DIR)).map_err(|error| {
        syn::Error::new(
            span,
            format!(
                "cannot create identifier artefact directory {}: {error}; unset ORTHO_CONFIG_EMIT_IDENTIFIERS or fix permissions",
                root.display()
            ),
        )
    })?;

    write_fragment(publication, span)?;
    republish(root, span)
}

/// Serialises and atomically writes the fragment for one expansion.
fn write_fragment(publication: &Publication<'_>, span: Span) -> syn::Result<()> {
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
fn republish(root: &Path, span: Span) -> syn::Result<()> {
    let rendered = render(merge_fragments(root).map_err(|error| syn::Error::new(span, error))?)
        .map_err(|error| syn::Error::new(span, error.to_string()))?;
    // Prune before publishing so a reader never observes a directory holding
    // both the new layout's files and unpruned leftovers from the old one.
    prune_stale_rendered(root, &rendered).map_err(|error| {
        syn::Error::new(
            span,
            format!(
                "cannot prune superseded identifier artefacts in {}: {error}",
                root.display()
            ),
        )
    })?;
    for file in rendered {
        atomic_write(&root.join(file.name), &file.contents)
            .map_err(|error| syn::Error::new(span, error.to_string()))?;
    }
    Ok(())
}
