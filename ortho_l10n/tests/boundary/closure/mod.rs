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
//! - a negative control injects each forbidden edge and must fail *for that
//!   reason*, naming the injected dependency.
//!
//! The third property is what keeps the first two honest. Without it a typo in
//! a crate name, or a fixture that failed to resolve at all, would read as
//! success.
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

use super::fixture_cargo::{
    DependencyEdge, EdgeKind, FixtureCargo, Metadata, absolute_member, describe_edges,
    ensure_failure_names, repository_root, toml_string,
};
use anyhow::{Context, Result, ensure};
use camino::Utf8Path;
use std::fmt::Write as _;

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

/// The fixture families a negative control can inject.
///
/// Each variant names the manifest table that carries the injected edge, so the
/// failure message can say which shape of edge was accepted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Injection {
    /// A normal dependency on Clap.
    NormalClap,
    /// A build-script dependency on Clap.
    BuildClap,
    /// A test-only dependency on Clap.
    DevClap,
    /// A test-only dependency on the workspace's shared test scaffolding.
    DevTestHelpers,
}

impl Injection {
    /// The crate the injection pulls in, which the failure must name.
    pub(crate) const fn injected_crate(self) -> &'static str {
        match self {
            Self::NormalClap | Self::BuildClap | Self::DevClap => "clap",
            Self::DevTestHelpers => "ortho_config_test_helpers",
        }
    }

    /// The manifest table the injected edge is written under.
    pub(crate) const fn table(self) -> &'static str {
        match self {
            Self::NormalClap => "dependencies",
            Self::BuildClap => "build-dependencies",
            Self::DevClap | Self::DevTestHelpers => "dev-dependencies",
        }
    }

    /// The edge classification the injection should have produced.
    pub(crate) const fn kind(self) -> EdgeKind {
        match self {
            Self::NormalClap => EdgeKind::Normal,
            Self::BuildClap => EdgeKind::Build,
            Self::DevClap | Self::DevTestHelpers => EdgeKind::Dev,
        }
    }

    /// Spelling for assertions and failure messages.
    pub(crate) fn describe(self) -> String {
        format!(
            "{} edge on {} under [{}]",
            self.kind(),
            self.injected_crate(),
            self.table(),
        )
    }
}

/// Why a negative control failed before it could prove anything.
///
/// The distinction matters: "the injection was rejected" and "the fixture was
/// never resolved" both exit non-zero, and only the first is evidence.
#[derive(Debug)]
pub(crate) enum ControlOutcome {
    /// Cargo refused the injected edge, naming the injected crate.
    Rejected {
        /// The line Cargo printed about the injected crate.
        detail: String,
    },
    /// Cargo resolved the graph; the closure check then failed on the name.
    AcceptedButCaughtByName {
        /// The offending packages the closure check reported.
        detail: String,
    },
}

/// Run one negative control: inject the edge, resolve, and require a failure.
///
/// Two outcomes are legitimate, and both are reported rather than flattened:
/// resolution may refuse the injected crate outright, or it may resolve and be
/// caught by [`assert_absent_packages`] naming that crate. Anything else - a
/// fixture that failed for an unrelated reason, or an injection that slipped
/// through - is an error.
///
/// # Errors
///
/// Returns an error when Cargo fails without naming the injected crate, or when
/// the injected edge resolves and the closure check accepts it.
pub(crate) fn run_negative_control(injection: Injection) -> Result<ControlOutcome> {
    let fixture = FixtureCargo::new()?;
    fixture.write(Utf8Path::new("src/lib.rs"), "//! Negative control.\n")?;
    fixture.write(
        Utf8Path::new("Cargo.toml"),
        &negative_control_manifest(injection)?,
    )?;
    // Resolve through the fixture's one host-narrowed entry point: a control
    // that built its own argument list could drift from the contracts it is
    // meant to guard, and did.
    let output = fixture.resolve()?;
    if !output.status.success() {
        ensure_failure_names(&output, injection.injected_crate())?;
        let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
        let detail = stderr
            .lines()
            .find(|line| line.contains(injection.injected_crate()))
            .unwrap_or_else(|| stderr.lines().next().unwrap_or("cargo printed nothing"))
            .trim()
            .to_owned();
        return Ok(ControlOutcome::Rejected { detail });
    }
    // Resolution succeeded: the injected edge is real, so the closure check
    // must be the thing that refuses it - and it must refuse it *because of the
    // injected crate*. Forbidding the whole list instead is vacuous:
    // `DevTestHelpers` also drags in `figment`, so the control stayed green on
    // `figment`'s account even with the helper crate removed from
    // `FORBIDDEN_PACKAGES`. It was re-proving that `figment` is forbidden,
    // which the runtime-closure contracts already prove, and saying nothing
    // about the edge under test.
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    let metadata = Metadata::parse(&stdout)?;
    let injected = injection.injected_crate();
    let detail = assert_absent_packages(&metadata, &[injected])
        .err()
        .with_context(|| {
            format!(
                "{} resolved and the closure check accepted it; the control proves nothing",
                injection.describe(),
            )
        })?
        .to_string();
    Ok(ControlOutcome::AcceptedButCaughtByName { detail })
}

/// Render a standalone runtime consumer manifest with one injected edge.
///
/// The member paths are absolute. A relative spelling is resolved against the
/// fixture root, which lives under `target/` rather than beside the members, so
/// the control would fail to find `ortho_l10n` and report that instead of
/// anything about the injected edge.
fn negative_control_manifest(injection: Injection) -> Result<String> {
    let repository = repository_root()?;
    let runtime_path = toml_string(&absolute_member(&repository, Utf8Path::new("ortho_l10n"))?);
    let helpers_path = toml_string(&absolute_member(
        &repository,
        Utf8Path::new("test_helpers"),
    )?);

    // A normal injection shares `[dependencies]` with the runtime, so both
    // entries are written under one heading. Emitting a heading twice is a TOML
    // duplicate-key error, which would fail the control for a reason that names
    // no crate at all.
    let mut manifest = String::new();
    writeln!(manifest, "[package]")?;
    writeln!(manifest, "name = \"{CONSUMER_PACKAGE}\"")?;
    writeln!(manifest, "version = \"0.0.0\"")?;
    writeln!(manifest, "edition = \"2024\"")?;
    writeln!(manifest)?;
    writeln!(manifest, "[workspace]")?;
    writeln!(manifest)?;
    writeln!(manifest, "[dependencies]")?;
    writeln!(manifest, "ortho_l10n = {{ path = {runtime_path} }}")?;
    if matches!(injection, Injection::NormalClap) {
        write_injected_edge(&mut manifest, injection, &helpers_path)?;
    }
    writeln!(manifest)?;
    if !matches!(injection, Injection::NormalClap) {
        writeln!(manifest, "[{}]", injection.table())?;
        write_injected_edge(&mut manifest, injection, &helpers_path)?;
    }
    Ok(manifest)
}

/// Write the one dependency line an injection adds.
fn write_injected_edge(
    manifest: &mut String,
    injection: Injection,
    helpers_path: &str,
) -> Result<()> {
    match injection {
        Injection::NormalClap | Injection::BuildClap | Injection::DevClap => {
            // A version requirement rather than a path: the control must not
            // depend on the workspace's Clap version pin staying reachable.
            writeln!(manifest, "clap = \"4\"")?;
        }
        Injection::DevTestHelpers => {
            writeln!(
                manifest,
                "ortho_config_test_helpers = {{ path = {helpers_path} }}"
            )?;
        }
    }
    Ok(())
}
