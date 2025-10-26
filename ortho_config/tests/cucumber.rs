//! Cucumber-based integration tests for `ortho_config`.
//!
//! Exercises end-to-end configuration loading using [`CsvEnv`] and the
//! derive macro.

use clap::{Args, Parser};
use cucumber::World as _;
use ortho_config::OrthoConfig;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

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
    pub result: Option<ortho_config::OrthoResult<RulesConfig>>,
    /// CLI reference value for subcommand scenarios.
    sub_ref: Option<String>,
    /// Configuration file reference for subcommand scenarios.
    sub_file: Option<String>,
    /// Environment variable reference for subcommand scenarios.
    sub_env: Option<String>,
    /// Result of subcommand configuration loading.
    pub sub_result: Option<Result<PrArgs, anyhow::Error>>,
    /// Result of aggregated error scenario.
    pub agg_result: Option<ortho_config::OrthoResult<ErrorConfig>>,
    /// File contents for flattened merging scenarios.
    flat_file: Option<String>,
    /// Result of flattened configuration loading.
    pub(crate) flat_result: Option<ortho_config::OrthoResult<FlatArgs>>,
    /// Dynamic rules file contents for collection strategy scenarios.
    dynamic_rules_file: Option<String>,
    /// Environment-provided dynamic rules for collection strategy scenarios.
    dynamic_rules_env: Vec<(String, bool)>,
}

/// CLI struct used for subcommand behavioural tests.
#[derive(Debug, Deserialize, Serialize, Parser, OrthoConfig, Default, Clone)]
#[command(name = "test")]
#[ortho_config(prefix = "APP_")]
pub struct PrArgs {
    #[arg(long, required = true)]
    reference: Option<String>,
}

/// CLI struct used for flattened merging tests.
#[derive(Debug, Deserialize, Serialize, Parser, Default, Clone)]
pub(crate) struct FlatArgs {
    #[command(flatten)]
    pub(crate) nested: NestedArgs,
}

/// Nested group flattened into [`FlatArgs`]; mimics `#[command(flatten)]` usage.
#[derive(Debug, Deserialize, Serialize, Args, Default, Clone)]
pub(crate) struct NestedArgs {
    #[arg(long)]
    pub(crate) value: Option<String>,
}

/// Configuration struct used in integration tests.
///
/// The `DDLINT_` prefix is applied to environment variables and rule lists may
/// be specified as comma-separated strings via [`CsvEnv`]. Dynamic rule tables
/// and ignore pattern lists are also supported.
#[derive(Debug, Deserialize, Serialize, OrthoConfig, Default)]
#[ortho_config(prefix = "DDLINT_")]
pub struct RulesConfig {
    /// List of lint rules parsed from CLI or environment.
    #[serde(default)]
    rules: Vec<String>,
    /// Patterns to exclude when scanning files.
    #[serde(default)]
    #[ortho_config(merge_strategy = "append")]
    ignore_patterns: Vec<String>,
    /// Dynamic rules table demonstrating replace semantics.
    #[serde(default)]
    #[ortho_config(skip_cli, merge_strategy = "replace")]
    dynamic_rules: BTreeMap<String, DynamicRule>,
    /// Optional configuration path using a custom flag name.
    #[serde(skip)]
    #[ortho_config(cli_long = "config")]
    #[expect(dead_code, reason = "used indirectly via CLI flag in tests")]
    config_path: Option<std::path::PathBuf>,
}

/// Toggleable rule used by collection merge behavioural tests.
#[derive(Debug, Deserialize, Serialize, Default)]
pub struct DynamicRule {
    enabled: bool,
}

/// Configuration used to verify aggregated error reporting.
///
/// # Examples
/// Load from environment variable `DDLINT_PORT`:
/// ```
/// std::env::set_var("DDLINT_PORT", "8080");
/// let cfg = ErrorConfig::load().expect("load ErrorConfig");
/// assert_eq!(cfg.port, Some(8080));
/// ```
///
/// Invalid values contribute to an aggregated error:
/// ```
/// std::env::set_var("DDLINT_PORT", "not-a-number");
/// let err = ErrorConfig::load().expect_err("expect aggregated error");
/// assert!(matches!(err, ortho_config::OrthoError::Aggregate(_)));
/// ```
#[derive(Debug, Deserialize, Serialize, OrthoConfig, Default)]
#[ortho_config(prefix = "DDLINT_")]
pub struct ErrorConfig {
    /// Port number sourced from configuration layers for the test server.
    pub port: Option<u32>,
}

mod steps;

#[tokio::main]
async fn main() {
    World::run("tests/features").await;
}
