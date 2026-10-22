//! Contract tests: a runtime-only consumer must not compile a facade.
//!
//! The removal claim from issue #566 is that `ortho_l10n` can be depended on
//! without configuration discovery, the derive macros, Figment, or Clap. The
//! tests here state it against a rendered foreign package root, both from
//! workspace path dependencies and from packaged sources, and back every
//! positive check with a negative control that must fail for its injected
//! edge.

use crate::boundary::closure::{
    CONSUMER_PACKAGE, FORBIDDEN_PACKAGES, Injection, assert_no_forbidden_packages,
    assert_no_forbidden_reachable, assert_root_edges, assert_runtime_normal_edges,
    run_negative_control,
};
use crate::boundary::consumer::{
    CONSUMER_PACKAGE as RENDERED_PACKAGE, Consumer, Dependency, DependencySource,
};
use crate::boundary::fixture_cargo::environment::CARGO_DEADLINE;
use crate::boundary::fixture_cargo::{
    FixtureCargo, Metadata, cargo_binary, describe_edges, inherited_toolchain, repository_root,
    run_with_deadline,
};
use crate::boundary::package_set::{ConsumerFixture, unpacked_directory};
use anyhow::{Context, Result, ensure};
use camino::Utf8Path;
use rstest::rstest;
use std::process::Command;

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
/// [`run_negative_control`] does the whole assertion: it requires Cargo to
/// *resolve* the injected edge and then requires the closure check to reject
/// it by name. A control that only inspected Cargo's exit status would accept
/// an offline cache miss as a rejection, since that diagnostic names the crate
/// too.
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
    let diagnostic = run_negative_control(injection).with_context(|| {
        format!(
            "the {} negative control did not behave",
            injection.describe()
        )
    })?;
    ensure!(
        diagnostic.contains(injection.injected_crate()),
        "the {} control was rejected without naming {}: {diagnostic}",
        injection.describe(),
        injection.injected_crate(),
    );
    Ok(())
}

/// A fixture that cannot resolve at all must not read as a rejection.
///
/// This is the guard that keeps the four controls above honest, and it is
/// stated against the path those controls actually take rather than against a
/// helper.
///
/// [`run_negative_control`] separates the two outcomes at Cargo's exit status:
/// a non-zero exit becomes its `Refused` failure, while a successful run is
/// parsed as a metadata document and then checked. A refused resolve is
/// therefore already an error before any closure check runs, and "non-zero exit
/// that mentions the crate" is never accepted as evidence.
///
/// What the test below states is the premise that separation rests on: a
/// manifest Cargo rejects produces no parseable document, so the two outcomes
/// really are distinguishable rather than both surfacing as a graph the closure
/// check would read as clean.
///
/// The manifest has no package name, so Cargo rejects it before considering any
/// dependency, and its diagnostic must not be mistaken for the closure check
/// naming the injected crate.
#[test]
fn an_unrelated_failure_does_not_satisfy_a_control() -> Result<()> {
    let fixture = FixtureCargo::new()?;
    fixture.write(Utf8Path::new("Cargo.toml"), "[package]\n")?;
    let output = fixture.cargo(&["metadata", "--format-version=1"])?;
    ensure!(
        !output.status.success(),
        "a manifest with no package name was accepted by Cargo",
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    ensure!(
        Metadata::parse(&stdout).is_err(),
        "a refused resolve still produced a metadata document, so a control \
         could read the refusal as evidence",
    );
    Ok(())
}

/// The runtime's own workspace closure must reach no forbidden package.
///
/// # Why this is not the same check as the consumer one
///
/// The consumer contracts above measure a *rendered stranger's* root, whose
/// only normal edge is the runtime. They therefore see whatever the runtime
/// declares, and nothing else. The crate under test is not that root: in
/// `make test` the workspace resolves `ortho_l10n` together with the facade,
/// the macros, and the test helpers, and this asks whether *that* graph pulls a
/// forbidden crate into the runtime's closure.
///
/// It matters because the two graphs are not nested. The workspace graph
/// includes `ortho_l10n`'s dev-dependencies - `proptest`, `rstest`, and the
/// shared fixtures a test may consult - and ADR-0015 extends the boundary to
/// "its focused test graph" explicitly. A dev-dependency of a dev-dependency
/// is reachable here and invisible to a consumer contract that stops at the
/// runtime's normal edges.
///
/// The workspace resolve is a *superset* of a focused one: Cargo offers no
/// `cargo metadata -p`, and the full graph contains every edge a focused
/// resolve would contain, so absence here entails absence there. That is the
/// reason this test needs no second Cargo invocation.
#[test]
fn the_runtime_workspace_closure_reaches_no_forbidden_package() -> Result<()> {
    assert_no_forbidden_reachable(&workspace_metadata()?, "ortho_l10n")
}

/// The check above must be able to fail, and fail naming the crate.
///
/// A closure assertion that can only ever pass is not evidence, and the risk
/// here is not hypothetical: `reachable` could return the root alone, and
/// [`assert_no_forbidden_reachable`] would then report an empty offending list
/// for every graph it is handed.
///
/// The positive control is the *facade's* closure in the same resolve, which
/// does contain all eight forbidden names on this host when its dev
/// dependencies are followed. So the two calls differ only in the root, and
/// only the root decides the outcome.
#[test]
fn the_closure_check_rejects_the_facade_in_the_same_graph() -> Result<()> {
    let metadata = workspace_metadata()?;
    assert_no_forbidden_reachable(&metadata, "ortho_l10n")?;
    let rejection = assert_no_forbidden_reachable(&metadata, "ortho_config").err();
    let reported = rejection.map(|error| error.to_string()).with_context(|| {
        concat!(
            "ortho_config's closure resolved no forbidden package in the workspace graph, ",
            "so the runtime's clean report proves nothing about that check"
        )
    })?;
    for forbidden in FORBIDDEN_PACKAGES {
        ensure!(
            reported.contains(forbidden),
            "the facade's closure was rejected without naming {forbidden}: {reported}",
        );
    }
    Ok(())
}

/// Resolve this workspace as the host sees it, narrowed to the host platform.
///
/// The narrowing is the same one [`FixtureCargo::resolve`] applies, for the
/// same reason: an unfiltered resolve must download every platform's
/// dependencies under `--offline`, and this repository's own Windows leg failed
/// on exactly that. Here it is applied through the *workspace* manifest rather
/// than a rendered fixture root, so the child is the repository itself and no
/// private target directory is needed.
///
/// Every edge kind is resolved. Dropping dev edges would remove the very part
/// of the graph these two tests exist to examine.
///
/// # Errors
///
/// Returns an error when the workspace is not a crate root, when Cargo cannot
/// be spawned or exceeds [`CARGO_DEADLINE`], or when its output is not the
/// single JSON document the contracts expect.
fn workspace_metadata() -> Result<Metadata> {
    let repository = repository_root()?;
    let mut command = Command::new(cargo_binary());
    command
        .current_dir(&repository)
        .args([
            "metadata",
            "--format-version=1",
            "--all-features",
            "--filter-platform",
            "host-tuple",
            "--offline",
            "--manifest-path",
        ])
        .arg(repository.join("Cargo.toml").as_std_path())
        .env_clear()
        .envs(inherited_toolchain());
    let output = run_with_deadline(command, CARGO_DEADLINE, "workspace cargo metadata")?;
    ensure!(
        output.status.success(),
        "workspace cargo metadata failed with {}\nstderr:\n{}",
        output.status,
        String::from_utf8_lossy(&output.stderr),
    );
    Metadata::parse(&String::from_utf8_lossy(&output.stdout))
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
