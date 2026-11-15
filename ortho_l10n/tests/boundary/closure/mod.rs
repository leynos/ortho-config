//! Dependency-closure contracts for a standalone runtime consumer.
//!
//! The claim under test is a *removal*: an application that only formats
//! localized messages must not compile configuration discovery, the derive
//! macros, Figment, or Clap. A closure claim is easy to state and easy to
//! weaken, so this module enforces it three ways:
//!
//! - every forbidden package is absent by name;
//! - the runtime's own resolved edges are exactly the runtime's declared
//!   dependencies, classified as normal, build, or root dev edges;
//! - a negative control injects each forbidden edge, *resolves* it, and then
//!   requires the closure check to reject it by name.
//!
//! The third property is what keeps the first two honest, and it lives in
//! [`control`] because "what a clean graph looks like" and "what happens when
//! an injection is present" are separate claims with separate failure modes.
//! Without it a typo in a crate name, or a fixture that failed to resolve at
//! all, would read as success.
//!
//! # The graph is the host's, and the manifests are every platform's
//!
//! Two of the three checks read a resolved graph, and that graph is narrowed to
//! the host triple. It has to be: `cargo metadata` otherwise includes every
//! platform's dependencies, and under `--offline` it must then download crates
//! the host will never build. A runner whose cache was warmed by building the
//! host has no reason to hold those - `figment` gates `atomic` on
//! `target_pointer_width`, which a 64-bit runner never satisfies - so the
//! resolve failed for a reason that had nothing to do with the edge under test.
//!
//! Narrowing does not weaken the claim. Whether the runtime *builds* without
//! the forbidden crates is a host question, and the host graph answers it
//! exactly. Whether it *declares* one is a platform-independent question about
//! manifest text, and [`declared_dependencies`] answers that over every
//! platform's tables at once, including `[target.'cfg(windows)'.dependencies]`
//! on a Linux runner.

use super::fixture_cargo::{DependencyEdge, EdgeKind, Metadata, describe_edges};
use anyhow::{Result, ensure};

pub(crate) mod control;
pub(crate) use control::{Injection, run_negative_control};

/// Packages that must never appear in a standalone runtime consumer's graph.
///
/// The list is the removal claim from issue #566: configuration discovery
/// (`figment`, `ortho_config`), the derive macros (`ortho_config_macros`), the
/// command-line layer (`clap`), and the workspace's shared test scaffolding.
/// The `clap_*` entries are Clap's own split-out crates; naming them as well
/// means a future Clap restructure cannot slip past a bare `clap` check.
pub(crate) const FORBIDDEN_PACKAGES: &[&str] = &[
    "clap",
    "clap_builder",
    "clap_derive",
    "clap_lex",
    "figment",
    "ortho_config",
    "ortho_config_macros",
    "ortho_config_test_helpers",
];

/// Dependency edges the standalone runtime is expected to have.
///
/// This is the normal-edge half of the closure claim. It is asserted positively
/// so that a *missing* edge fails too: an over-shrunk closure that dropped
/// Fluent would otherwise pass the forbidden-name checks vacuously.
pub(crate) const RUNTIME_NORMAL_EDGES: &[&str] = &[
    "fluent-bundle",
    "fluent-syntax",
    "thiserror",
    "tracing",
    "unic-langid",
];

/// The consumer root's package name, used to find its resolve node.
pub(crate) const CONSUMER_PACKAGE: &str = "boundary-consumer";

/// Assert that no forbidden package appears anywhere in `metadata`.
///
/// # Errors
///
/// Returns an error naming every forbidden package that resolved, with its
/// version and source, so the message shows where the edge came from.
pub(crate) fn assert_no_forbidden_packages(metadata: &Metadata) -> Result<()> {
    assert_absent_packages(metadata, FORBIDDEN_PACKAGES)
}

