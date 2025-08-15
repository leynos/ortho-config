//! Cucumber-based integration tests for `ortho_config`.
//!
//! Exercises end-to-end configuration loading using [`CsvEnv`] and the
//! derive macro.

use clap::Parser;
use cucumber::World as _;
use ortho_config::OrthoConfig;
use serde::{Deserialize, Serialize};

/// Test world state shared between Cucumber steps.
#[derive(Debug, Default, cucumber::World)]
pub struct World {
    /// Environment variable value set during the scenario.
    env_value: Option<String>,
    /// Configuration file value set during the scenario.
    file_value: Option<String>,
    /// Whether the scenario requires an extended configuration file.
    extends: bool,
    /// Whether to create a cyclic inheritance scenario.
    cyclic: bool,
    /// Whether the base file is missing.
    missing_base: bool,
    /// Result of attempting to load configuration.
    pub result: Option<Result<RulesConfig, ortho_config::OrthoError>>,
    /// CLI reference value for subcommand scenarios.
    sub_ref: Option<String>,
    /// Configuration file reference for subcommand scenarios.
    sub_file: Option<String>,
    /// Environment variable reference for subcommand scenarios.
    sub_env: Option<String>,
    /// Result of subcommand configuration loading.
    pub sub_result: Option<Result<PrArgs, ortho_config::OrthoError>>,
    /// Result of aggregated error scenario.
    pub agg_result: Option<Result<ErrorConfig, ortho_config::OrthoError>>,
}

/// CLI struct used for subcommand behavioural tests.
#[derive(Debug, Deserialize, Serialize, Parser, OrthoConfig, Default, Clone)]
#[command(name = "test")]
#[ortho_config(prefix = "APP_")]
pub struct PrArgs {
    #[arg(long, required = true)]
    reference: Option<String>,
}

/// Configuration struct used in integration tests.
///
/// The `DDLINT_` prefix is applied to environment variables and rule lists may
/// be specified as comma-separated strings via [`CsvEnv`].
#[derive(Debug, Deserialize, Serialize, OrthoConfig, Default)]
#[ortho_config(prefix = "DDLINT_")]
pub struct RulesConfig {
    /// List of lint rules parsed from CLI or environment.
    rules: Vec<String>,
}

/// Configuration used to verify aggregated error reporting.
#[derive(Debug, Deserialize, Serialize, OrthoConfig, Default)]
#[ortho_config(prefix = "DDLINT_")]
pub struct ErrorConfig {
    port: u32,
}

mod steps;

#[tokio::main]
async fn main() {
    World::run("tests/features").await;
}
