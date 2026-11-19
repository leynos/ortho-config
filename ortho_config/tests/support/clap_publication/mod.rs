//! Pre-publication identity and inventory helpers for the clap coverage gate.
//!
//! ADR-0024 keeps the gate test-only and workspace-scoped, and requires that
//! every refusal names the check that failed. This module therefore keeps the
//! gate's decisions out of the test body: `lock` parses and selects from
//! `Cargo.lock`, `identity` holds the reviewed identities and the metadata
//! cross-checks, `source` locates and counts `ErrorKind`, `library` settles
//! which file Cargo compiles as the crate root, `audit` holds the attribute,
//! declaration-shape and item-macro refusals, `symlinks` stages the trees that
//! prove which symlinks the walk may ignore and which it must refuse, and
//! `files` confines the reads to capability handles.
//! `tests/clap_error_coverage.rs` wires them together in the published order:
//! metadata, lock, identity, edge, count.

mod audit;
mod files;
mod identity;
mod library;
mod lock;
mod source;

#[cfg(test)]
mod classes;

#[cfg(test)]
mod controls;

#[cfg(test)]
mod properties;

#[cfg(all(test, unix))]
mod symlinks;

use anyhow::{Result, ensure};

pub(crate) use files::read_text;
pub(crate) use identity::{
    REVIEWED_CLAP, REVIEWED_CLAP_BUILDER, load_metadata, ortho_config_package, resolved_clap,
    resolved_clap_builder, select_lock_package, verify_lock_edge, verify_lock_identity,
    verify_metadata_identity,
};
pub(crate) use lock::Lockfile;
pub(crate) use source::count_resolved_error_kinds;

/// Compare the parsed source inventory with the reviewed total.
///
/// The failure text names both numbers and the constant to update, because the
/// only correct repair is a human review of the new `clap_builder` variant.
pub(crate) fn compare_count(parsed: usize, reviewed: usize) -> Result<()> {
    ensure!(
        parsed == reviewed,
        "clap_builder declares {parsed} ErrorKind variants but the reviewed total is {reviewed}; \
         review the change, then update CLAP_ERROR_KIND_COUNT and the matrix together",
    );
    Ok(())
}
