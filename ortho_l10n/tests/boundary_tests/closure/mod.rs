//! Contract tests: a runtime-only consumer must not compile a facade.
//!
//! The removal claim from issue #566 is that `ortho_l10n` can be depended on
//! without configuration discovery, the derive macros, Figment, or Clap. The
//! tests here state it against a rendered foreign package root, both from
//! workspace path dependencies and from packaged sources, and back every
//! positive check with a negative control that must fail for its injected
//! edge.

use crate::boundary::closure::{
    CONSUMER_PACKAGE, ControlOutcome, FORBIDDEN_PACKAGES, Injection, assert_no_forbidden_packages,
    assert_root_edges, assert_runtime_normal_edges, run_negative_control,
};
use crate::boundary::consumer::{
    CONSUMER_PACKAGE as RENDERED_PACKAGE, Consumer, Dependency, DependencySource,
};
use crate::boundary::fixture_cargo::{
    FixtureCargo, Metadata, describe_edges, ensure_failure_names,
};
use crate::boundary::package_set::{ConsumerFixture, unpacked_directory};
use anyhow::{Context, Result, ensure};
use camino::Utf8Path;
use rstest::rstest;

/// Resolve a runtime consumer from path dependencies and assert its closure.
///
/// This is the shape an unpublished dependent sees: the sources in this
/// workspace, reached by path, with no packaged manifest in the graph. The
/// consumer's own body is built and tested too, so a closure that resolved
/// but did not compile would not pass.
#[test]
fn workspace_runtime_consumer_has_no_facade_closure() -> Result<()> {
    let consumer = Consumer::render(Dependency::Runtime, &DependencySource::Workspace)?;
    let fixture = ConsumerFixture::new()?.write_consumer(&consumer)?;
    let metadata = fixture.metadata()?;

    assert_no_forbidden_packages(&metadata)?;
    assert_runtime_normal_edges(&metadata)?;
    assert_root_edges(&metadata)?;
    fixture.test()?;
    Ok(())
}

/// Resolve a runtime consumer from packaged sources and assert its closure.
///
/// This is the shape a stranger sees: `ortho_l10n = "0.9.0"` resolved through
/// `[patch.crates-io]` at the unpacked package roots, so the edges come from
/// the packaged manifest rather than the workspace's. The absence of
/// `ortho_config` here is the point: a workspace path dependency can succeed
/// because this repository is already on the resolution path, and a packaged
/// one cannot.
#[test]
fn packaged_runtime_consumer_has_no_facade_closure() -> Result<()> {
    let source = DependencySource::Packaged {
        unpacked: unpacked_directory()?,
    };
    let consumer = Consumer::render(Dependency::Runtime, &source)?;
    let fixture = ConsumerFixture::new()?.write_consumer(&consumer)?;
    let metadata = fixture.metadata()?;

    assert_no_forbidden_packages(&metadata)?;
    assert_runtime_normal_edges(&metadata)?;
    assert_root_edges(&metadata)?;
    assert_patched_packages_come_from_paths(&metadata)?;
    fixture.test()?;
    Ok(())
}

/// The patched packages that did resolve must come from the unpacked roots.
///
/// Without this the packaged consumer could be reading the workspace after
/// all, and the closure it reported would describe the wrong graph. Cargo
/// reports `source: None` for a path package, which is what an unpacked
/// `.crate` root is; anything else means a registry package was resolved and
/// the patch did not take.
fn assert_patched_packages_come_from_paths(metadata: &Metadata) -> Result<()> {
    let runtime = metadata.package("ortho_l10n")?;
    ensure!(
        runtime.source.is_none(),
        "ortho_l10n resolved from {}, not from an unpacked package root",
        runtime.source.as_deref().unwrap_or("an unnamed source"),
    );
    for name in ["ortho_config", "ortho_config_macros"] {
        if let Ok(package) = metadata.package(name) {
            ensure!(
                package.source.is_none(),
                "{name} resolved from {}, not from an unpacked package root",
                package.source.as_deref().unwrap_or("an unnamed source"),
            );
        }
    }
    Ok(())
}

