//! Tests for the append merge strategy on vectors.
use anyhow::{Result, anyhow, ensure};
use ortho_config::OrthoConfig;
use rstest::rstest;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize, OrthoConfig)]
struct VecConfig {
    #[ortho_config(merge_strategy = "append")]
    values: Vec<String>,
}

#[derive(Debug, Deserialize, Serialize, OrthoConfig)]
struct DefaultVec {
    #[ortho_config(default = vec!["def".to_owned()], merge_strategy = "append")]
    values: Vec<String>,
}

#[derive(Debug, Deserialize, Serialize, OrthoConfig)]
struct EmptyVec {
    #[ortho_config(default = vec![], merge_strategy = "append")]
    values: Vec<String>,
}

fn with_jail<F>(f: F) -> Result<()>
where
    F: FnOnce(&mut figment::Jail) -> Result<()>,
{
    figment::Jail::try_with(|j| f(j).map_err(|err| figment::Error::from(err.to_string())))
        .map_err(|err| anyhow!(err))
}

#[rstest]
fn append_merges_all_sources() -> Result<()> {
    with_jail(|j| {
        j.create_file(".config.toml", "values = [\"file\"]")?;
        j.set_env("VALUES", "[\"env\"]");
        let cfg = VecConfig::load_from_iter(["prog", "--values", "cli1", "--values", "cli2"])
            .map_err(|err| anyhow!(err))?;
        let expected = vec!["file", "env", "cli1", "cli2"]
            .into_iter()
            .map(String::from)
            .collect::<Vec<_>>();
        ensure!(
            cfg.values == expected,
            "expected {:?}, got {:?}",
            expected,
            cfg.values
        );
        Ok(())
    })?;
    Ok(())
}

#[rstest]
fn append_empty_sources_yields_empty() -> Result<()> {
    with_jail(|_| {
        let cfg = EmptyVec::load_from_iter(["prog"]).map_err(|err| anyhow!(err))?;
        ensure!(
            cfg.values.is_empty(),
            "expected empty values, got {:?}",
            cfg.values
        );
        Ok(())
    })?;
    Ok(())
}

#[rstest]
fn append_includes_defaults() -> Result<()> {
    with_jail(|j| {
        j.create_file(".config.toml", "values = [\"file\"]")?;
        j.set_env("VALUES", "[\"env\"]");
        let cfg =
            DefaultVec::load_from_iter(["prog", "--values", "cli"]).map_err(|err| anyhow!(err))?;
        let expected = vec!["def", "file", "env", "cli"]
            .into_iter()
            .map(String::from)
            .collect::<Vec<_>>();
        ensure!(
            cfg.values == expected,
            "expected {:?}, got {:?}",
            expected,
            cfg.values
        );
        Ok(())
    })?;
    Ok(())
}
