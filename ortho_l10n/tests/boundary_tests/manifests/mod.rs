//! Contract tests: the packaged manifests declare what the release claims.
//!
//! `cargo package` rewrites every `path` dependency into a version
//! requirement and emits a normalized manifest. That normalized file - not the
//! workspace's - is what a stranger's resolver reads, so it is where two
//! separate claims have to hold:
//!
//! - the runtime's published edges are still only its five runtime crates, and
//!   the facade is nowhere among them;
//! - the facade's published edges still point at the runtime and the derive
//!   macros by version, so the facade remains a re-export rather than a copy.
//!
//! Reading the manifest as TOML rather than as text matters here: a crate name
//! mentioned in a comment, in the normalized header, or in a feature table is
//! not a declared edge, and a text search cannot tell the difference.
//!
//! # Why the forbidden check ignores edge kind
//!
//! The first version of this module filtered to normal edges, which left the
//! same manifest free to declare a forbidden crate as a dev- or build-only
//! edge. That is not a corner case: [`crate::boundary::closure`] follows dev
//! edges when it asks what a consumer compiles, and ADR-0015's boundary covers
//! the runtime's focused test graph explicitly. A manifest is either clean of
//! these names in every table or it is not, so the check is kind-blind and
//! target-table-blind and reports the kind it found.

use crate::boundary::closure::{FORBIDDEN_PACKAGES, RUNTIME_NORMAL_EDGES};
use crate::boundary::fixture_cargo::{
    DependencyEdge, EdgeKind, declared_dependencies, declared_path_dependencies, describe_edges,
};
use crate::boundary::package_set::{PACKAGED_CRATES, assert_packaged_set_complete, packaged_crate};
use anyhow::{Context, Result, ensure};

/// The runtime's published edges must be exactly its runtime crates.
///
/// This is the removal claim as a stranger's resolver sees it. A workspace
/// path dependency can resolve without the facet the packaged manifest
/// declares, so the packaged manifest is the stronger statement.
#[test]
fn packaged_runtime_declares_only_its_runtime_crates() -> Result<()> {
    assert_packaged_set_complete()?;
    let packaged = packaged_crate("ortho_l10n")?;
    let declared = declared_dependencies(&packaged.read_manifest()?)?;
    let mut expected = RUNTIME_NORMAL_EDGES.to_vec();
    expected.sort_unstable();
    let mut found = normal_edges(&declared);
    found.sort_unstable();
    ensure!(
        found == expected,
        "the packaged runtime's normal edges are {found:?}, expected {expected:?}\nall edges: {}",
        describe_edges(&declared),
    );
    Ok(())
}

/// The packaged *runtime* must declare no forbidden crate under any table.
///
/// The forbidden list is the issue's removal claim, and the runtime is the root
/// that claim is about. It is deliberately not applied to the facade: the
/// facade is expected to declare Clap itself, so widening the check to it would
/// fail for the correct behaviour. The facade's runtime-facing edges are
/// covered separately by
/// [`packaged_facade_declares_the_runtime_and_the_macros`].
///
/// Every edge kind is checked, including target-conditional tables. A dev-only
/// or build-only declaration is still a resolved edge for the crate's own test
/// and build targets, and a `cfg(windows)` table is invisible to a host-narrowed
/// resolve on Linux, so neither can be excluded without un-asking part of the
/// question.
#[test]
fn packaged_runtime_declares_no_forbidden_edge() -> Result<()> {
    assert_packaged_set_complete()?;
    let packaged = packaged_crate("ortho_l10n")?;
    let declared = declared_dependencies(&packaged.read_manifest()?)?;
    let offending = forbidden_edges(&declared);
    ensure!(
        offending.is_empty(),
        "the packaged runtime declares forbidden edges: {}\nall edges: {}",
        describe_edges(&offending),
        describe_edges(&declared),
    );
    Ok(())
}

/// The facade's published edges must still reach the runtime and the macros.
///
/// If `cargo package` ever stopped rewriting the `path` dependency, or the
/// facade stopped re-exporting the runtime, this is the first place it would
/// show: the facade's own normal edges are what a stranger resolves.
#[test]
fn packaged_facade_declares_the_runtime_and_the_macros() -> Result<()> {
    assert_packaged_set_complete()?;
    let packaged = packaged_crate("ortho_config")?;
    let declared = declared_dependencies(&packaged.read_manifest()?)?;
    for required in ["ortho_l10n", "ortho_config_macros"] {
        ensure!(
            declared
                .iter()
                .any(|edge| edge.package == required && edge.kind == EdgeKind::Normal),
            "the packaged facade does not declare a normal edge on {required}\nall edges: {}",
            describe_edges(&declared),
        );
    }
    Ok(())
}

/// No packaged manifest may declare a dependency by `path`.
///
/// Cargo rewrites every path dependency into a version requirement during
/// packaging. A surviving one would mean the unpacked root is still pointing
/// into this worktree, and every edge measured from it would describe this
/// repository rather than a stranger's graph.
///
/// The check looks at dependency *entries*, not at the text: `[lib] path`,
/// `[[test]] path`, and the like are expected to survive, since they name
/// files inside the package.
#[test]
fn packaged_manifests_carry_no_path_dependencies() -> Result<()> {
    assert_packaged_set_complete()?;
    for (name, _version) in PACKAGED_CRATES {
        let manifest = packaged_crate(name)?.read_manifest()?;
        let offenders = declared_path_dependencies(&manifest)
            .with_context(|| format!("read the packaged {name} manifest"))?;
        ensure!(
            offenders.is_empty(),
            "the packaged {name} manifest declares {} by path",
            offenders.join(", "),
        );
    }
    Ok(())
}

/// The forbidden edges in `declared`, in any table and under any kind.
///
/// The edges are returned rather than described so the caller can report the
/// kind of each offence: "clap (dev)" says which table to open, while a bare
/// name would leave the reader searching a manifest for a line the check has
/// already found.
fn forbidden_edges(declared: &[DependencyEdge]) -> Vec<DependencyEdge> {
    declared
        .iter()
        .filter(|edge| FORBIDDEN_PACKAGES.contains(&edge.package.as_str()))
        .cloned()
        .collect()
}

/// The normal edges of `declared`, as package names.
fn normal_edges(declared: &[DependencyEdge]) -> Vec<String> {
    declared
        .iter()
        .filter(|edge| edge.kind == EdgeKind::Normal)
        .map(|edge| edge.package.clone())
        .collect()
}
