//! Contract tests: the packaged manifests declare what the release claims.
//!
//! `cargo package` rewrites every `path` dependency into a version
//! requirement and emits a normalised manifest. That normalised file - not the
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

use crate::boundary::closure::{FORBIDDEN_PACKAGES, RUNTIME_NORMAL_EDGES};
use crate::boundary::fixture_cargo::{
    DependencyEdge, EdgeKind, declared_dependencies, describe_edges,
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
    let normal = normal_edges(&declared);
    let mut expected = RUNTIME_NORMAL_EDGES.to_vec();
    expected.sort_unstable();
    let mut found = normal;
    found.sort_unstable();
    ensure!(
        found == expected,
        "the packaged runtime's normal edges are {found:?}, expected {expected:?}\nall edges: {}",
        describe_edges(&declared),
    );
    Ok(())
}

/// No packaged manifest may declare a forbidden crate as a normal edge.
///
/// The forbidden list is the issue's removal claim, so it is applied to every
/// published root rather than only to the runtime: a facade that leaked Clap
/// into the runtime's *build* graph would be a different defect, and this test
/// deliberately says nothing about the facade's own Clap edge, which is
/// expected.
#[test]
fn no_packaged_manifest_declares_a_forbidden_runtime_edge() -> Result<()> {
    assert_packaged_set_complete()?;
    let packaged = packaged_crate("ortho_l10n")?;
    let declared = declared_dependencies(&packaged.read_manifest()?)?;
    let offending = declared
        .iter()
        .filter(|edge| edge.kind == EdgeKind::Normal)
        .filter(|edge| FORBIDDEN_PACKAGES.contains(&edge.package.as_str()))
        .map(ToString::to_string)
        .collect::<Vec<_>>();
    ensure!(
        offending.is_empty(),
        "the packaged runtime declares forbidden runtime edges: {}\nall edges: {}",
        offending.join(", "),
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
        let document: toml::Table = toml::from_str(&manifest)
            .with_context(|| format!("parse the packaged {name} manifest"))?;
        let offenders = path_dependencies(&document);
        ensure!(
            offenders.is_empty(),
            "the packaged {name} manifest declares {} by path",
            offenders.join(", "),
        );
    }
    Ok(())
}

/// The normal edges of `declared`, as package names.
fn normal_edges(declared: &[DependencyEdge]) -> Vec<String> {
    declared
        .iter()
        .filter(|edge| edge.kind == EdgeKind::Normal)
        .map(|edge| edge.package.clone())
        .collect()
}

/// Every dependency name in `document` whose entry carries a `path` key.
fn path_dependencies(document: &toml::Table) -> Vec<String> {
    let mut offenders = Vec::new();
    collect_path_dependencies(document, &mut offenders);
    offenders
}

/// Walk the dependency tables of `table`, including target-scoped ones.
fn collect_path_dependencies(table: &toml::Table, offenders: &mut Vec<String>) {
    for heading in ["dependencies", "dev-dependencies", "build-dependencies"] {
        let entries = table.get(heading).and_then(toml::Value::as_table);
        for (name, entry) in entries.into_iter().flat_map(toml::Table::iter) {
            if entry.get("path").is_some() {
                offenders.push(name.clone());
            }
        }
    }
    let sections = table.get("target").and_then(toml::Value::as_table);
    for scope in sections.into_iter().flat_map(toml::Table::values) {
        if let Some(scope_table) = scope.as_table() {
            collect_path_dependencies(scope_table, offenders);
        }
    }
}
