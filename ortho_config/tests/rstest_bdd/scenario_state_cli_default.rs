//! CLI default-as-absent scenario state and fixtures.
//!
//! Split from `scenario_state` so each module stays beneath the 400-line
//! cap. The steps import the state via the `scenario_state` re-exports.

use crate::cli_default_mode::CliDefaultMode;
use anyhow::Error;
use clap::Parser;
use ortho_config::OrthoConfig;
use rstest::fixture;
use rstest_bdd::Slot;
use rstest_bdd_macros::ScenarioState;
use serde::{Deserialize, Serialize};

/// Captures the optional punctuation inputs used by CLI default-as-absent scenarios.
#[derive(Debug, Default, Clone)]
pub struct CliDefaultSources {
    /// The clap default value (always present for non-Option fields).
    pub clap_default: Option<String>,
    /// Explicit CLI argument value (user typed --punctuation on CLI).
    pub explicit_cli: Option<String>,
    /// Value from configuration file.
    pub file: Option<String>,
    /// Value from environment variable.
    pub env: Option<String>,
}

/// Scenario state for `cli_default_as_absent` precedence scenarios.
#[derive(Debug, Default, ScenarioState)]
pub struct CliDefaultContext {
    pub sources: Slot<CliDefaultSources>,
    pub merge_result: Slot<Result<CliDefaultArgs, Error>>,
    pub extracted: Slot<serde_json::Value>,
}

/// CLI struct used for `cli_default_as_absent` behavioural tests.
#[derive(Debug, Deserialize, Serialize, Parser, OrthoConfig, Default, Clone, PartialEq)]
#[command(name = "greet")]
#[ortho_config(prefix = "APP_")]
pub struct CliDefaultArgs {
    /// Punctuation at the end of the greeting.
    #[arg(long, id = "punctuation", default_value = "!")]
    #[ortho_config(cli_default_as_absent)]
    pub punctuation: String,

    /// Output mode used to exercise clap `ValueEnum` default inference.
    #[arg(long, default_value = "fast", value_enum)]
    #[ortho_config(cli_default_as_absent)]
    pub mode: CliDefaultMode,
}

/// Provides a clean CLI default-as-absent context for precedence scenarios.
#[fixture]
pub fn cli_default_context() -> CliDefaultContext {
    CliDefaultContext::default()
}