/// Assert that none of `forbidden` appears anywhere in `metadata`.
///
/// The forbidden set is a parameter so a negative control can forbid *only* the
/// crate it injected. Checking the full list instead is vacuous: the
/// `DevTestHelpers` injection also drags in `figment`, so the control stayed
/// green on `figment`'s account even with the helper crate removed from
/// [`FORBIDDEN_PACKAGES`], proving nothing about the edge under test.
///
/// # Errors
///
/// Returns an error naming every forbidden package that resolved.
pub(crate) fn assert_absent_packages(metadata: &Metadata, forbidden: &[&str]) -> Result<()> {
    let offending = metadata
        .packages
        .iter()
        .filter(|package| forbidden.contains(&package.name.as_str()))
        .map(|package| {
            let source = package.source.as_deref().unwrap_or("a path package");
            format!("{} {} from {source}", package.name, package.version)
        })
        .collect::<Vec<_>>();
    ensure!(
        offending.is_empty(),
        concat!(
            "a standalone ortho_l10n consumer resolved forbidden packages: {}\n",
            "the resolved graph is: {}"
        ),
        offending.join(", "),
        metadata.package_names().join(", "),
    );
    Ok(())
}

/// Assert that no forbidden package is reachable from `root` in `metadata`.
///
/// This is the closure claim as a *consumer* experiences it, and it is
/// deliberately stronger than "the root declares no forbidden edge". A
/// forbidden crate can enter the graph at any depth, so only the transitive
/// closure answers the question. Dev edges are followed as well, because Cargo
/// resolves them for the crate's own test targets and an exclusion there would
/// leave exactly the graph the issue complains about unexamined.
///
/// # Errors
///
/// Returns an error naming every forbidden package that is reachable.
pub(crate) fn assert_no_forbidden_reachable(metadata: &Metadata, root: &str) -> Result<()> {
    let reachable = metadata.reachable(root)?;
    let offending = FORBIDDEN_PACKAGES
        .iter()
        .filter(|forbidden| reachable.contains(forbidden))
        .copied()
        .collect::<Vec<_>>();
    ensure!(
        offending.is_empty(),
        "the dependency closure rooted at {root} reaches forbidden packages: {}\n\
         the reachable graph is: {}",
        offending.join(", "),
        reachable.join(", "),
    );
    Ok(())
}

/// Assert the runtime package has exactly the expected normal edges.
///
/// # Errors
///
/// Returns an error when the runtime node is missing or its normal edges differ
/// from [`RUNTIME_NORMAL_EDGES`] in either direction.
pub(crate) fn assert_runtime_normal_edges(metadata: &Metadata) -> Result<()> {
    let edges = metadata.edges("ortho_l10n")?;
    let mut found = edges
        .iter()
        .filter(|edge| edge.kind == EdgeKind::Normal)
        .map(|edge| edge.package.as_str())
        .collect::<Vec<_>>();
    found.sort_unstable();
    let mut expected = RUNTIME_NORMAL_EDGES.to_vec();
    expected.sort_unstable();
    ensure!(
        found == expected,
        "ortho_l10n's normal edges are {found:?}, expected {expected:?}\nall edges: {}",
        describe_edges(&edges),
    );
    Ok(())
}

/// Assert that the consumer root has exactly one normal edge, to the runtime.
///
/// The root's own dependency is the one edge the contract controls. A dev or
/// build edge to the runtime would widen the published graph a stranger has to
/// resolve, so it is rejected explicitly rather than tolerated.
///
/// # Errors
///
/// Returns an error when the root's edges differ from the expected single
/// normal edge.
pub(crate) fn assert_root_edges(metadata: &Metadata) -> Result<()> {
    let edges = metadata.edges(CONSUMER_PACKAGE)?;
    let expected = [DependencyEdge {
        package: String::from("ortho_l10n"),
        kind: EdgeKind::Normal,
    }];
    ensure!(
        edges == expected,
        "the consumer root's edges are {}, expected {}",
        describe_edges(&edges),
        describe_edges(&expected),
    );
    Ok(())
}
