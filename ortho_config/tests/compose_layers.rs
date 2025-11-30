//! Compose-layer builder coverage for derive-generated helpers.

use anyhow::Result;
use ortho_config::{MergeLayer, MergeProvenance, OrthoConfig, ResultIntoFigment};
use rstest::rstest;
use serde::{Deserialize, Serialize};

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
