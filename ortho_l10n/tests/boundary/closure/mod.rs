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
//! The third property is what keeps the first two honest. Without it a typo in
//! a crate name, or a fixture that failed to resolve at all, would read as
//! success.
//!
//! Resolution succeeding is part of the claim, not a detail of the fixture. A
//! control exists to show that [`assert_absent_packages`] can reject a
//! forbidden package that is present in a graph; if Cargo refuses the resolve,
//! that check never ran. A refusal is not a weaker form of the same evidence,
//! because an `--offline` resolve whose crate is missing from the package
//! cache fails with a diagnostic that names the crate - indistinguishable, by
//! name alone, from a deliberate rejection. [`run_negative_control`] therefore
//! reports a refusal as an error rather than as a pass.
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
    repository_root, toml_string,
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

/// Every way a control can fail before it proves anything.
///
/// Kept as an enum so the message distinguishes a resolver refusal - which is
/// an infrastructure condition, reported with Cargo's own words - from an
/// injection that resolved and was accepted, which is a real defect in the
/// check under test.
#[derive(Debug, thiserror::Error)]
enum ControlFailure {
    /// Cargo refused the resolve, so the closure check never ran.
    #[error(
        "the {injection} never resolved, so the closure check could not reject it; \
         Cargo said: {detail}"
    )]
    Refused {
        /// The injection whose fixture was refused.
        injection: String,
        /// Cargo's explanation, in full.
        detail: String,
    },
    /// The injected edge resolved and the closure check accepted it.
    #[error("the {injection} resolved and the closure check accepted it: {detail}")]
    Accepted {
        /// The injection that was accepted.
        injection: String,
        /// The full graph, so a reader can see what the check was looking at.
        detail: String,
    },
}

/// Run one negative control: inject the edge, resolve, and require rejection.
///
/// The control is evidence only when Cargo *resolved* the injected edge and
/// [`assert_absent_packages`] then rejected it by name. Both halves matter:
///
/// - if the resolve fails, the check under test never ran, so nothing has been
///   demonstrated. This is not tolerated even when the diagnostic names the
///   injected crate, because an offline resolve of an uncached crate fails with
///   exactly such a message and is indistinguishable from a real rejection by
///   name alone;
/// - if the resolve succeeds and the check accepts the edge, the control has
///   found a real hole.
///
/// Returns the closure check's rejection message on success, which is what a
/// reader sees in the log if the control later turns out to have been vacuous.
///
/// # Errors
///
/// Returns an error when Cargo refuses the resolve, or when the injected edge
/// resolves and the closure check accepts it.
pub(crate) fn run_negative_control(injection: Injection) -> Result<String> {
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
        return Err(ControlFailure::Refused {
            injection: injection.describe(),
            detail: String::from_utf8_lossy(&output.stderr).into_owned(),
        }
        .into());
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
        .map(|error| error.to_string())
        .with_context(|| ControlFailure::Accepted {
            injection: injection.describe(),
            detail: metadata.package_names().join(", "),
        })?;
    Ok(detail)
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
