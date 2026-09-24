//! Compose-layer builder coverage for derive-generated helpers, plus direct
//! discovery-level coverage of `ConfigDiscovery::compose_layers`.

use anyhow::{Context as _, Result, ensure};
use cap_std::{ambient_authority, fs::Dir};
use ortho_config::{
    MapEnv, MergeLayer, MergeProvenance, OrthoConfig, SharedEnvSource, SharedScanEnvSource,
};
use rstest::rstest;
use serde::{Deserialize, Serialize};
use std::{path::Path, sync::Arc};

mod discovery_compose_layers {
    //! `ConfigDiscovery::compose_layers` pinned at its own API.
    //!
    //! The derive-generated route is covered end to end in `extends.rs`, which
    //! builds a three-file chain and asserts the merged result; these cases
    //! assert the *returned layer stack itself* — count, order, and paths — so
    //! a regression in chain composition is named at its source instead of
    //! surfacing as a distant merge mismatch, and so the error partition is
    //! pinned for the required, optional, invalid, and broken-chain outcomes.

    use std::path::{Path, PathBuf};
    use std::sync::Arc;

    use anyhow::{Context as _, Result, ensure};
    use cap_std::{ambient_authority, fs::Dir};
    use ortho_config::{ConfigDiscovery, MapEnv};

    /// Write a fixture through a capability handle, per the repository's
    /// filesystem policy: the handle names the directory it may touch.
    fn write_file(dir: &Path, name: &str, content: &str) -> Result<PathBuf> {
        let cap = Dir::open_ambient_dir(dir, ambient_authority())
            .context("open the temporary directory")?;
        cap.write(name, content.as_bytes())
            .context("write the fixture")?;
        Ok(dir.join(name))
    }

    /// Discovery reading nothing but the paths a test hands it.
    ///
    /// `XDG_CONFIG_DIRS` is pinned to a base that holds no configuration:
    /// with the variable absent (or separator-only), the Unix XDG rung falls
    /// back to `/etc/xdg`, and a host carrying `/etc/xdg/<name>/config.toml`
    /// would load it ahead of the fixtures. `discovery_telemetry.rs` applies
    /// the same guard.
    fn isolated_builder(name: &str) -> ortho_config::ConfigDiscoveryBuilder {
        ConfigDiscovery::builder(name)
            .clear_project_roots()
            .env_source(Arc::new(
                MapEnv::new().with_var("XDG_CONFIG_DIRS", "/nonexistent/ortho-config-test-xdg"),
            ))
    }

