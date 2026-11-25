//! Reviewed identities and the `cargo metadata` cross-checks.
//!
//! ADR-0028 requires the gate to confirm the lock against
//! `cargo metadata --locked --offline`, to follow the `clap` → `clap_builder`
//! normal edge, and to refuse an ambiguous resolution before any count is
//! compared. The reviewed constants below are the only place the supported
//! identity is written down; a version bump fails every check until they are
//! updated deliberately.

use anyhow::{Context, Result, bail, ensure};
use camino::Utf8Path;
use cargo_metadata::{DependencyKind, Metadata, MetadataCommand, Node, NodeDep, Package};
use std::time::Duration;
use test_helpers::process::run_with_deadline;

use super::lock::{LockedPackage, dependency_name};

/// Budget for the gate's `cargo metadata` cross-check.
const METADATA_DEADLINE: Duration = Duration::from_secs(120);

/// One reviewed registry package identity.
pub(crate) struct ReviewedPackage {
    /// Package name.
    pub(crate) name: &'static str,
    /// Exact reviewed version.
    pub(crate) version: &'static str,
    /// Registry source identity.
    pub(crate) source: &'static str,
    /// Registry checksum.
    pub(crate) checksum: &'static str,
}

/// The supported `clap` identity this workspace resolves.
pub(crate) const REVIEWED_CLAP: ReviewedPackage = ReviewedPackage {
    name: "clap",
    version: "4.6.7",
    source: "registry+https://github.com/rust-lang/crates.io-index",
    checksum: "aa8876b300ab35ba921adea3dfd70157a46249b33f95c9084ae5709785478946",
};

/// The supported `clap_builder` identity reachable from `clap`.
pub(crate) const REVIEWED_CLAP_BUILDER: ReviewedPackage = ReviewedPackage {
    name: "clap_builder",
    version: "4.6.7",
    source: "registry+https://github.com/rust-lang/crates.io-index",
    checksum: "ec0797fb7aeb1406c84efac526901f7ec3ead2124f946b494e72879d4b54704d",
};

/// Resolve a direct normal dependency of `builder` by name.
///
/// The reviewed external glob needs this: the gate trusts a glob rooted at
/// `anstyle`, so it has to establish that the crate that root names is the one
/// the reviewed graph resolves, rather than take the spelling on trust.
pub(crate) fn resolved_dependency<'a>(
    metadata: &'a Metadata,
    builder: &Package,
    name: &str,
) -> Result<&'a Package> {
    let node = resolve_node(metadata, builder)?;
    normal_dependency(metadata, node, name)
}

/// The reviewed crate behind the source's one external glob.
///
/// The gate trusts `clap_builder`'s `pub use anstyle::*;`, so the crate that
/// name resolves to is pinned here exactly as `clap` and `clap_builder` are: a
/// version or checksum change fails the gate until a maintainer reviews the new
/// contents. That is what makes the trust reviewable — no static audit of an
/// arbitrary crate can prove a negative about every shape it might contain, but
/// a pinned checksum makes any change visible.
pub(crate) const REVIEWED_ANSTYLE: ReviewedPackage = ReviewedPackage {
    name: "anstyle",
    version: "1.0.14",
    source: "registry+https://github.com/rust-lang/crates.io-index",
    checksum: "940b3a0ca603d1eade50a4846a2afffd5ef57a9feac2c0e2ec2e14f9ead76000",
};

/// The facade crate whose production dependency selects `clap`.
const FACADE_PACKAGE: &str = "ortho_config";

