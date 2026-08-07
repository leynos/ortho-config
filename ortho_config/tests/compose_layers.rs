//! Compose-layer builder coverage for derive-generated helpers, plus direct
//! discovery-level coverage of `ConfigDiscovery::compose_layers`.

use anyhow::Result;
use ortho_config::{MergeLayer, MergeProvenance, OrthoConfig, ResultIntoFigment};
use rstest::rstest;
use serde::{Deserialize, Serialize};

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

#[rstest]
fn compose_layers_collects_cli_env_and_file() -> Result<()> {
    figment::Jail::try_with(|jail| {
        jail.clear_env();
        jail.set_env("APP_PORT", "3030");
        jail.create_file(".app.toml", "port = 2020")?;

        let composition = BuilderConfig::compose_layers_from_iter(["prog", "--port", "4040"]);
        let (layers, errors) = composition.into_parts();

        if !errors.is_empty() {
            return Err(figment::Error::from("expected composition without errors"));
        }
        let provenances: Vec<MergeProvenance> = layers.iter().map(MergeLayer::provenance).collect();
        let expected = vec![
            MergeProvenance::Defaults,
            MergeProvenance::File,
            MergeProvenance::Environment,
            MergeProvenance::Cli,
        ];
        if provenances != expected {
            return Err(figment::Error::from("unexpected provenance ordering"));
        }

        let merged = BuilderConfig::merge_from_layers(layers.clone()).to_figment()?;
        if merged.port != 4040 {
            return Err(figment::Error::from("CLI override should win"));
        }

        let file_layer = layers
            .iter()
            .find(|layer| layer.provenance() == MergeProvenance::File)
            .and_then(|layer| layer.path())
            .and_then(|path| path.file_name())
            .map(str::to_owned);
        if file_layer.as_deref() != Some(".app.toml") {
            return Err(figment::Error::from("unexpected file layer"));
        }
        Ok(())
    })?;
    Ok(())
}

#[rstest]
#[expect(
    clippy::panic_in_result_fn,
    reason = "Assertions give clearer intent for this negative path"
)]
fn compose_layers_collects_cli_parse_errors() -> Result<()> {
    figment::Jail::try_with(|jail| {
        jail.clear_env();
        let composition =
            BuilderConfig::compose_layers_from_iter(["prog", "--port", "not-a-number"]);
        let (_layers, errors) = composition.into_parts();
        assert!(
            !errors.is_empty(),
            "expected CLI parsing error to be captured during composition"
        );
        assert_eq!(errors.len(), 1, "expected a single CLI error");
        Ok(())
    })?;
    Ok(())
}

#[rstest]
#[expect(
    clippy::panic_in_result_fn,
    reason = "Assertions give clearer intent for this negative path"
)]
fn compose_layers_collects_env_and_file_errors() -> Result<()> {
    figment::Jail::try_with(|jail| {
        jail.clear_env();
        jail.set_env("APP_PORT", "env-not-a-number");
        jail.create_file(".app.toml", r#"port = "file-not-a-number""#)?;

        let composition = BuilderConfig::compose_layers_from_iter(["prog"]);
        let (layers, errors) = composition.into_parts();

        let merged = BuilderConfig::merge_from_layers(layers.clone());
        assert!(
            merged.is_err(),
            "expected merge_from_layers to fail with malformed layers"
        );
        let aggregated = ortho_config::declarative::LayerComposition::new(layers, errors)
            .into_merge_result(BuilderConfig::merge_from_layers);
        assert!(
            aggregated.is_err(),
            "expected aggregated merge to fail with malformed values"
        );
        Ok(())
    })?;
    Ok(())
}
