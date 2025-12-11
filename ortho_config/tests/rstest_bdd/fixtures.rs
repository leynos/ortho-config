//! Shared fixtures for the `rstest-bdd` behavioural scaffolding.

use anyhow::Error;
use clap::{Args, Parser};
use ortho_config::{Localizer, MergeLayer, OrthoConfig};
use rstest::fixture;
use rstest_bdd::Slot;
use rstest_bdd_macros::ScenarioState;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

pub use super::common::merge_fixtures::MergeErrorSample;

/// Scenario state for rules-oriented precedence scenarios (CLI, env, config path, ignore).
#[derive(Debug, Default, ScenarioState)]
pub struct RulesContext {
    pub env_value: Slot<String>,
    pub file_value: Slot<String>,
    pub result: Slot<ortho_config::OrthoResult<RulesConfig>>,
}

/// Scenario state for collection merge scenarios.
#[derive(Debug, Default, ScenarioState)]
pub struct CollectionContext {
    pub dynamic_rules_file: Slot<String>,
    pub dynamic_rules_env: Slot<Vec<(String, bool)>>,
    pub result: Slot<ortho_config::OrthoResult<RulesConfig>>,
}

/// Scenario state for configuration inheritance scenarios.
#[derive(Debug, Default, ScenarioState)]
pub struct ExtendsContext {
    pub extends_flag: Slot<()>,
    pub cyclic_flag: Slot<()>,
    pub missing_base_flag: Slot<()>,
    pub result: Slot<ortho_config::OrthoResult<RulesConfig>>,
}

/// Scenario state for merge composer builder scenarios.
#[derive(Debug, Default, ScenarioState)]
pub struct ComposerContext {
    pub layers: Slot<Vec<MergeLayer<'static>>>,
    pub config: Slot<RulesConfig>,
}

/// Scenario state for aggregated error reporting scenarios.
#[derive(Debug, Default, ScenarioState)]
pub struct ErrorContext {
    pub env_value: Slot<String>,
    pub file_value: Slot<String>,
    pub agg_result: Slot<ortho_config::OrthoResult<ErrorConfig>>,
}

/// Scenario state for flattened CLI merging scenarios.
#[derive(Debug, Default, ScenarioState)]
pub struct FlattenContext {
    pub flat_file: Slot<String>,
    pub flat_result: Slot<ortho_config::OrthoResult<FlatArgs>>,
}

/// Scenario state for subcommand precedence scenarios.
#[derive(Debug, Default, ScenarioState)]
pub struct SubcommandContext {
    pub sources: Slot<SubcommandSources>,
    pub result: Slot<Result<PrArgs, Error>>,
}

/// Scenario state for localisation helper scenarios.
#[derive(Debug, Default, ScenarioState, Clone)]
pub struct LocalizerContext {
    pub localizer: Slot<Box<dyn Localizer + 'static>>,
    pub resolved: Slot<String>,
    pub issues: Slot<Arc<Mutex<Vec<String>>>>,
    pub clap_error: Slot<clap::Error>,
    pub baseline_error: Slot<String>,
    pub argument_label: Slot<String>,
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

impl LocalizerContext {
    #[must_use]
    pub fn init_issues() -> Slot<Arc<Mutex<Vec<String>>>> {
        let slot = Slot::default();
        slot.set(Arc::new(Mutex::new(Vec::new())));
        slot
    }

    fn issues_arc(&self) -> Option<Arc<Mutex<Vec<String>>>> {
        self.issues.with_ref(|issues| Arc::clone(issues))
    }

    pub fn record_issue(&self, id: String) {
        if let Some(issues) = self.issues_arc() {
            if let Ok(mut guard) = issues.lock() {
                guard.push(id);
            }
        }
    }

    #[must_use]
    pub fn take_issues(&self) -> Vec<String> {
        if let Some(issues) = self.issues.take() {
            let mut guard = issues
                .lock()
                .expect("formatting issue mutex poisoned during take");
            let collected = guard.clone();
            guard.clear();
            collected
        } else {
            Vec::new()
        }
    }
}

/// Provides a clean rules context so precedence-focused steps can share state.
#[fixture]
pub fn rules_context() -> RulesContext {
    RulesContext::default()
}

/// Provides a clean collection context for collection merge scenarios.
#[fixture]
pub fn collection_context() -> CollectionContext {
    CollectionContext::default()
}

/// Provides a clean extends context for inheritance scenarios.
#[fixture]
pub fn extends_context() -> ExtendsContext {
    ExtendsContext::default()
}

/// Provides a clean composer context for layer-composition scenarios.
#[fixture]
pub fn composer_context() -> ComposerContext {
    ComposerContext::default()
}

/// Provides a clean error context for aggregated error scenarios.
#[fixture]
pub fn error_context() -> ErrorContext {
    ErrorContext::default()
}

/// Provides a clean flatten context for flattened CLI scenarios.
#[fixture]
pub fn flatten_context() -> FlattenContext {
    FlattenContext::default()
}

/// Provides a clean subcommand context so subcommand steps can share state.
#[fixture]
pub fn subcommand_context() -> SubcommandContext {
    SubcommandContext::default()
}

/// Provides a clean localisation context so translation scenarios share state.
#[fixture]
pub fn localizer_context() -> LocalizerContext {
    LocalizerContext {
        issues: LocalizerContext::init_issues(),
        ..LocalizerContext::default()
    }
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

/// Scenario state for merge error routing scenarios.
#[derive(Debug, Default, ScenarioState)]
pub struct MergeErrorContext {
    pub layers: Slot<Vec<MergeLayer<'static>>>,
    pub result: Slot<ortho_config::OrthoResult<MergeErrorSample>>,
}


/// Provides a clean merge error context for error routing scenarios.
#[fixture]
pub fn merge_error_context() -> MergeErrorContext { MergeErrorContext::default() }

/// Configuration used to verify aggregated error reporting.
///
/// # Examples
/// Load from environment variable `DDLINT_PORT`:
/// ```
/// figment::Jail::expect_with(|jail| {
///     jail.set_env("DDLINT_PORT", "8080");
///     let cfg = ErrorConfig::load().expect("load ErrorConfig");
///     assert_eq!(cfg.port, Some(8080));
///     Ok(())
/// });
/// ```
///
/// Invalid values contribute to an aggregated error:
/// ```
/// figment::Jail::expect_with(|jail| {
///     jail.set_env("DDLINT_PORT", "not-a-number");
///     let err = ErrorConfig::load().expect_err("expect aggregated error");
///     assert!(matches!(err, ortho_config::OrthoError::Aggregate(_)));
///     Ok(())
/// });
/// ```
#[derive(Debug, Deserialize, Serialize, OrthoConfig, Default)]
#[ortho_config(prefix = "DDLINT_")]
pub struct ErrorConfig {
    pub port: Option<u32>,
}
