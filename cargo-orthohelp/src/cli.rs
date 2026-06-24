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
    /// Emit all outputs (IR, man pages, and `PowerShell` help).
    All,
    /// Emit compact agent-context JSON output.
    AgentContext,
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
    use ortho_config::AGENT_CONTEXT_COMMAND;
    use proptest::prelude::*;
    use rstest::rstest;

    use super::{CargoSubcommand, Cli, OutputFormat};

    const RESERVED_AGENT_CONTEXT_ALIAS: &str = "agent-context";

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

    #[test]
    fn no_context_or_agent_context_subcommand_alias() {
        let command = Cli::command();
        let mut violations = Vec::new();

        collect_reserved_agent_context_commands(&command, &mut Vec::new(), &mut violations);

        assert!(
            violations.is_empty(),
            "reserved downstream context command names leaked into cargo-orthohelp: {violations:?}"
        );
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

    proptest! {
        #[test]
        fn parses_option_and_bool_flag_combinations(
            package in prop::option::of("[a-z][a-z0-9_-]{0,8}"),
            bin in prop::option::of("[a-z][a-z0-9_-]{0,8}"),
            root_type in prop::option::of("[A-Z][A-Za-z0-9]{0,8}"),
            locales in prop::collection::vec("[a-z]{2}(-[A-Z]{2})?", 0..4),
            should_select_lib in any::<bool>(),
            should_use_all_locales in any::<bool>(),
            should_cache in any::<bool>(),
            should_skip_build in any::<bool>(),
            should_split_man_subcommands in any::<bool>(),
            format in prop::sample::select(vec!["ir", "man", "ps", "all", "agent-context"]),
            man_section in 1_u8..=8,
            should_split_ps_subcommands in prop::option::of(any::<bool>()),
            should_include_common_parameters in prop::option::of(any::<bool>()),
            should_ensure_en_us in any::<bool>(),
        ) {
            let mut argv = vec![
                "cargo-orthohelp".to_owned(),
                "orthohelp".to_owned(),
                "--format".to_owned(),
                format.to_owned(),
                "--man-section".to_owned(),
                man_section.to_string(),
                "--ensure-en-us".to_owned(),
                should_ensure_en_us.to_string(),
            ];

            push_optional_arg(&mut argv, "--package", package.as_deref());
            push_optional_arg(&mut argv, "--bin", bin.as_deref());
            push_optional_arg(&mut argv, "--root-type", root_type.as_deref());
            push_bool_flag(&mut argv, "--lib", should_select_lib);
            push_bool_flag(&mut argv, "--all-locales", should_use_all_locales);
            push_bool_flag(&mut argv, "--cache", should_cache);
            push_bool_flag(&mut argv, "--no-build", should_skip_build);
            push_bool_flag(&mut argv, "--man-split-subcommands", should_split_man_subcommands);
            push_optional_bool_arg(
                &mut argv,
                "--ps-split-subcommands",
                should_split_ps_subcommands,
            );
            push_optional_bool_arg(
                &mut argv,
                "--ps-include-common-parameters",
                should_include_common_parameters,
            );
            for locale in &locales {
                argv.push("--locale".to_owned());
                argv.push(locale.clone());
            }

            let cli = Cli::try_parse_from(argv)?;
            let CargoSubcommand::Orthohelp(args) = cli.command;

            prop_assert_eq!(args.package, package);
            prop_assert_eq!(args.bin, bin);
            prop_assert_eq!(args.is_lib, should_select_lib);
            prop_assert_eq!(args.root_type, root_type);
            prop_assert_eq!(args.locale, locales);
            prop_assert_eq!(args.should_use_all_locales, should_use_all_locales);
            prop_assert_eq!(args.cache.should_cache, should_cache);
            prop_assert_eq!(args.cache.should_skip_build, should_skip_build);
            prop_assert_eq!(args.man.section, man_section);
            prop_assert_eq!(
                args.man.should_split_subcommands,
                should_split_man_subcommands
            );
            prop_assert_eq!(
                args.powershell.should_split_subcommands,
                should_split_ps_subcommands
            );
            prop_assert_eq!(
                args.powershell.should_include_common_parameters,
                should_include_common_parameters
            );
            prop_assert_eq!(args.powershell.should_ensure_en_us, should_ensure_en_us);
            prop_assert!(matches!(
                (format, args.format),
                ("ir", OutputFormat::Ir)
                    | ("man", OutputFormat::Man)
                    | ("ps", OutputFormat::Ps)
                    | ("all", OutputFormat::All)
                    | ("agent-context", OutputFormat::AgentContext)
            ));
        }
    }

    fn push_optional_arg(argv: &mut Vec<String>, flag: &str, maybe_value: Option<&str>) {
        if let Some(value) = maybe_value {
            argv.push(flag.to_owned());
            argv.push(value.to_owned());
        }
    }

    fn push_optional_bool_arg(argv: &mut Vec<String>, flag: &str, maybe_value: Option<bool>) {
        if let Some(value) = maybe_value {
            argv.push(flag.to_owned());
            argv.push(value.to_string());
        }
    }

    fn push_bool_flag(argv: &mut Vec<String>, flag: &str, is_enabled: bool) {
        if is_enabled {
            argv.push(flag.to_owned());
        }
    }

    fn collect_reserved_agent_context_commands(
        command: &clap::Command,
        path: &mut Vec<String>,
        violations: &mut Vec<String>,
    ) {
        for subcommand in command.get_subcommands() {
            path.push(subcommand.get_name().to_owned());
            let display_path = path.join(" ");

            record_reserved_subcommand_name(subcommand, &display_path, violations);
            record_reserved_aliases(subcommand, &display_path, violations);
            collect_reserved_agent_context_commands(subcommand, path, violations);
            path.pop();
        }
    }

    fn record_reserved_subcommand_name(
        subcommand: &clap::Command,
        display_path: &str,
        violations: &mut Vec<String>,
    ) {
        if is_reserved_agent_context_command(subcommand.get_name()) {
            violations.push(format!("subcommand `{display_path}`"));
        }
    }

    fn record_reserved_aliases(
        subcommand: &clap::Command,
        display_path: &str,
        violations: &mut Vec<String>,
    ) {
        for alias in subcommand
            .get_all_aliases()
            .filter(|alias| is_reserved_agent_context_command(alias))
        {
            violations.push(format!("alias `{alias}` on `{display_path}`"));
        }
    }

    fn is_reserved_agent_context_command(candidate: &str) -> bool {
        matches!(
            candidate,
            AGENT_CONTEXT_COMMAND | RESERVED_AGENT_CONTEXT_ALIAS
        )
    }
}
