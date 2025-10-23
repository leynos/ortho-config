//! Shared types and helpers for the CLI integration tests.

use anyhow::{anyhow, ensure, Result};
use ortho_config::OrthoConfig;
use serde::{Deserialize, Serialize};

pub(crate) trait OrthoResultExt<T> {
    fn to_anyhow(self) -> Result<T>;
}

impl<T> OrthoResultExt<T> for ortho_config::OrthoResult<T> {
    fn to_anyhow(self) -> Result<T> {
        self.map_err(|err| anyhow!(err))
    }
}

pub(crate) fn with_jail<T, F>(f: F) -> Result<T>
where
    F: FnOnce(&mut figment::Jail) -> Result<T>,
{
    figment::Jail::try_with(|j| f(j).map_err(|err| figment::Error::from(err.to_string())))
        .map_err(|err| anyhow!(err))
}

pub(crate) fn assert_config_values(
    config: &TestConfig,
    expected_sample: Option<&str>,
    expected_other: Option<&str>,
) -> Result<()> {
    ensure!(
        config.sample_value.as_deref() == expected_sample,
        "expected sample_value {:?}, got {:?}",
        expected_sample,
        config.sample_value
    );
    ensure!(
        config.other.as_deref() == expected_other,
        "expected other {:?}, got {:?}",
        expected_other,
        config.other
    );
    Ok(())
}

#[derive(Debug, Deserialize, Serialize, OrthoConfig)]
pub(crate) struct TestConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) sample_value: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) other: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, OrthoConfig)]
pub(crate) struct OptionConfig {
    pub(crate) maybe: Option<u32>,
}

#[derive(Debug, Deserialize, Serialize, OrthoConfig)]
pub(crate) struct RequiredConfig {
    pub(crate) sample_value: String,
}

#[derive(Debug, Deserialize, Serialize, OrthoConfig)]
pub(crate) struct ConflictConfig {
    pub(crate) second: Option<String>,
    pub(crate) sample: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, OrthoConfig)]
pub(crate) struct RenamedPathConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) sample: Option<String>,
    #[serde(skip)]
    #[ortho_config(cli_long = "config")]
    pub(crate) config_path: Option<std::path::PathBuf>,
}

pub(crate) use ortho_config::OrthoError;
