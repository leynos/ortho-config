//! Fixture declarations for the clap documentation-surface regressions.
//!
//! These types are compiled through the real derives, so they live beside the
//! regression tests that assert the generated metadata rather than inside the
//! test module, which keeps that suite within its line budget.

use clap::Parser;
use ortho_config::OrthoConfig;
use serde::{Deserialize, Serialize};

/// Top-level commands exercised by the regression.
#[derive(Debug, clap::Subcommand, ortho_config::OrthoConfigSubcommandDocs)]
pub(super) enum Commands {
    /// Validates a proposal.
    Validate(ValidateArgs),
}

/// A payload that relies on clap's default command name.
#[derive(Debug, clap::Subcommand, ortho_config::OrthoConfigSubcommandDocs)]
pub(super) enum UnnamedCommands {
    /// Runs without a declared command name.
    Run(UnnamedArgs),
}

/// Arguments whose clap command name defaults to the package name.
#[derive(Debug, Clone, Default, Parser, Serialize, Deserialize, OrthoConfig)]
#[ortho_config(prefix = "SPLITTERS")]
pub(super) struct UnnamedArgs {
    /// Token read from the mounted namespace.
    #[arg(long)]
    token: Option<String>,
}

/// Arguments that clap parses and the subcommand merge reads.
#[derive(Debug, Clone, Default, Parser, Serialize, Deserialize, OrthoConfig)]
#[command(name = "validate")]
#[ortho_config(prefix = "SPLITTERS", localization_base = "splitters.cli.validate")]
pub(super) struct ValidateArgs {
    /// Proposal identifier.
    #[arg(value_name = "PROPOSAL")]
    proposal: String,
    /// Check command.
    #[arg(long, value_name = "COMMAND")]
    check: Option<String>,
}

/// A boolean flag that clap parses as a bare switch.
#[derive(Debug, Clone, Default, Parser, Serialize, Deserialize, OrthoConfig)]
#[ortho_config(prefix = "SPLITTERS")]
pub(super) struct PurgeArgs {
    /// Skip the confirmation prompt.
    #[arg(long)]
    force: bool,
}

/// Arguments whose container casing changes the derived long flag.
#[derive(Debug, Clone, Default, Parser, Serialize, Deserialize, OrthoConfig)]
#[command(rename_all = "snake_case")]
#[ortho_config(prefix = "SPLITTERS")]
pub(super) struct SnakeCaseArgs {
    /// Output file path.
    #[arg(long)]
    output_file: String,
}

/// A clap-required argument that also carries a Serde default.
#[derive(Debug, Clone, Default, Parser, Serialize, Deserialize, OrthoConfig)]
#[ortho_config(prefix = "SPLITTERS")]
pub(super) struct HostArgs {
    /// Host to contact.
    #[arg(long)]
    #[serde(default)]
    host: String,
}

/// A skipped CLI field whose configuration default keeps it non-required.
#[derive(Debug, Clone, Default, Parser, Serialize, Deserialize, OrthoConfig)]
#[ortho_config(prefix = "SPLITTERS")]
pub(super) struct SkippedArgs {
    /// Internal value supplied by configuration.
    #[arg(skip)]
    #[ortho_config(default = String::from("fallback"))]
    internal: String,
}
