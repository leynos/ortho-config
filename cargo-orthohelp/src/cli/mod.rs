//! Command-line interface definitions for `cargo-orthohelp`.
//!
//! Cargo treats binaries named `cargo-*` as external subcommands, so this
//! module models the wrapper shape that `cargo orthohelp` expects while still
//! supporting direct execution of `cargo-orthohelp`. `Cli` is the top-level
//! parser, `CargoSubcommand` names the external subcommand entrypoint, and
//! `Args` carries the `orthohelp` options that drive documentation
//! generation. `main.rs` calls `Cli::parse()`, matches
//! `CargoSubcommand::Orthohelp(args)`, and passes those arguments through the
//! metadata, localization, and output pipeline.

use camino::Utf8PathBuf;
use cargo_orthohelp::policy::PolicyMode;
use clap::{ArgAction, Args as ClapArgs, Parser, Subcommand, ValueEnum};

/// Output formats supported by `cargo-orthohelp`.
#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum OutputFormat {
    /// Emit the localized IR JSON output.
    Ir,
    /// Emit Unix roff man pages.
    Man,
    /// Emit `PowerShell` help output.
    Ps,
    /// Emit all outputs (IR, man pages, `PowerShell` help, and agent-context).
    All,
    /// Emit compact agent-context JSON output.
    AgentContext,
}

/// Policy enforcement modes supported by `--check-agent-native`.
#[derive(Debug, Clone, Copy, ValueEnum, PartialEq, Eq)]
pub enum PolicyModeArg {
    /// Do not evaluate policy rules.
    Off,
    /// Emit policy findings without failing the command.
    Warn,
    /// Fail the command when a deny-level finding is emitted.
    Deny,
}

impl From<PolicyModeArg> for PolicyMode {
    fn from(value: PolicyModeArg) -> Self {
        match value {
            PolicyModeArg::Off => Self::Off,
            PolicyModeArg::Warn => Self::Warn,
            PolicyModeArg::Deny => Self::Deny,
        }
    }
}

/// Parsed Cargo external-subcommand entrypoint for `cargo-orthohelp`.
#[derive(Debug, Parser)]
#[command(name = "cargo")]
#[command(bin_name = "cargo")]
#[command(version)]
pub struct Cli {
    /// Cargo subcommand dispatched to this binary.
    #[command(subcommand)]
    pub command: CargoSubcommand,
}

/// Cargo external subcommands implemented by `cargo-orthohelp`.
#[derive(Debug, Subcommand)]
pub enum CargoSubcommand {
    /// Generate localized `OrthoConfig` documentation IR.
    #[command(version)]
    Orthohelp(Args),
}

/// Parsed CLI arguments for the `orthohelp` Cargo subcommand.
#[derive(Debug, ClapArgs, Clone)]
pub struct Args {
    /// Cargo package to document.
    #[arg(long)]
    pub package: Option<String>,
    /// Binary target name (used for metadata validation).
    #[arg(long)]
    pub bin: Option<String>,
    /// Select the package's library target.
    #[arg(long = "lib")]
    pub is_lib: bool,
    /// Root configuration type (for example, `my_crate::Config`).
    #[arg(long, value_name = "path::Type")]
    pub root_type: Option<String>,
    /// Locale to render (repeat for multiple locales).
    #[arg(long, value_name = "locale")]
    pub locale: Vec<String>,
    /// Generate for every locale declared in package metadata.
    #[arg(long = "all-locales")]
    pub should_use_all_locales: bool,
    /// Output directory for generated artefacts.
    #[arg(long, value_name = "path")]
    pub out_dir: Option<Utf8PathBuf>,
    /// Bridge cache behaviour flags.
    #[command(flatten)]
    pub cache: CacheArgs,
    /// Output format selection.
    #[arg(long, value_enum, default_value_t = OutputFormat::Ir)]
    pub format: OutputFormat,
    /// Evaluate the minimal agent-native policy and emit a JSON report to stdout.
    #[arg(long = "check-agent-native")]
    pub should_check_agent_native: bool,
    /// Enforcement mode for `--check-agent-native`.
    #[arg(long, value_enum, default_value_t = PolicyModeArg::Warn)]
    pub policy_mode: PolicyModeArg,
    /// Man page generation arguments.
    #[command(flatten)]
    pub man: ManArgs,
    /// `PowerShell` generation arguments.
    #[command(flatten)]
    pub powershell: PowerShellArgs,
}

