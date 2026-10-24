//! Executable contracts for the installation and upgrade examples in the
//! public documentation.
//!
//! Split from `documentation_examples_tests.rs` so each documentation test
//! binary owns one concern and stays within the repository's 400-line cap. The
//! shared loader lives in the `documentation_examples` directory module, which
//! every documentation test binary declares independently.

mod documentation_examples;

use anyhow::{Context, Result, ensure};
use cap_std::{ambient_authority, fs::Dir};
use documentation_examples::{DocumentedExample, documented_example, load_documented_examples};
use ortho_config::toml;
use rstest::rstest;

/// The release the README and user's guide install.
///
/// Derived from the crate version so the contracts track a release bump:
/// `lading bump` rewrites both the manifests and the documentation globs in
/// `lading.toml` together, and a document left behind fails here.
const DOCUMENTED_RELEASE: &str = env!("CARGO_PKG_VERSION");

/// Documents whose manifests describe a fixed upgrade path rather than the
/// current release, so the release sweep must not rewrite their versions.
const MIGRATION_GUIDES: &[&str] = &["docs/v0-10-0-migration-guide.md"];

/// Every documented manifest that depends on `ortho_config` installs the release.
///
/// Sweeping the whole registry, rather than naming each fence, means a new
/// installation example is covered the moment it is published.
#[test]
fn every_documented_manifest_installs_the_current_release() -> Result<()> {
    let mut checked = Vec::new();
    let release_manifests = load_documented_examples()?
        .iter()
        .filter(|example| is_release_manifest(example));
    for example in release_manifests {
        let manifest = parse_manifest(example)?;
        for version in ortho_config_versions(&manifest) {
            ensure!(
                version == Some(DOCUMENTED_RELEASE),
                "{} should install ortho_config {DOCUMENTED_RELEASE}, got {version:?}",
                example.id
            );
            checked.push(example.id.as_str());
        }
    }
    ensure!(
        checked.len() >= 5,
        "expected installation manifests across the README and user's guide, found {checked:?}"
    );
    Ok(())
}

/// Each install manifest also selects the companion crates and features its
/// section promises.
#[rstest]
#[case::readme("readme-install", "serde", None)]
#[case::guide_clap("guide-install", "clap", None)]
#[case::guide_serde("guide-install", "serde", None)]
#[case::yaml("guide-yaml-install", "ortho_config", Some("yaml"))]
#[case::metrics("guide-metrics-install", "ortho_config", Some("metrics"))]
#[case::tracing("guide-tracing-install", "tracing-subscriber", Some("env-filter"))]
fn installation_manifest_declares_its_dependency(
    #[case] id: &str,
    #[case] name: &str,
    #[case] expected_feature: Option<&str>,
) -> Result<()> {
    let manifest = parse_manifest(documented_example(id)?)?;
    let dependency = manifest
        .get("dependencies")
        .and_then(|dependencies| dependencies.get(name))
        .with_context(|| format!("{id} should declare {name}"))?;
    if let Some(feature) = expected_feature {
        ensure!(
            dependency
                .get("features")
                .and_then(toml::Value::as_array)
                .is_some_and(|features| features
                    .iter()
                    .any(|value| value.as_str() == Some(feature))),
            "{id} should enable the {feature} feature of {name}"
        );
    }
    Ok(())
}

/// The aliased manifest must rename the real package, or the derive's
/// `crate = "config_layer"` path would point at nothing.
#[test]
fn aliased_manifest_renames_the_ortho_config_package() -> Result<()> {
    let manifest = parse_manifest(documented_example("guide-alias-install")?)?;
    let alias = manifest
        .get("dependencies")
        .and_then(|dependencies| dependencies.get("config_layer"))
        .context("the alias manifest should declare config_layer")?;
    ensure!(alias.get("package").and_then(toml::Value::as_str) == Some("ortho_config"));
    Ok(())
}

