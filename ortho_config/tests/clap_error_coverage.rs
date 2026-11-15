//! Pre-publication coverage gate for the resolved clap error inventory.
//!
//! ADR-0022 makes this the mechanical signal checked before publishing: the
//! workspace lock and `cargo metadata --locked --offline` must agree on the
//! supported `clap` and `clap_builder` identities, `clap_builder` must be
//! reached through exactly one normal facade edge, and the `ErrorKind` source
//! resolved through metadata must declare exactly the reviewed number of
//! variants. The gate is test-only and workspace-scoped; it never rejects a
//! consumer build and never runs in a build script.
//!
//! Each step refuses with the check that failed, so a red gate names the
//! identity, edge, or count that drifted. `support/clap_publication/controls.rs`
//! holds the committed fixtures proving every refusal fires for its stated
//! reason.

#[path = "support/clap_publication/mod.rs"]
mod clap_publication;
#[path = "support/clap_reviewed.rs"]
mod clap_reviewed;

use anyhow::Result;
use camino::Utf8Path;
use cargo_metadata::{Metadata, Package};
use clap_publication::{
    Lockfile, REVIEWED_CLAP, REVIEWED_CLAP_BUILDER, compare_count, count_resolved_error_kinds,
    load_metadata, ortho_config_package, read_text, resolved_clap, resolved_clap_builder,
    select_lock_package, verify_lock_edge, verify_lock_identity, verify_metadata_identity,
};

/// The resolved builder inventory equals the reviewed total.
///
/// The work is split into one stage per published step so each stage reads as
/// its own check. A single body carrying every stage reads as a wall of
/// fallible calls in which the identity, edge, and count checks are hard to
/// tell apart, and the lint gate scores such a body as several separated
/// clusters of conditional logic.
#[test]
fn resolved_clap_builder_inventory_matches_the_reviewed_total() -> Result<()> {
    let manifest_dir = Utf8Path::new(env!("CARGO_MANIFEST_DIR"));
    let metadata = load_metadata(manifest_dir)?;
    let parsed = verified_inventory(&metadata)?;
    compare_count(parsed, clap_reviewed::CLAP_ERROR_KIND_COUNT)
}

/// Resolve the reviewed packages and verify the resolved inventory.
///
/// Metadata must name the reviewed `clap` and `clap_builder` identities, the
/// lock must agree with them, and the source must declare the resolved total.
fn verified_inventory(metadata: &Metadata) -> Result<usize> {
    let (clap, builder) = verified_metadata(metadata)?;
    verified_lock(metadata, clap, builder)?;
    count_resolved_error_kinds(builder)
}

/// Resolve `clap` and `clap_builder` and verify both metadata identities.
fn verified_metadata(metadata: &Metadata) -> Result<(&Package, &Package)> {
    let facade = ortho_config_package(metadata)?;
    let clap = resolved_clap(metadata, facade)?;
    let builder = resolved_clap_builder(metadata, clap)?;
    verify_metadata_identity(clap, &REVIEWED_CLAP)?;
    verify_metadata_identity(builder, &REVIEWED_CLAP_BUILDER)?;
    Ok((clap, builder))
}

/// Verify the locked identities and the single normal facade edge.
fn verified_lock(metadata: &Metadata, clap: &Package, builder: &Package) -> Result<()> {
    let lock_text = read_text(&metadata.workspace_root.join("Cargo.lock"))?;
    let lock = Lockfile::parse(&lock_text)?;
    let locked_clap = select_lock_package(lock.packages(), clap)?;
    let locked_builder = select_lock_package(lock.packages(), builder)?;
    verify_lock_identity(locked_clap, &REVIEWED_CLAP)?;
    verify_lock_identity(locked_builder, &REVIEWED_CLAP_BUILDER)?;
    verify_lock_edge(locked_clap)
}