/// Bridge cache behaviour flags.
#[derive(Debug, ClapArgs, Clone, Copy)]
pub struct CacheArgs {
    /// Cache and reuse the bridge IR when possible.
    #[arg(long = "cache")]
    pub should_cache: bool,
    /// Skip building the bridge and rely on cached IR.
    #[arg(long = "no-build")]
    pub should_skip_build: bool,
}

/// Man page generation arguments.
#[derive(Debug, ClapArgs, Clone)]
pub struct ManArgs {
    /// Man page section number (1-8, default: 1 for user commands).
    #[arg(
        long = "man-section",
        value_name = "N",
        default_value = "1",
        value_parser = clap::value_parser!(u8).range(1..=8)
    )]
    pub section: u8,
    /// Date for man page header (format: YYYY-MM-DD or "January 2026").
    #[arg(long = "man-date", value_name = "DATE")]
    pub date: Option<String>,
    /// Generate separate man pages for each subcommand.
    #[arg(long = "man-split-subcommands")]
    pub should_split_subcommands: bool,
}

/// `PowerShell` help generation arguments.
#[derive(Debug, ClapArgs, Clone)]
pub struct PowerShellArgs {
    /// `PowerShell` module name override.
    #[arg(long = "ps-module-name", value_name = "NAME")]
    pub module_name: Option<String>,
    /// Split subcommands into separate wrapper functions.
    #[arg(
        id = "ps_should_split_subcommands",
        long = "ps-split-subcommands",
        value_name = "BOOL",
        action = ArgAction::Set
    )]
    pub should_split_subcommands: Option<bool>,
    /// Include `CommonParameters` in help output.
    #[arg(
        long = "ps-include-common-parameters",
        value_name = "BOOL",
        action = ArgAction::Set
    )]
    pub should_include_common_parameters: Option<bool>,
    /// `HelpInfoUri` for Update-Help payloads.
    #[arg(long = "ps-help-info-uri", value_name = "URI")]
    pub help_info_uri: Option<String>,
    /// Ensure an en-US help file exists.
    #[arg(
        long = "ensure-en-us",
        value_name = "BOOL",
        default_value_t = true,
        action = ArgAction::Set
    )]
    pub should_ensure_en_us: bool,
}

#[cfg(test)]
mod tests {
    //! Parser tests for Cargo external-subcommand dispatch.

    use clap::{CommandFactory, Parser, error::ErrorKind};
    use rstest::rstest;

    use super::{CargoSubcommand, Cli, OutputFormat, PolicyModeArg};
    use cargo_orthohelp::policy::PolicyMode;

    #[test]
    fn format_defaults_to_ir() {
        let cli = Cli::parse_from(["cargo-orthohelp", "orthohelp"]);
        let CargoSubcommand::Orthohelp(args) = cli.command;

        assert!(matches!(args.format, OutputFormat::Ir));
    }

    #[rstest]
    #[case("ir", OutputFormat::Ir)]
    #[case("man", OutputFormat::Man)]
    #[case("ps", OutputFormat::Ps)]
    #[case("all", OutputFormat::All)]
    fn format_accepts_legacy_values(#[case] value: &str, #[case] expected: OutputFormat) {
        let cli = Cli::parse_from(["cargo-orthohelp", "orthohelp", "--format", value]);
        let CargoSubcommand::Orthohelp(args) = cli.command;

        assert_eq!(
            std::mem::discriminant(&args.format),
            std::mem::discriminant(&expected)
        );
    }

