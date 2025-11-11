//! Shared fixtures for the `rstest-bdd` behavioural scaffolding.

use anyhow::Error;
use clap::{Args, Parser};
use ortho_config::OrthoConfig;
use rstest::fixture;
use rstest_bdd::Slot;
use rstest_bdd_macros::ScenarioState;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::PathBuf;

/// Scenario state shared by the cucumber-parity behavioural tests.
#[derive(Debug, Default, ScenarioState)]
pub struct World {
    pub env_value: Slot<String>,
    pub file_value: Slot<String>,
    pub extends_flag: Slot<()>,
    pub cyclic_flag: Slot<()>,
    pub missing_base_flag: Slot<()>,
    pub result: Slot<ortho_config::OrthoResult<RulesConfig>>,
    pub sub_sources: Slot<SubcommandSources>,
    pub sub_result: Slot<Result<PrArgs, Error>>,
    pub agg_result: Slot<ortho_config::OrthoResult<ErrorConfig>>,
    pub flat_file: Slot<String>,
    pub flat_result: Slot<ortho_config::OrthoResult<FlatArgs>>,
    pub dynamic_rules_file: Slot<String>,
    pub dynamic_rules_env: Slot<Vec<(String, bool)>>,
}

/// Captures the optional reference inputs used by subcommand scenarios.
#[derive(Debug, Default, Clone)]
pub struct SubcommandSources {
    pub cli: Option<String>,
    pub file: Option<String>,
    pub env: Option<String>,
}

impl SubcommandSources {
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.cli.is_none() && self.file.is_none() && self.env.is_none()
    }
}

/// Provides a clean behavioural world state per scenario.
#[fixture]
pub fn world() -> World {
    World::default()
}

/// Minimal configuration struct used by the rstest-bdd canary scenario.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, OrthoConfig)]
pub struct CanaryConfig {
    #[ortho_config(default = 7)]
    pub level: u8,
}

/// Scenario state that shares the last-loaded canary config between steps.
#[derive(Debug, Default, ScenarioState)]
pub struct CanaryState {
    pub loaded_config: Slot<CanaryConfig>,
}

/// Creates a clean canary state so steps can share loaded configs.
#[fixture]
pub fn canary_state() -> CanaryState {
    CanaryState::default()
}

/// Provides the logical binary name used when constructing CLI args.
#[fixture]
pub fn binary_name() -> &'static str {
    "ortho-config-bdd"
}

/// CLI struct used for subcommand behavioural tests.
#[derive(Debug, Deserialize, Serialize, Parser, OrthoConfig, Default, Clone)]
#[command(name = "test")]
#[ortho_config(prefix = "APP_")]
pub struct PrArgs {
    #[arg(long, required = true)]
    pub reference: Option<String>,
}

/// CLI struct used for flattened merging tests.
#[derive(Debug, Deserialize, Serialize, Parser, Default, Clone)]
pub struct FlatArgs {
    #[command(flatten)]
    pub nested: NestedArgs,
}

/// Nested group flattened into [`FlatArgs`]; mimics `#[command(flatten)]` usage.
#[derive(Debug, Deserialize, Serialize, Args, Default, Clone)]
pub struct NestedArgs {
    #[arg(long)]
    pub value: Option<String>,
}

/// Configuration struct used in integration tests.
///
/// The `DDLINT_` prefix is applied to environment variables and rule lists may
/// be specified as comma-separated strings via `CsvEnv`. Dynamic rule tables
/// and ignore pattern lists are also supported.
#[derive(Debug, Deserialize, Serialize, OrthoConfig, Default)]
#[ortho_config(prefix = "DDLINT_")]
pub struct RulesConfig {
    #[serde(default)]
    pub rules: Vec<String>,
    #[serde(default)]
    #[ortho_config(merge_strategy = "append")]
    pub ignore_patterns: Vec<String>,
    #[serde(default)]
    #[ortho_config(skip_cli, merge_strategy = "replace")]
    pub dynamic_rules: BTreeMap<String, DynamicRule>,
    #[serde(skip)]
    #[ortho_config(cli_long = "config")]
    #[expect(dead_code, reason = "used indirectly via CLI flag in tests")]
    pub config_path: Option<PathBuf>,
}

/// Toggleable rule used by collection merge behavioural tests.
#[derive(Debug, Deserialize, Serialize, Default)]
pub struct DynamicRule {
    pub enabled: bool,
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
    pub port: Option<u32>,
}