/// Load workspace metadata with the gate's locked, offline, host resolution.
///
/// `--offline` keeps the gate hermetic; `--locked` makes Cargo fail rather
/// than update the lock, so metadata and `Cargo.lock` describe one resolution.
/// `--filter-platform host-tuple` keeps the resolve graph to the platform the
/// tests run on: without it Cargo resolves every target, and an offline run
/// then fails when a dependency for another platform has never been fetched.
/// The gate's premise — one unconditional normal `clap` edge — is unaffected,
/// because the facade declares that dependency without a target predicate.
///
/// Cargo runs through the shared deadline-bounded test runner, so a metadata
/// invocation that hangs cannot block the whole suite.
pub(crate) fn load_metadata(manifest_dir: &Utf8Path) -> Result<Metadata> {
    let manifest = manifest_dir.join("Cargo.toml");
    let command = MetadataCommand::new()
        .manifest_path(manifest.as_std_path())
        .other_options(vec![
            "--locked".to_owned(),
            "--offline".to_owned(),
            "--filter-platform".to_owned(),
            "host-tuple".to_owned(),
        ])
        .cargo_command();
    let output = run_with_deadline(
        command,
        METADATA_DEADLINE,
        "cargo metadata --locked --offline",
    )
    .with_context(|| format!("run cargo metadata --locked --offline for {manifest}"))?;
    ensure!(
        output.status.success(),
        "cargo metadata --locked --offline failed for {manifest}: {stderr}",
        stderr = String::from_utf8_lossy(&output.stderr),
    );
    let report =
        std::str::from_utf8(&output.stdout).context("cargo metadata should report UTF-8 JSON")?;
    MetadataCommand::parse(report).context("parse the cargo metadata report")
}

/// Select the unique workspace member named [`FACADE_PACKAGE`].
pub(crate) fn ortho_config_package(metadata: &Metadata) -> Result<&Package> {
    let mut matches = metadata
        .packages
        .iter()
        .filter(|package| package.name == FACADE_PACKAGE);
    let Some(facade) = matches.next() else {
        bail!("cargo metadata has no package named {FACADE_PACKAGE:?}");
    };
    ensure!(
        matches.next().is_none(),
        "cargo metadata lists more than one package named {FACADE_PACKAGE:?}",
    );
    ensure!(
        metadata.workspace_members.contains(&facade.id),
        "{FACADE_PACKAGE} should be a workspace member",
    );
    Ok(facade)
}

/// Resolve the `clap` package across [`FACADE_PACKAGE`]'s normal edge.
pub(crate) fn resolved_clap<'a>(metadata: &'a Metadata, facade: &Package) -> Result<&'a Package> {
    let node = resolve_node(metadata, facade)?;
    normal_dependency(metadata, node, REVIEWED_CLAP.name)
}

/// Resolve the `clap_builder` package across `clap`'s normal facade edge.
pub(crate) fn resolved_clap_builder<'a>(
    metadata: &'a Metadata,
    clap: &Package,
) -> Result<&'a Package> {
    let node = resolve_node(metadata, clap)?;
    normal_dependency(metadata, node, REVIEWED_CLAP_BUILDER.name)
}

/// Select the lock entry describing `package`, by name and exact version.
pub(crate) fn select_lock_package<'a>(
    packages: &'a [LockedPackage],
    package: &Package,
) -> Result<&'a LockedPackage> {
    let name: &str = package.name.as_ref();
    let locked = super::lock::select_unique(packages, name)?;
    ensure!(
        locked.version == package.version.to_string(),
        "Cargo.lock records {} {} but cargo metadata resolved {}",
        locked.name,
        locked.version,
        package.version,
    );
    Ok(locked)
}

/// Confirm one lock entry is exactly the reviewed identity.
pub(crate) fn verify_lock_identity(
    locked: &LockedPackage,
    reviewed: &ReviewedPackage,
) -> Result<()> {
    ensure!(
        locked.name == reviewed.name,
        "Cargo.lock resolves {:?}, but the reviewed package is {:?}",
        locked.name,
        reviewed.name,
    );
    ensure!(
        locked.version == reviewed.version,
        "Cargo.lock resolves {} {}, but the reviewed version is {}",
        locked.name,
        locked.version,
        reviewed.version,
    );
    ensure!(
        locked.source.as_deref() == Some(reviewed.source),
        "Cargo.lock gives {} the source {:?}, but the reviewed source is {:?}",
        locked.name,
        locked.source,
        reviewed.source,
    );
    ensure!(
        locked.checksum.as_deref() == Some(reviewed.checksum),
        "Cargo.lock gives {} the checksum {:?}, but the reviewed checksum is {:?}",
        locked.name,
        locked.checksum,
        reviewed.checksum,
    );
    Ok(())
}