    #[test]
    fn format_accepts_agent_context() {
        let cli = Cli::parse_from(["cargo-orthohelp", "orthohelp", "--format", "agent-context"]);
        let CargoSubcommand::Orthohelp(args) = cli.command;

        assert!(matches!(args.format, OutputFormat::AgentContext));
    }

    #[rstest]
    #[case("off", PolicyMode::Off)]
    #[case("warn", PolicyMode::Warn)]
    #[case("deny", PolicyMode::Deny)]
    fn policy_mode_maps_to_the_policy_contract(#[case] value: &str, #[case] expected: PolicyMode) {
        let cli = Cli::parse_from([
            "cargo-orthohelp",
            "orthohelp",
            "--check-agent-native",
            "--policy-mode",
            value,
        ]);
        let CargoSubcommand::Orthohelp(args) = cli.command;

        assert!(args.should_check_agent_native);
        assert_eq!(PolicyMode::from(args.policy_mode), expected);
    }

    #[test]
    fn policy_mode_defaults_to_warn() {
        let cli = Cli::parse_from(["cargo-orthohelp", "orthohelp"]);
        let CargoSubcommand::Orthohelp(args) = cli.command;

        assert_eq!(args.policy_mode, PolicyModeArg::Warn);
    }

    #[test]
    fn parses_cargo_injected_subcommand_arguments() {
        let cli = Cli::parse_from([
            "cargo-orthohelp",
            "orthohelp",
            "--package",
            "fixture",
            "--locale",
            "en-US",
            "--format",
            "man",
        ]);

        let CargoSubcommand::Orthohelp(args) = cli.command;
        assert_eq!(args.package.as_deref(), Some("fixture"));
        assert_eq!(args.locale, [String::from("en-US")]);
        assert!(matches!(args.format, OutputFormat::Man));
    }

    #[test]
    fn rejects_options_without_injected_subcommand() {
        let error = Cli::try_parse_from(["cargo-orthohelp", "--format", "ir"])
            .expect_err("top-level options should require the Cargo subcommand");

        assert_eq!(error.kind(), ErrorKind::UnknownArgument);
    }

    #[test]
    fn rejects_unknown_output_format() {
        let error = Cli::try_parse_from(["cargo-orthohelp", "orthohelp", "--format", "foo"])
            .expect_err("unknown output formats should be rejected");

        assert_eq!(error.kind(), ErrorKind::InvalidValue);
    }

    #[test]
    fn format_rejects_unsupported_values() {
        let error = Cli::try_parse_from(["cargo-orthohelp", "orthohelp", "--format", "xml"])
            .expect_err("unsupported formats should fail before generation");

        assert_eq!(error.kind(), ErrorKind::InvalidValue);
    }

    #[test]
    fn rejects_invalid_powershell_bool() {
        let error = Cli::try_parse_from([
            "cargo-orthohelp",
            "orthohelp",
            "--ps-split-subcommands",
            "notabool",
        ])
        .expect_err("invalid bool values should be rejected");

        assert_eq!(error.kind(), ErrorKind::InvalidValue);
    }

    #[test]
    fn top_level_help_uses_cargo_dispatch_name() {
        let help = Cli::command().render_help().to_string();

        assert!(
            help.contains("Usage: cargo <COMMAND>"),
            "unexpected top-level help:\n{help}"
        );
    }

    #[test]
    fn subcommand_help_uses_cargo_dispatch_name() {
        let help = Cli::command()
            .try_get_matches_from(["cargo-orthohelp", "orthohelp", "--help"])
            .expect_err("help should short-circuit parsing")
            .to_string();

        assert!(
            help.contains("Usage: cargo orthohelp [OPTIONS]"),
            "unexpected subcommand help:\n{help}"
        );
    }
}

#[cfg(test)]
mod property_tests;

#[cfg(test)]
mod reserved_agent_context_tests;