    fn layer_file_names(layers: &[ortho_config::MergeLayer<'static>]) -> Vec<String> {
        layers
            .iter()
            .filter_map(|layer| layer.path())
            .filter_map(|path| path.file_name())
            .map(str::to_owned)
            .collect()
    }

    #[test]
    fn an_extends_chain_returns_one_layer_per_file_base_first() -> Result<()> {
        let dir = tempfile::tempdir()?;
        write_file(dir.path(), "parent.toml", "retries = 2\n")?;
        let child = write_file(
            dir.path(),
            "app.toml",
            "extends = \"parent.toml\"\nretries = 3\n",
        )?;

        let outcome = isolated_builder("demo")
            .add_required_path(&child)
            .build()
            .compose_layers();

        ensure!(
            outcome.required_errors.is_empty() && outcome.optional_errors.is_empty(),
            "chain should compose without errors: {:?} / {:?}",
            outcome.required_errors,
            outcome.optional_errors,
        );
        let names = layer_file_names(&outcome.value);
        ensure!(
            names == ["parent.toml", "app.toml"],
            "layers should be one per file, base first: {names:?}",
        );
        Ok(())
    }

    #[test]
    fn a_missing_required_candidate_partitions_as_required() {
        let outcome = isolated_builder("demo")
            .add_required_path(Path::new("/nonexistent/required.toml"))
            .build()
            .compose_layers();

        assert!(outcome.value.is_empty(), "no layers should be returned");
        assert_eq!(outcome.required_errors.len(), 1);
        assert!(outcome.optional_errors.is_empty());
    }

    #[test]
    fn a_missing_optional_candidate_is_not_an_error() {
        let outcome = isolated_builder("demo")
            .add_explicit_path(Path::new("/nonexistent/optional.toml"))
            .build()
            .compose_layers();

        assert!(outcome.value.is_empty());
        assert!(outcome.required_errors.is_empty());
        assert!(outcome.optional_errors.is_empty());
    }

    #[test]
    fn an_unparsable_optional_candidate_partitions_as_optional() -> Result<()> {
        let dir = tempfile::tempdir()?;
        let broken = write_file(dir.path(), "broken.toml", "this is not toml = = =\n")?;

        let outcome = isolated_builder("demo")
            .add_explicit_path(&broken)
            .build()
            .compose_layers();

        ensure!(outcome.value.is_empty(), "no layers should be returned");
        ensure!(outcome.required_errors.is_empty());
        ensure!(outcome.optional_errors.len() == 1);
        Ok(())
    }

    #[test]
    fn a_broken_extends_chain_partitions_as_required() -> Result<()> {
        let dir = tempfile::tempdir()?;
        let child = write_file(dir.path(), "app.toml", "extends = \"absent-parent.toml\"\n")?;

        let outcome = isolated_builder("demo")
            .add_required_path(&child)
            .build()
            .compose_layers();

        ensure!(outcome.value.is_empty(), "no layers should be returned");
        ensure!(
            outcome.required_errors.len() == 1,
            "the broken chain should be a required failure: {:?}",
            outcome.required_errors,
        );
        Ok(())
    }
}

#[derive(Debug, Deserialize, Serialize, OrthoConfig)]
#[ortho_config(prefix = "APP_")]
struct BuilderConfig {
    #[ortho_config(default = 7)]
    port: u16,
}

/// Write one selected configuration file through a directory capability.
///
/// The selector is explicit so the assertions do not depend on the test
/// process's current working directory or home configuration.
fn selected_file(root: &Path, contents: &str) -> Result<std::path::PathBuf> {
    let directory = Dir::open_ambient_dir(root, ambient_authority())
        .context("open composition fixture root")?;
    directory
        .write("selected.toml", contents.as_bytes())
        .context("write composition fixture")?;
    Ok(root.join("selected.toml"))
}

#[rstest]
fn compose_layers_collects_cli_env_and_file() -> Result<()> {
    let fixture = tempfile::tempdir().context("create composition fixture root")?;
    let file = selected_file(fixture.path(), "port = 2020")?;
    let discovery: SharedEnvSource = Arc::new(MapEnv::new().with_var("APP_CONFIG_PATH", &file));
    let merge: SharedScanEnvSource = Arc::new(MapEnv::new().with_var("APP_PORT", "3030"));
    let composition = BuilderConfig::compose_layers_from_iter_with_sources(
        ["prog", "--port", "4040"],
        discovery,
        merge,
    );
    let (layers, errors) = composition.into_parts();
    ensure!(
        errors.is_empty(),
        "expected composition without errors: {errors:?}"
    );
    let provenances: Vec<MergeProvenance> = layers.iter().map(MergeLayer::provenance).collect();
    ensure!(
        provenances
            == [
                MergeProvenance::Defaults,
                MergeProvenance::File,
                MergeProvenance::Environment,
                MergeProvenance::Cli,
            ],
        "unexpected provenance ordering: {provenances:?}"
    );
    let merged =
        BuilderConfig::merge_from_layers(layers.clone()).map_err(|error| anyhow::anyhow!(error))?;
    ensure!(merged.port == 4040, "CLI override should win");
    let file_layer = layers
        .iter()
        .find(|layer| layer.provenance() == MergeProvenance::File)
        .and_then(|layer| layer.path())
        .and_then(|path| path.file_name())
        .map(str::to_owned);
    ensure!(
        file_layer.as_deref() == Some("selected.toml"),
        "unexpected file layer: {file_layer:?}"
    );
    Ok(())
}

#[rstest]
fn compose_layers_collects_cli_parse_errors() -> Result<()> {
    let fixture = tempfile::tempdir().context("create invalid CLI fixture root")?;
    let file = selected_file(fixture.path(), "# empty selected configuration\n")?;
    let discovery: SharedEnvSource = Arc::new(MapEnv::new().with_var("APP_CONFIG_PATH", &file));
    let merge: SharedScanEnvSource = Arc::new(MapEnv::new());
    let composition = BuilderConfig::compose_layers_from_iter_with_sources(
        ["prog", "--port", "not-a-number"],
        discovery,
        merge,
    );
    let (_layers, errors) = composition.into_parts();
    ensure!(
        !errors.is_empty(),
        "expected CLI parsing error during composition"
    );
    ensure!(errors.len() == 1, "expected a single CLI error: {errors:?}");
    Ok(())
}

#[rstest]
fn compose_layers_collects_env_and_file_errors() -> Result<()> {
    let fixture = tempfile::tempdir().context("create invalid layers fixture root")?;
    let file = selected_file(fixture.path(), "port = \"file-not-a-number\"")?;
    let discovery: SharedEnvSource = Arc::new(MapEnv::new().with_var("APP_CONFIG_PATH", &file));
    let merge: SharedScanEnvSource =
        Arc::new(MapEnv::new().with_var("APP_PORT", "env-not-a-number"));
    let composition =
        BuilderConfig::compose_layers_from_iter_with_sources(["prog"], discovery, merge);
    let (layers, errors) = composition.into_parts();
    let merged = BuilderConfig::merge_from_layers(layers.clone());
    ensure!(merged.is_err(), "expected malformed layer merge to fail");
    let aggregated = ortho_config::declarative::LayerComposition::new(layers, errors)
        .into_merge_result(BuilderConfig::merge_from_layers);
    ensure!(
        aggregated.is_err(),
        "expected aggregated malformed merge to fail"
    );
    Ok(())
}
