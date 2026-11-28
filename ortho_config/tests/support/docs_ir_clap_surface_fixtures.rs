//! Fixture declarations for the clap documentation-surface regressions.
//!
//! These types are compiled through the real derives, so they live beside the
//! regression tests that assert the generated metadata rather than inside the
//! test module, which keeps that suite within its line budget.

use clap::{Args, Parser};
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

/// Arguments where one field inherits a parent command's global argument.
#[derive(Debug, Clone, Default, Args, Serialize, Deserialize, OrthoConfig)]
#[ortho_config(prefix = "SPLITTERS")]
pub(super) struct GlobalArgs {
    /// Inherited from the parent command, not a field-local argument.
    #[arg(from_global)]
    verbose: bool,
    /// Ordinary positional operand owned by this command.
    #[arg(value_name = "INPUT")]
    input: String,
}

/// The value name is a constant, so the derive cannot read it statically.
pub(super) const PROPOSAL_VALUE_NAME: &str = "PROPOSAL-LITERAL";

/// Arguments whose clap declaration is valid but not statically readable.
#[derive(Debug, Clone, Default, Parser, Serialize, Deserialize, OrthoConfig)]
#[ortho_config(prefix = "SPLITTERS")]
pub(super) struct UnreadableArgs {
    /// Proposal identifier with a non-literal clap value name.
    #[arg(value_name = PROPOSAL_VALUE_NAME)]
    proposal: String,
}

/// Arguments that mix declared and undeclared fields.
///
/// clap parses every field of a struct that declares any argument, so the
/// undeclared fields are positional operands in declaration order.
#[derive(Debug, Clone, Default, Parser, Serialize, Deserialize, OrthoConfig)]
#[ortho_config(prefix = "SPLITTERS")]
pub(super) struct PartlyDeclaredArgs {
    /// Undeclared operand clap positions first.
    source: String,
    /// Declared operand clap positions second.
    #[arg(value_name = "SECOND")]
    second: String,
    /// Named flag declared between the operands.
    #[arg(long)]
    verbose: bool,
}

/// Arguments whose first operand has an unreadable value name.
///
/// clap still parses the operand and places `second` at index 2, so the
/// documentation must count the field it does not describe.
#[derive(Debug, Clone, Default, Parser, Serialize, Deserialize, OrthoConfig)]
#[ortho_config(prefix = "SPLITTERS")]
pub(super) struct UnreadableOperandArgs {
    /// Operand whose placeholder is a constant.
    #[arg(value_name = PROPOSAL_VALUE_NAME)]
    proposal: String,
    /// Operand that follows it.
    #[arg(value_name = "SECOND")]
    second: String,
}

/// Arguments whose first argument is an unreadable named option.
#[derive(Debug, Clone, Default, Parser, Serialize, Deserialize, OrthoConfig)]
#[ortho_config(prefix = "SPLITTERS")]
pub(super) struct UnreadableOptionArgs {
    /// Named option whose placeholder is a constant.
    #[arg(long, value_name = PROPOSAL_VALUE_NAME)]
    check: Option<String>,
    /// Operand that follows the named option.
    #[arg(value_name = "INPUT")]
    input: String,
}

/// The long flag clap reads from a constant expression.
pub(super) const FLAG_NAME: &str = "check";

/// The short flag clap reads from a constant expression.
pub(super) const FLAG_CHAR: char = 'c';

/// An explicit positional index clap reads from a constant expression.
pub(super) const FIRST_INDEX: usize = 2;

/// Arguments whose flag spellings are constant expressions.
#[derive(Debug, Clone, Default, Parser, Serialize, Deserialize, OrthoConfig)]
#[ortho_config(prefix = "SPLITTERS")]
pub(super) struct RawSpellingArgs {
    /// Named option whose spellings the derive cannot read.
    #[arg(long(FLAG_NAME), short(FLAG_CHAR))]
    check: Option<String>,
    /// Operand that follows the named option.
    #[arg(value_name = "INPUT")]
    input: String,
}

/// Arguments whose explicit index is a constant expression.
#[derive(Debug, Clone, Default, Parser, Serialize, Deserialize, OrthoConfig)]
#[ortho_config(prefix = "SPLITTERS")]
pub(super) struct RawIndexArgs {
    /// Operand whose index the derive cannot read.
    #[arg(value_name = "FIRST", index(FIRST_INDEX))]
    first: String,
    /// Operand that clap numbers from its implicit counter.
    #[arg(value_name = "SECOND")]
    second: String,
}

/// A flattened child that owns an operand.
#[derive(Debug, Clone, Default, Args, Serialize, Deserialize)]
pub(super) struct SharedOperandArgs {
    /// Operand owned by the flattened child.
    #[arg(value_name = "SHARED")]
    shared: String,
}

/// Arguments whose only clap marker is a flattened child owning an operand.
#[derive(Debug, Clone, Default, Parser, Serialize, Deserialize, OrthoConfig)]
#[ortho_config(prefix = "SPLITTERS")]
pub(super) struct FlattenedOperandArgs {
    /// Flattened child whose operand shifts later indices.
    #[command(flatten)]
    #[ortho_config(skip_cli)]
    shared: SharedOperandArgs,
    /// Parent operand whose absolute index only clap knows.
    input: String,
}