/// Confirm one metadata package is exactly the reviewed identity.
pub(crate) fn verify_metadata_identity(
    package: &Package,
    reviewed: &ReviewedPackage,
) -> Result<()> {
    ensure!(
        package.name == reviewed.name,
        "cargo metadata resolves {:?}, but the reviewed package is {:?}",
        package.name,
        reviewed.name,
    );
    ensure!(
        package.version.to_string() == reviewed.version,
        "cargo metadata resolves {} {}, but the reviewed version is {}",
        package.name,
        package.version,
        reviewed.version,
    );
    let source = metadata_source(package);
    ensure!(
        source.as_deref() == Some(reviewed.source),
        "cargo metadata gives {} the source {:?}, but the reviewed source is {:?}",
        package.name,
        source,
        reviewed.source,
    );
    Ok(())
}

/// Confirm the lock records exactly one `clap` → `clap_builder` dependency.
pub(crate) fn verify_lock_edge(clap: &LockedPackage) -> Result<()> {
    let mut edges = clap
        .dependencies
        .iter()
        .filter(|entry| dependency_name(entry) == REVIEWED_CLAP_BUILDER.name);
    ensure!(
        edges.next().is_some(),
        "Cargo.lock records no {} dependency for {}",
        REVIEWED_CLAP_BUILDER.name,
        REVIEWED_CLAP.name,
    );
    ensure!(
        edges.next().is_none(),
        "Cargo.lock records more than one {} dependency for {}",
        REVIEWED_CLAP_BUILDER.name,
        REVIEWED_CLAP.name,
    );
    Ok(())
}

/// The `resolve` node for `package`.
fn resolve_node<'a>(metadata: &'a Metadata, package: &Package) -> Result<&'a Node> {
    let resolve = metadata
        .resolve
        .as_ref()
        .context("cargo metadata should include a resolved dependency graph")?;
    resolve
        .nodes
        .iter()
        .find(|node| node.id == package.id)
        .with_context(|| format!("cargo metadata has no resolve node for {}", package.name))
}

/// Resolve `name` across `node`'s normal dependency edges only.
///
/// A matched edge that is also a build or development dependency fails: the
/// gate's premise is that the facade selects one production dependency.
fn normal_dependency<'a>(metadata: &'a Metadata, node: &Node, name: &str) -> Result<&'a Package> {
    let mut matches = Vec::new();
    for dependency in &node.deps {
        let package = dependency_package(metadata, dependency)?;
        // Cargo records the import binding in the edge name, so a renamed
        // dependency is reached under its binding while `package.name` stays
        // the underlying package. Matching the package alone would miss a
        // `anstyle = { package = "…" }` rename and could match a different
        // crate that happens to share the package name.
        if dependency.name == name {
            ensure!(
                has_only_normal_kinds(dependency),
                "the {name} edge is not a normal dependency: {:?}",
                dependency.dep_kinds,
            );
            matches.push(package);
        }
    }
    let Some(found) = matches.first().copied() else {
        bail!("the resolved graph has no normal {name} dependency");
    };
    ensure!(
        matches.len() == 1,
        "the resolved graph has {name} more than once"
    );
    Ok(found)
}

/// The package a resolved dependency edge points at.
fn dependency_package<'a>(metadata: &'a Metadata, dependency: &NodeDep) -> Result<&'a Package> {
    metadata
        .packages
        .iter()
        .find(|package| package.id == dependency.pkg)
        .with_context(|| format!("cargo metadata has no package for edge {}", dependency.pkg))
}

/// Whether every recorded kind for this edge is a normal dependency.
fn has_only_normal_kinds(dependency: &NodeDep) -> bool {
    !dependency.dep_kinds.is_empty()
        && dependency
            .dep_kinds
            .iter()
            .all(|info| info.kind == DependencyKind::Normal)
}

/// The package's source identity as metadata reports it.
pub(crate) fn metadata_source(package: &Package) -> Option<String> {
    package.source.as_ref().map(ToString::to_string)
}
