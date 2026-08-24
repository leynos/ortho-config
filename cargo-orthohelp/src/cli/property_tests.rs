//! Property tests for `cargo-orthohelp` command-line parsing.
//!
//! These cases exercise combinations of optional values and boolean flags so
//! CLI additions retain the parser's established input contract.

use cargo_orthohelp::policy::PolicyMode;
use clap::Parser;
use proptest::prelude::*;

use super::{CargoSubcommand, Cli, OutputFormat, PolicyModeArg};

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
        should_check_agent_native in any::<bool>(),
        policy_mode in prop::sample::select(vec![
            PolicyModeArg::Off,
            PolicyModeArg::Warn,
            PolicyModeArg::Deny,
        ]),
        format in prop::sample::select(vec!["ir", "man", "ps", "all", "agent-context"]),
        man_section in 1_u8..=8,
        should_split_ps_subcommands in prop::option::of(any::<bool>()),
        should_include_common_parameters in prop::option::of(any::<bool>()),
        should_ensure_en_us in any::<bool>(),
    ) {
        let policy_mode_value = match policy_mode {
            PolicyModeArg::Off => "off",
            PolicyModeArg::Warn => "warn",
            PolicyModeArg::Deny => "deny",
        };
        let mut argv = vec![
            "cargo-orthohelp".to_owned(),
            "orthohelp".to_owned(),
            "--format".to_owned(),
            format.to_owned(),
            "--policy-mode".to_owned(),
            policy_mode_value.to_owned(),
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
        push_bool_flag(&mut argv, "--check-agent-native", should_check_agent_native);
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
        prop_assert_eq!(args.should_check_agent_native, should_check_agent_native);
        prop_assert_eq!(PolicyMode::from(args.policy_mode), PolicyMode::from(policy_mode));
        prop_assert_eq!(args.cache.should_skip_build, should_skip_build);
        prop_assert_eq!(args.man.section, man_section);
        prop_assert_eq!(args.man.should_split_subcommands, should_split_man_subcommands);
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
