//! Contract tests: every consumer shape builds, tests, and keeps its edges.
//!
//! Three shapes are exercised against both sources. The *runtime* consumer is
//! the `ortho_l10n`-only root; the *facade* consumer imports only
//! `ortho_config`, and must keep working unchanged for applications that
//! already depend on it; the *mixed* consumer imports both, and is the only
//! shape that can show the two import paths name the same nominal types.
//!
//! Each test builds and runs the consumer's own tests. A root that resolved
//! but whose body no longer compiled would otherwise pass a metadata-only
//! check while proving nothing about the code a consumer writes.

use crate::boundary::consumer::{Dependency, DependencySource};
use crate::boundary::fixture_cargo::Metadata;
use crate::boundary::package_set::{
    ConsumerFixture, assert_packaged_set_complete, unpacked_directory,
};
use anyhow::{Context, Result, ensure};

/// Build and test a runtime consumer resolved from workspace path dependencies.
#[test]
fn workspace_runtime_consumer_builds_and_tests() -> Result<()> {
    let fixture = consumer_fixture(Dependency::Runtime, &DependencySource::Workspace)?;
    fixture.test()?;
    assert_runtime_only_edges(&fixture.metadata()?)?;
    Ok(())
}

/// Build and test a facade consumer resolved from workspace path dependencies.
///
/// The facade consumer is what existing applications already do. It must keep
/// resolving `ortho_config`, and it must reach the localizer through the
/// facade's re-export rather than through a direct runtime dependency.
#[test]
fn workspace_facade_consumer_builds_and_tests() -> Result<()> {
    let fixture = consumer_fixture(Dependency::Configuration, &DependencySource::Workspace)?;
    fixture.test()?;
    let metadata = fixture.metadata()?;
    ensure!(
        metadata.package("ortho_l10n").is_ok(),
        "the facade consumer did not resolve ortho_l10n through ortho_config",
    );
    Ok(())
}

/// Build and test a mixed consumer resolved from workspace path dependencies.
///
/// The mixed root is the only one that compiles a cross-assignment between the
/// two import paths. If the facade's re-exports drifted into wrappers the
/// consumer would fail to compile, so `test()` passing *is* the nominal
/// identity evidence.
#[test]
fn workspace_mixed_consumer_builds_and_tests() -> Result<()> {
    let fixture = consumer_fixture(Dependency::Mixed, &DependencySource::Workspace)?;
    fixture.test()?;
    assert_both_crates_resolve(&fixture.metadata()?)?;
    Ok(())
}

/// Build and test every shape against packaged sources.
///
/// Packaged sources are the stranger's view: version requirements patched at
/// the unpacked roots, with the packaged manifests' own edges. All three
/// shapes must still build, because a consumer reading a published crate gets
/// exactly this graph.
#[test]
fn every_packaged_consumer_shape_builds_and_tests() -> Result<()> {
    assert_packaged_set_complete()?;
    let source = DependencySource::Packaged {
        unpacked: unpacked_directory()?,
    };
    for dependency in Dependency::ALL {
        let shape = format!(
            "the {} consumer from {} sources",
            dependency.describe(),
            source.describe(),
        );
        let fixture = consumer_fixture(dependency, &source).with_context(|| shape.clone())?;
        fixture.test().with_context(|| shape.clone())?;
        assert_paths_for(dependency, &fixture.metadata()?).with_context(|| shape)?;
    }
    Ok(())
}

/// Render, write, and return a consumer fixture for `dependency`.
fn consumer_fixture(dependency: Dependency, source: &DependencySource) -> Result<ConsumerFixture> {
    let consumer = crate::boundary::consumer::Consumer::render(dependency, source)?;
    ConsumerFixture::new()?.write_consumer(&consumer)
}

/// The runtime-only consumer must have exactly the standalone edge set.
fn assert_runtime_only_edges(metadata: &Metadata) -> Result<()> {
    let edges = metadata.edges("boundary-consumer")?;
    ensure!(
        edges.iter().all(|edge| edge.package != "ortho_config"),
        "the runtime consumer resolved ortho_config; edges: {}",
        crate::boundary::fixture_cargo::describe_edges(&edges),
    );
    Ok(())
}

/// The mixed consumer must have resolved both crates.
fn assert_both_crates_resolve(metadata: &Metadata) -> Result<()> {
    for name in ["ortho_l10n", "ortho_config"] {
        ensure!(
            metadata.package(name).is_ok(),
            "the mixed consumer did not resolve {name}",
        );
    }
    Ok(())
}

/// A packaged consumer must resolve the crates its shape names, all by path.
///
/// Which crates that is depends on the shape: a runtime consumer reaches only
/// `ortho_l10n`, and demanding `ortho_config` of it would state the opposite of
/// the removal claim. Every crate that *does* resolve must come from an
/// unpacked root, so a patch that silently failed and fell back to the
/// workspace cannot pass.
fn assert_paths_for(dependency: Dependency, metadata: &Metadata) -> Result<()> {
    let required: &[&str] = match dependency {
        Dependency::Runtime => &["ortho_l10n"],
        Dependency::Configuration => &["ortho_config", "ortho_l10n"],
        Dependency::Mixed => &["ortho_l10n", "ortho_config"],
    };
    for name in required {
        metadata.package(name).with_context(|| {
            format!(
                "the packaged {} consumer did not resolve {name}",
                dependency.describe()
            )
        })?;
    }
    for name in ["ortho_l10n", "ortho_config", "ortho_config_macros"] {
        if let Ok(package) = metadata.package(name) {
            ensure!(
                package.source.is_none(),
                "the packaged {} consumer resolved {name} from {} rather than an unpacked root",
                dependency.describe(),
                package.source.as_deref().unwrap_or("an unnamed source"),
            );
        }
    }
    Ok(())
}