/// Every forbidden package must be absent by name, including Clap's splits.
///
/// A dedicated test rather than an assertion inside the two above, so a
/// regression names the package that reappeared instead of reporting a render
/// or build failure somewhere else.
#[test]
fn no_forbidden_package_appears_in_a_workspace_runtime_consumer() -> Result<()> {
    let consumer = Consumer::render(Dependency::Runtime, &DependencySource::Workspace)?;
    let metadata = ConsumerFixture::new()?
        .write_consumer(&consumer)?
        .metadata()?;
    let resolved = metadata.package_names();
    for forbidden in FORBIDDEN_PACKAGES {
        ensure!(
            !resolved.contains(forbidden),
            "{forbidden} resolved into a standalone runtime consumer; graph: {}",
            resolved.join(", "),
        );
    }
    Ok(())
}

/// Each negative control must fail, and fail *for its injected edge*.
///
/// One case per injection rather than a loop, so a failure names the injection
/// that produced it instead of stopping at the first one in enum order, and
/// nextest can schedule the four nested Cargo runs independently. Each case
/// spawns a `cargo metadata` of its own; each is deadline-bounded by the
/// fixture's own [`CARGO_DEADLINE`], and they share one fixture target
/// directory, so running them concurrently costs one warm resolve rather than
/// four cold ones.
///
/// Two outcomes are legitimate and both are accepted: Cargo may refuse the
/// injected crate during resolution, or resolve it and have the closure check
/// reject it by name. The control only counts as evidence when one of those
/// happened *and* the diagnostic names the injected crate; a fixture that
/// failed for an unrelated reason is an error here, not a pass. Both
/// conditions are asserted in one case because the second subsumes the first:
/// a diagnostic that names a crate is not empty.
#[rstest]
fn negative_controls_reject_every_injected_edge(
    #[values(
        Injection::NormalClap,
        Injection::BuildClap,
        Injection::DevClap,
        Injection::DevTestHelpers
    )]
    injection: Injection,
) -> Result<()> {
    let outcome = run_negative_control(injection).with_context(|| {
        format!(
            "the {} negative control did not behave",
            injection.describe()
        )
    })?;
    let diagnostic = diagnostic_of(&outcome);
    ensure!(
        diagnostic.contains(injection.injected_crate()),
        "the {} control failed without naming {}: {diagnostic}",
        injection.describe(),
        injection.injected_crate(),
    );
    Ok(())
}

/// The diagnostic either control outcome carried.
fn diagnostic_of(outcome: &ControlOutcome) -> &str {
    match outcome {
        ControlOutcome::Rejected { detail }
        | ControlOutcome::AcceptedButCaughtByName { detail } => detail,
    }
}

/// A fixture that cannot resolve at all must not read as a rejection.
///
/// The guard is what keeps the four controls above honest: if the fixture
/// root were malformed, every one of them would fail for that reason alone
/// and still exit non-zero. This manifest has no package name, so Cargo
/// rejects it before considering any dependency, and the failure must not
/// name the injected crate.
#[test]
fn an_unrelated_failure_does_not_satisfy_a_control() -> Result<()> {
    let fixture = FixtureCargo::new()?;
    fixture.write(Utf8Path::new("Cargo.toml"), "[package]\n")?;
    let output = fixture.cargo(&["metadata", "--format-version=1"])?;
    ensure!(
        ensure_failure_names(&output, "clap").is_err(),
        "a manifest Cargo cannot read was accepted as an injected-Clap rejection",
    );
    Ok(())
}

/// The rendered consumer must be the package the closure checks read.
///
/// A rename in the renderer and a rename in the closure checks would
/// otherwise have to agree by luck; this states the shared spelling once and
/// then proves the root resolves under it.
#[test]
fn the_rendered_consumer_is_the_package_the_closure_reads() -> Result<()> {
    ensure!(
        RENDERED_PACKAGE == CONSUMER_PACKAGE,
        "the renderer names the consumer {RENDERED_PACKAGE}, the closure reads {CONSUMER_PACKAGE}",
    );
    let consumer = Consumer::render(Dependency::Runtime, &DependencySource::Workspace)?;
    let metadata = ConsumerFixture::new()?
        .write_consumer(&consumer)?
        .metadata()?;
    let edges = metadata.edges(CONSUMER_PACKAGE)?;
    let described = describe_edges(&edges);
    ensure!(
        described.contains("ortho_l10n (normal)"),
        "the consumer's edges are {described}",
    );
    Ok(())
}
