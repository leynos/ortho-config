//! Shared nested-command documentation fixture for behavioural IR tests.

use clap::{Args, Parser, Subcommand};
use ortho_config::{OrthoConfig, OrthoConfigSubcommandDocs};
use rstest::fixture;
use rstest_bdd::Slot;
use rstest_bdd_macros::ScenarioState;
use serde::{Deserialize, Serialize};

/// Scenario state for nested documentation IR scenarios.
#[derive(Debug, Default, ScenarioState)]
pub struct NestedDocsContext {
    pub metadata: Slot<ortho_config::docs::DocMetadata>,
}

/// Provides a clean nested documentation context for IR scenarios.
#[fixture]
pub fn nested_docs_context() -> NestedDocsContext {
    NestedDocsContext::default()
}

/// Root configuration for recursive documentation metadata assertions.
#[derive(Debug, Parser, Deserialize, Serialize, OrthoConfig)]
#[ortho_config(
    prefix = "NESTED_APP_",
    discovery(app_name = "nested-app"),
    windows(module_name = "Nested", include_common_parameters = true)
)]
pub struct NestedDocsConfig {
    #[arg(long)]
    pub global: String,
    #[serde(skip)]
    #[command(subcommand)]
    pub command: NestedDocsCommand,
}

/// Top-level commands used by the nested documentation fixture.
#[derive(Debug, Subcommand, OrthoConfigSubcommandDocs)]
pub enum NestedDocsCommand {
    Greet(NestedGreetArgs),
    Version(NestedVersionArgs),
    Admin(NestedAdminArgs),
}

impl Default for NestedDocsCommand {
    fn default() -> Self {
        Self::Greet(NestedGreetArgs::default())
    }
}

/// Leaf command with options, defaults, and command-level examples.
#[derive(Debug, Args, Default, Deserialize, Serialize, OrthoConfig)]
#[ortho_config(
    prefix = "NESTED_APP_",
    discovery(app_name = "greet"),
    example(
        code = "nested-app greet --recipient Ada",
        title_id = "nested-app.examples.greet.title"
    )
)]
pub struct NestedGreetArgs {
    #[arg(long)]
    pub excited: bool,
    #[arg(long)]
    #[ortho_config(default = String::from("World"))]
    pub recipient: String,
}

/// Leaf command with no options.
#[derive(Debug, Args, Default, Deserialize, Serialize, OrthoConfig)]
#[ortho_config(prefix = "NESTED_APP_")]
pub struct NestedVersionArgs {
    // `OrthoConfig` introspects named fields to generate config, serde, and
    // docs mappings. A no-options command must therefore use an empty braced
    // struct here; unit or tuple structs are rejected by the derive parser.
}

/// Top-level command that contains its own subcommand selector.
#[derive(Debug, Args, Default, Deserialize, Serialize, OrthoConfig)]
#[ortho_config(
    prefix = "NESTED_APP_",
    windows(module_name = "NestedAdmin", split_subcommands = true)
)]
pub struct NestedAdminArgs {
    #[arg(long)]
    #[ortho_config(example(
        code = "nested-app admin --scope tenant-a audit --dry-run",
        title_id = "nested-app.examples.admin.scope.title"
    ))]
    pub scope: Option<String>,
    #[serde(skip)]
    #[command(subcommand)]
    pub command: NestedAdminCommand,
}

/// Second-level admin commands used to prove recursive command metadata.
#[derive(Debug, Subcommand, OrthoConfigSubcommandDocs)]
pub enum NestedAdminCommand {
    Audit(NestedAuditArgs),
    #[command(name = "grant-access")]
    Grant(NestedGrantArgs),
}

impl Default for NestedAdminCommand {
    fn default() -> Self {
        Self::Audit(NestedAuditArgs::default())
    }
}

/// Audit command that exposes a boolean option.
#[derive(Debug, Args, Default, Deserialize, Serialize, OrthoConfig)]
#[ortho_config(prefix = "NESTED_APP_", discovery(app_name = "audit"))]
pub struct NestedAuditArgs {
    #[arg(long)]
    pub dry_run: bool,
}

/// Access-grant command that exercises `#[command(name = ...)]` overrides.
#[derive(Debug, Args, Default, Deserialize, Serialize, OrthoConfig)]
#[ortho_config(prefix = "NESTED_APP_")]
pub struct NestedGrantArgs {
    #[arg(long)]
    pub principal: String,
}
