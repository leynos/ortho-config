//! Parser tests for Cargo external-subcommand dispatch.

use clap::{CommandFactory, Parser, error::ErrorKind};
use proptest::prelude::*;
use rstest::rstest;

use super::{CargoSubcommand, Cli, OutputFormat};

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
