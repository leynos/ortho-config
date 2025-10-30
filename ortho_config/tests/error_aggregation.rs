//! Tests for aggregated error reporting across configuration sources.
use anyhow::{Result, anyhow, ensure};
use ortho_config::{OrthoConfig, OrthoError};
use rstest::rstest;
use serde::Deserialize;

#[path = "test_utils.rs"]
mod test_utils;
use test_utils::with_jail;

#[derive(Debug, Deserialize, OrthoConfig)]
struct AggConfig {
    #[expect(
        dead_code,
        reason = "Field is read via deserialization only in this test"
    )]
    port: u32,
}

#[derive(Debug, Deserialize, OrthoConfig)]
#[ortho_config(
    prefix = "AGG_",
    discovery(
        app_name = "agg_config",
        env_var = "AGG_CONFIG_PATH",
        dotfile_name = ".agg.toml"
    )
)]
struct DiscoveryErrorConfig {
    #[ortho_config(default = 0)]
    port: u32,
}

#[rstest]
fn aggregates_cli_file_env_errors() -> Result<()> {
    with_jail(|j| {
        j.create_file(".config.toml", "port = ")?; // invalid TOML
        j.set_env("PORT", "notanumber");
        let err = match AggConfig::load_from_iter(["prog", "--bogus"]) {
            Ok(cfg) => return Err(anyhow!("expected aggregated error, got config {cfg:?}")),
            Err(err) => err,
        };
        let agg = match &*err {
            OrthoError::Aggregate(agg) => agg,
            other => return Err(anyhow!("unexpected error variant: {other:?}")),
        };
        let actual = agg.len();
        ensure!(
            actual == 3,
            "expected three aggregated errors, got {actual}"
        );
        let mut kinds = agg
            .iter()
            .map(|e| match e {
                OrthoError::CliParsing(_) => Ok(1),
                OrthoError::File { .. } => Ok(2),
                OrthoError::Merge { .. } | OrthoError::Gathering(_) => Ok(3),
                other => Err(anyhow!("unexpected aggregated error variant: {other:?}")),
            })
            .collect::<Result<Vec<_>>>()?;
        kinds.sort_unstable();
        ensure!(kinds == vec![1, 2, 3], "unexpected error kinds: {kinds:?}");
        Ok(())
    })?;
    Ok(())
}

#[rstest]
fn discovery_errors_hidden_when_fallback_succeeds() -> Result<()> {
    with_jail(|j| {
        j.create_file("invalid.toml", "port = ???")?;
        j.create_file(".agg.toml", "port = 7000")?;
        j.set_env("AGG_CONFIG_PATH", "invalid.toml");

        let cfg = DiscoveryErrorConfig::load_from_iter(["prog"]).map_err(|err| anyhow!(err))?;
        let actual = cfg.port;
        ensure!(actual == 7000, "expected port 7000, got {actual}");
        Ok(())
    })?;
    Ok(())
}

#[rstest]
fn required_path_errors_surface_even_with_fallback() -> Result<()> {
    with_jail(|j| {
        j.create_file(".agg.toml", "port = 7000")?;

        let err =
            match DiscoveryErrorConfig::load_from_iter(["prog", "--config-path", "missing.toml"]) {
                Ok(cfg) => {
                    return Err(anyhow!("expected missing config path error, got {cfg:?}"));
                }
                Err(err) => err,
            };
        ensure!(
            matches!(&*err, OrthoError::File { .. }),
            "expected file error, got {err:?}"
        );
        Ok(())
    })?;
    Ok(())
}

#[rstest]
fn discovery_errors_surface_when_all_candidates_fail() -> Result<()> {
    with_jail(|j| {
        j.create_file("invalid.toml", "port = ???")?;
        j.set_env("AGG_CONFIG_PATH", "invalid.toml");

        let err = match DiscoveryErrorConfig::load_from_iter(["prog"]) {
            Ok(cfg) => return Err(anyhow!("expected discovery error, got {cfg:?}")),
            Err(err) => err,
        };
        ensure!(
            matches!(&*err, OrthoError::File { .. }),
            "expected file error, got {err:?}"
        );
        Ok(())
    })?;
    Ok(())
}
