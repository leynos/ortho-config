//! Negative controls for the dependency-closure contracts.
//!
//! A closure claim is only as good as its ability to fail, so every forbidden
//! dependency the boundary names is injected into a rendered consumer root and
//! the contract must then refuse it. The machinery lives here rather than in
//! the parent module so that the assertion helpers and the injection fixtures
//! stay separately readable: the parent states what a clean graph looks like,
//! and this module states what happens when it is not one.
//!
//! # What counts as evidence
//!
//! The control is evidence only when Cargo *resolved* the injected edge and the
//! closure check then rejected it by name. A refused resolve is an error, not a
//! weaker form of the same evidence: an offline resolve of an uncached crate
//! fails with a diagnostic that names the crate, so accepting a refusal would
//! let an infrastructure condition stand in for a working check. See
//! [`ControlFailure`] for the two failure modes this module distinguishes.

use super::super::fixture_cargo::{
    EdgeKind, FixtureCargo, Metadata, absolute_member, repository_root, toml_string,
};
use super::{CONSUMER_PACKAGE, assert_absent_packages};
use anyhow::{Context, Result};
use camino::Utf8Path;
use std::fmt::Write as _;

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
