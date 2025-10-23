//! Shared types and helpers for the CLI integration tests.

use anyhow::{anyhow, Result};
use ortho_config::{OrthoConfig, OrthoError, OrthoResult};
use serde::{Deserialize, Serialize};
use std::fmt;

#[path = "../test_utils.rs"]
mod test_utils;
use test_utils::with_jail;

#[path = "../clap_test_utils.rs"]
mod clap_test_utils;
pub(crate) use clap_test_utils::assert_config_values;
use clap_test_utils::ConfigValueAssertions;

pub(crate) trait OrthoResultExt<T> {
    fn to_anyhow(self) -> Result<T>;
}

impl<T> OrthoResultExt<T> for ortho_config::OrthoResult<T> {
    fn to_anyhow(self) -> Result<T> {
        self.map_err(|err| anyhow!(err))
    }
}

fn default_recipient() -> String { String::from("World") }

fn default_salutations() -> Vec<String> { vec![String::from("Hello")] }

#[derive(Debug, Deserialize, Serialize, OrthoConfig)]
pub(crate) struct TestConfig {
    #[ortho_config(default = default_recipient())]
    pub(crate) recipient: String,
    #[serde(default = "default_salutations")]
    #[ortho_config(default = default_salutations())]
    pub(crate) salutations: Vec<String>,
    #[serde(default)]
    #[ortho_config(default = false)]
    pub(crate) is_excited: bool,
    #[serde(default)]
    #[ortho_config(default = false)]
    pub(crate) is_quiet: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) sample_value: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) other: Option<String>,
}

impl Default for TestConfig {
    fn default() -> Self {
        Self {
            recipient: default_recipient(),
            salutations: default_salutations(),
            is_excited: false,
            is_quiet: false,
            sample_value: None,
            other: None,
        }
    }
}

impl ConfigValueAssertions for TestConfig {
    fn assert_values(
        &self,
        expected_sample: Option<&'static str>,
        expected_other: Option<&'static str>,
    ) -> Result<()> {
        let expected = ExpectedConfig {
            sample_value: expected_sample,
            other: expected_other,
            ..ExpectedConfig::default()
        };
        assert_config_eq(self, &expected).to_anyhow()
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct ExpectedConfig {
    pub recipient: &'static str,
    pub salutations: &'static [&'static str],
    pub is_excited: bool,
    pub is_quiet: bool,
    pub sample_value: Option<&'static str>,
    pub other: Option<&'static str>,
}

impl Default for ExpectedConfig {
    fn default() -> Self {
        Self {
            recipient: "World",
            salutations: &["Hello"],
            is_excited: false,
            is_quiet: false,
            sample_value: None,
            other: None,
        }
    }
}

pub(crate) fn load_from_iter<I>(args: I) -> OrthoResult<TestConfig>
where
    I: IntoIterator<Item = &'static str>,
{
    TestConfig::load_from_iter(args)
}

fn validation_mismatch<T>(key: &str, expected: String, actual: T) -> OrthoResult<()>
where
    T: fmt::Debug,
{
    Err(OrthoError::Validation {
        key: key.to_owned(),
        message: format!("expected {expected}, got {actual:?}"),
    }
    .into())
}

pub(crate) fn assert_config_eq(config: &TestConfig, expected: &ExpectedConfig) -> OrthoResult<()> {
    if config.recipient != expected.recipient {
        return validation_mismatch(
            "recipient",
            expected.recipient.to_owned(),
            &config.recipient,
        );
    }

    let actual_salutations: Vec<&str> = config.salutations.iter().map(String::as_str).collect();
    if actual_salutations.as_slice() != expected.salutations {
        return validation_mismatch(
            "salutations",
            format!("{:?}", expected.salutations),
            actual_salutations,
        );
    }

    if config.is_excited != expected.is_excited {
        return validation_mismatch(
            "is_excited",
            expected.is_excited.to_string(),
            config.is_excited,
        );
    }

    if config.is_quiet != expected.is_quiet {
        return validation_mismatch(
            "is_quiet",
            expected.is_quiet.to_string(),
            config.is_quiet,
        );
    }

    if config.sample_value.as_deref() != expected.sample_value {
        return validation_mismatch(
            "sample_value",
            format!("{:?}", expected.sample_value),
            config.sample_value.as_deref(),
        );
    }

    if config.other.as_deref() != expected.other {
        return validation_mismatch(
            "other",
            format!("{:?}", expected.other),
            config.other.as_deref(),
        );
    }

    Ok(())
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