/// The upgrade example differs only in the version it selects, so a reader
/// can apply it without changing features by accident.
#[test]
fn migration_manifests_upgrade_only_the_version() -> Result<()> {
    let before = parse_manifest(documented_example("migration-dependencies-before")?)?;
    let after = parse_manifest(documented_example("migration-dependencies-after")?)?;
    let before_dependency = ortho_config_dependency(&before)?;
    let after_dependency = ortho_config_dependency(&after)?;
    ensure!(dependency_version(before_dependency) == Some("0.9.0"));
    ensure!(dependency_version(after_dependency) == Some("0.10.0"));
    ensure!(before_dependency.get("features") == after_dependency.get("features"));
    Ok(())
}

/// The standalone runtime is adopted at the release the guide upgrades to.
#[test]
fn migration_runtime_manifest_selects_the_upgrade_release() -> Result<()> {
    let manifest = parse_manifest(documented_example("migration-l10n-dependency")?)?;
    let runtime = manifest
        .get("dependencies")
        .and_then(|dependencies| dependencies.get("ortho_l10n"))
        .context("the runtime manifest should declare ortho_l10n")?;
    ensure!(dependency_version(runtime) == Some("0.10.0"));
    Ok(())
}

#[test]
fn documented_binstall_command_matches_the_packaged_crate() -> Result<()> {
    let example = documented_example("readme-binstall")?;
    ensure!(example.language == "console");
    ensure!(
        example.body == "cargo binstall --disable-strategies compile cargo-orthohelp\n",
        "documented cargo-binstall command drifted"
    );

    // The command only avoids a source build if the named crate publishes the
    // binstall metadata that resolves its release archives.
    let repository_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let repository = Dir::open_ambient_dir(repository_root, ambient_authority())
        .context("open the repository root")?;
    let manifest: toml::Value = toml::from_str(
        &repository
            .read_to_string("cargo-orthohelp/Cargo.toml")
            .context("read the cargo-orthohelp manifest")?,
    )
    .context("parse the cargo-orthohelp manifest")?;
    let package = manifest
        .get("package")
        .context("cargo-orthohelp should declare [package]")?;
    ensure!(package.get("name").and_then(toml::Value::as_str) == Some("cargo-orthohelp"));
    let binstall = package
        .get("metadata")
        .and_then(|metadata| metadata.get("binstall"))
        .context("cargo-orthohelp should declare [package.metadata.binstall]")?;
    ensure!(binstall.get("pkg-fmt").and_then(toml::Value::as_str) == Some("tgz"));
    for key in ["pkg-url", "bin-dir"] {
        ensure!(
            binstall.get(key).and_then(toml::Value::as_str).is_some(),
            "binstall metadata should declare {key}"
        );
    }
    Ok(())
}

fn parse_manifest(example: &DocumentedExample) -> Result<toml::Value> {
    ensure!(example.language == "toml", "{} should be TOML", example.id);
    toml::from_str(&example.body).with_context(|| format!("parse {}", example.id))
}

/// Whether the release sweep covers this example: a TOML manifest outside the
/// migration guides, whose upgrade paths pin historical versions on purpose.
fn is_release_manifest(example: &DocumentedExample) -> bool {
    example.language == "toml" && !MIGRATION_GUIDES.contains(&example.source)
}

/// The version requirement of every `ortho_config` dependency in a manifest,
/// whether named directly or through a `package = ...` alias.
fn ortho_config_versions(manifest: &toml::Value) -> Vec<Option<&str>> {
    manifest
        .get("dependencies")
        .and_then(toml::Value::as_table)
        .into_iter()
        .flatten()
        .filter(|(name, dependency)| names_ortho_config(name, dependency))
        .map(|(_, dependency)| dependency_version(dependency))
        .collect()
}

fn ortho_config_dependency(manifest: &toml::Value) -> Result<&toml::Value> {
    manifest
        .get("dependencies")
        .and_then(|dependencies| dependencies.get("ortho_config"))
        .context("manifest should declare ortho_config")
}

/// Whether a dependency entry resolves to the `ortho_config` package, either
/// under its own name or through a Cargo `package = ...` alias.
fn names_ortho_config(name: &str, dependency: &toml::Value) -> bool {
    name == "ortho_config"
        || dependency.get("package").and_then(toml::Value::as_str) == Some("ortho_config")
}

/// Read a dependency's version from either the string or the table form.
fn dependency_version(dependency: &toml::Value) -> Option<&str> {
    dependency
        .as_str()
        .or_else(|| dependency.get("version").and_then(toml::Value::as_str))
}
