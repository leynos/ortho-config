//! Tests for the `cli_default_as_absent` field attribute.
//!
//! This attribute allows non-`Option` fields with clap defaults to be treated as
//! absent when the user did not provide a value on the command line. This enables
//! file and environment configuration to take precedence over clap defaults while
//! still honouring explicit CLI overrides.
//!
//! Precedence (lowest -> highest): struct defaults < file < environment < CLI.
//! Fields marked with `cli_default_as_absent` are only included in the CLI layer
//! when `value_source()` returns `CommandLine`.

#[path = "support/default_punct.rs"]
mod default_punct;

use anyhow::{Context, Result, ensure};
use cap_std::{ambient_authority, fs::Dir};
use clap::{CommandFactory, FromArgMatches, Parser, ValueEnum};
use ortho_config::subcommand::Prefix;
use ortho_config::{
    CliValueExtractor, OrthoConfig, OrthoError, load_and_merge_subcommand,
    load_and_merge_subcommand_with_matches,
};
use rstest::{fixture, rstest};
use serde::{Deserialize, Serialize};
use serial_test::serial;
use tempfile::TempDir;
use test_helpers::{cwd, env};

/// Subcommand with `cli_default_as_absent` attribute on a non-Option field.
#[derive(Debug, Parser, Serialize, Deserialize, OrthoConfig, PartialEq)]
#[command(name = "greet")]
#[ortho_config(prefix = "APP_")]
struct GreetArgs {
    /// Punctuation at the end of the greeting.
    #[arg(
        long,
        id = "punctuation",
        default_value_t = default_punct::default_punct()
    )]
    #[ortho_config(cli_default_as_absent)]
    punctuation: String,

    /// Regular Option field for comparison.
    #[arg(long)]
    name: Option<String>,
}

impl Default for GreetArgs {
    fn default() -> Self {
        Self {
            punctuation: default_punct::default_punct(),
            name: None,
        }
    }
}

#[fixture]
fn config_dir(#[default("")] cfg: &str) -> Result<(TempDir, cwd::CwdGuard)> {
    let dir = tempfile::tempdir().context("create temp dir")?;
    let cap = Dir::open_ambient_dir(dir.path(), ambient_authority()).context("open temp dir")?;
    cap.write(".app.toml", cfg.as_bytes())
        .context("write config")?;
    let guard = cwd::set_dir(dir.path())?;
    Ok((dir, guard))
}

struct GreetPrecedenceCase {
    config_content: &'static str,
    env_val: Option<&'static str>,
    cli_args: Vec<&'static str>,
    expected_punctuation: &'static str,
}

/// Test that `cli_default_as_absent` allows file values to override clap defaults.
#[rstest]
#[case::clap_default_used_when_no_other_sources(
    GreetPrecedenceCase {
        config_content: "",
        env_val: None,
        cli_args: vec!["greet"],
        expected_punctuation: "!",
    },
)]
#[case::file_overrides_clap_default(
    GreetPrecedenceCase {
        config_content: "[cmds.greet]\npunctuation = \"?\"\n",
        env_val: None,
        cli_args: vec!["greet"],
        expected_punctuation: "?",
    },
)]
#[case::env_overrides_clap_default(
    GreetPrecedenceCase {
        config_content: "",
        env_val: Some("..."),
        cli_args: vec!["greet"],
        expected_punctuation: "...",
    },
)]
#[case::env_overrides_file(
    GreetPrecedenceCase {
        config_content: "[cmds.greet]\npunctuation = \"?\"\n",
        env_val: Some("..."),
        cli_args: vec!["greet"],
        expected_punctuation: "...",
    },
)]
#[case::explicit_cli_overrides_all(
    GreetPrecedenceCase {
        config_content: "[cmds.greet]\npunctuation = \"?\"\n",
        env_val: Some("..."),
        cli_args: vec!["greet", "--punctuation", "!!!"],
        expected_punctuation: "!!!",
    },
)]
#[serial]
fn test_cli_default_as_absent_precedence(#[case] case: GreetPrecedenceCase) -> Result<()> {
    let (_temp_dir, _cwd_guard) = config_dir(case.config_content)?;
    let _guard = case
        .env_val
        .map(|val| env::set_var("APP_CMDS_GREET_PUNCTUATION", val));

    let punctuation_on_cli = case.cli_args.contains(&"--punctuation");
    let matches = GreetArgs::command().get_matches_from(case.cli_args);
    let punctuation_source = matches.value_source("punctuation");
    if punctuation_on_cli {
        ensure!(
            punctuation_source == Some(clap::parser::ValueSource::CommandLine),
            "expected punctuation source=CommandLine when --punctuation is present, got {punctuation_source:?}",
        );
    } else {
        ensure!(
            punctuation_source != Some(clap::parser::ValueSource::CommandLine),
            "expected punctuation source != CommandLine when --punctuation is absent, got {punctuation_source:?}",
        );
    }
    let args = GreetArgs::from_arg_matches(&matches).context("parse CLI args")?;
    let prefix = Prefix::new("APP_");
    let merged = load_and_merge_subcommand_with_matches(&prefix, &args, &matches)
        .context("merge greet args")?;

    ensure!(
        merged.punctuation == case.expected_punctuation,
        "expected punctuation {:?}, got {:?}",
        case.expected_punctuation,
        merged.punctuation
    );
    Ok(())
}

/// Parser used to prove inferred string defaults reuse a field's clap parser.
fn parse_tcp_port(value: &str) -> Result<u16, String> {
    value
        .strip_prefix("tcp:")
        .ok_or_else(|| String::from("expected a tcp: port"))?
        .parse::<u16>()
        .map_err(|error| error.to_string())
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, ValueEnum)]
#[serde(rename_all = "kebab-case")]
enum Mode {
    Fast,
    Safe,
}

/// Subcommand that exercises parser-faithful string default inference.
#[derive(Debug, Parser, Serialize, Deserialize, OrthoConfig, PartialEq)]
#[command(name = "default-parity")]
#[ortho_config(prefix = "APP_")]
struct DefaultParityArgs {
    #[arg(long, default_value = "8")]
    #[ortho_config(cli_default_as_absent)]
    count: u16,

    #[arg(long, default_value = "FAST", value_enum, ignore_case = true)]
    #[ortho_config(cli_default_as_absent)]
    mode: Mode,

    #[arg(long, default_value = "tcp:7", value_parser = parse_tcp_port)]
    #[ortho_config(cli_default_as_absent)]
    port: u16,

    #[arg(long, default_value = "default")]
    #[ortho_config(cli_default_as_absent)]
    label: Option<String>,
}

impl Default for DefaultParityArgs {
    fn default() -> Self {
        Self {
            count: 8,
            mode: Mode::Fast,
            port: 7,
            label: Some(String::from("default")),
        }
    }
}

#[rstest]
#[serial]
fn inferred_default_value_preserves_clap_parsers() -> Result<()> {
    {
        let no_file_dir = tempfile::tempdir().context("create no-file config dir")?;
        let _cwd_guard = cwd::set_dir(no_file_dir.path())?;
        let inferred = DefaultParityArgs::load_from_iter(["default-parity"])
            .context("load inferred clap defaults")?;
        ensure!(
            inferred.count == 8,
            "expected inferred count, got {}",
            inferred.count
        );
        ensure!(
            inferred.mode == Mode::Fast,
            "expected inferred fast mode, got {:?}",
            inferred.mode
        );
        ensure!(
            inferred.port == 7,
            "expected inferred port, got {}",
            inferred.port
        );
        ensure!(
            inferred.label.as_deref() == Some("default"),
            "expected inferred label, got {:?}",
            inferred.label,
        );
    }

    let (_temp_dir, _cwd_guard) = config_dir(
        "[cmds.default-parity]\ncount = 5\nmode = \"safe\"\nport = 6\nlabel = \"file\"\n",
    )?;
    let prefix = Prefix::new("APP_");

    let matches = DefaultParityArgs::command().get_matches_from(["default-parity"]);
    let args = DefaultParityArgs::from_arg_matches(&matches).context("parse defaults")?;
    let merged = load_and_merge_subcommand_with_matches(&prefix, &args, &matches)
        .context("merge parser-faithful defaults")?;
    ensure!(
        merged.count == 5,
        "expected file count, got {}",
        merged.count
    );
    ensure!(
        merged.mode == Mode::Safe,
        "expected file mode, got {:?}",
        merged.mode
    );
    ensure!(merged.port == 6, "expected file port, got {}", merged.port);
    ensure!(
        merged.label.as_deref() == Some("file"),
        "expected file label, got {:?}",
        merged.label,
    );

    let explicit_matches = DefaultParityArgs::command().get_matches_from([
        "default-parity",
        "--count",
        "9",
        "--mode",
        "fast",
        "--port",
        "tcp:10",
        "--label",
        "cli",
    ]);
    let explicit_args =
        DefaultParityArgs::from_arg_matches(&explicit_matches).context("parse explicit values")?;
    let explicit =
        load_and_merge_subcommand_with_matches(&prefix, &explicit_args, &explicit_matches)
            .context("merge explicit values")?;
    ensure!(
        explicit.count == 9,
        "expected cli count, got {}",
        explicit.count
    );
    ensure!(
        explicit.mode == Mode::Fast,
        "expected cli mode, got {:?}",
        explicit.mode
    );
    ensure!(
        explicit.port == 10,
        "expected cli port, got {}",
        explicit.port
    );
    ensure!(
        explicit.label.as_deref() == Some("cli"),
        "expected cli label, got {:?}",
        explicit.label,
    );
    Ok(())
}

/// Explicit `OrthoConfig` defaults continue to override inferred clap defaults.
#[derive(Debug, Parser, Serialize, Deserialize, OrthoConfig, PartialEq)]
#[command(name = "explicit-default")]
#[ortho_config(prefix = "APP_")]
struct ExplicitDefaultArgs {
    #[arg(long, default_value = "8")]
    #[ortho_config(default = 11, cli_default_as_absent)]
    count: u16,
}

impl Default for ExplicitDefaultArgs {
    fn default() -> Self {
        Self { count: 11 }
    }
}

#[rstest]
#[serial]
fn explicit_ortho_default_overrides_inferred_default_value() -> Result<()> {
    let (_temp_dir, _cwd_guard) = config_dir("")?;
    let prefix = Prefix::new("APP_");
    let matches = ExplicitDefaultArgs::command().get_matches_from(["explicit-default"]);
    let args = ExplicitDefaultArgs::from_arg_matches(&matches).context("parse defaults")?;
    let merged = load_and_merge_subcommand_with_matches(&prefix, &args, &matches)
        .context("merge explicit OrthoConfig default")?;
    ensure!(
        merged.count == 11,
        "expected explicit default, got {}",
        merged.count
    );
    Ok(())
}

/// Configuration with a clap default that its field parser rejects.
#[derive(Debug, Parser, Serialize, Deserialize, OrthoConfig, PartialEq)]
#[command(name = "invalid-default")]
#[ortho_config(prefix = "APP_")]
struct InvalidDefaultArgs {
    #[arg(long, default_value = "udp:7", value_parser = parse_tcp_port)]
    #[ortho_config(cli_default_as_absent)]
    port: u16,
}

impl Default for InvalidDefaultArgs {
    fn default() -> Self {
        Self { port: 7 }
    }
}

fn contains_default_value_conversion(error: &OrthoError) -> bool {
    match error {
        OrthoError::DefaultValueConversion { .. } => true,
        OrthoError::Aggregate(errors) => errors
            .iter()
            .any(|entry| matches!(entry, OrthoError::DefaultValueConversion { .. })),
        _ => false,
    }
}

#[rstest]
#[serial]
fn invalid_inferred_default_is_reported_without_panicking() -> Result<()> {
    let (_temp_dir, _cwd_guard) = config_dir("")?;
    let error = InvalidDefaultArgs::load_from_iter(["invalid-default"])
        .err()
        .ok_or_else(|| anyhow::anyhow!("expected invalid default to fail"))?;
    ensure!(
        contains_default_value_conversion(error.as_ref()),
        "expected DefaultValueConversion, got {error:?}",
    );
    Ok(())
}

/// Test that `load_and_merge_subcommand` keeps clap defaults over file configuration.
#[test]
#[serial]
fn test_load_and_merge_subcommand_keeps_clap_default() -> Result<()> {
    let (_temp_dir, _cwd_guard) = config_dir("[cmds.greet]\npunctuation = \"?\"\n")?;

    let matches = GreetArgs::command().get_matches_from(["greet"]);
    let args = GreetArgs::from_arg_matches(&matches).context("parse greet args")?;
    let prefix = Prefix::new("APP_");
    let merged = load_and_merge_subcommand(&prefix, &args).context("merge greet args")?;

    ensure!(
        merged.punctuation == default_punct::default_punct(),
        "expected punctuation {:?}, got {:?}",
        default_punct::default_punct(),
        merged.punctuation
    );
    Ok(())
}

/// Verify that `extract_user_provided` uses custom clap argument IDs.
#[derive(Debug, Parser, Serialize, Deserialize, OrthoConfig, PartialEq)]
#[command(name = "custom-id")]
#[ortho_config(prefix = "APP_")]
struct CustomIdArgs {
    #[arg(
        long,
        id = "custom_punctuation",
        default_value_t = default_punct::default_punct()
    )]
    #[ortho_config(cli_default_as_absent)]
    punctuation: String,
}

impl Default for CustomIdArgs {
    fn default() -> Self {
        Self {
            punctuation: default_punct::default_punct(),
        }
    }
}

/// Parse `cli_args` into `T` and ensure `extract_user_provided` contains `expected_key`.
fn check_extraction_key<T>(cli_args: &[&str], expected_key: &str) -> Result<()>
where
    T: CommandFactory + FromArgMatches + CliValueExtractor,
{
    let args_iter = cli_args.iter().copied();
    let matches = T::command().get_matches_from(args_iter);
    let args = T::from_arg_matches(&matches).context("parse CLI args")?;
    let extracted = args
        .extract_user_provided(&matches)
        .context("extract user provided")?;

    ensure!(
        extracted.get(expected_key).is_some(),
        "expected {expected_key} to be present, but it was absent: {extracted:?}",
    );

    Ok(())
}

#[test]
fn test_extract_user_provided_excludes_clap_default_with_custom_id() -> Result<()> {
    let matches = CustomIdArgs::command().get_matches_from(["custom-id"]);
    let args = CustomIdArgs::from_arg_matches(&matches).context("parse CLI args")?;
    let extracted = args
        .extract_user_provided(&matches)
        .context("extract user provided")?;

    ensure!(
        extracted.get("punctuation").is_none(),
        "expected punctuation to be absent when using clap default, got: {extracted:?}",
    );

    Ok(())
}

#[test]
fn test_extract_user_provided_respects_custom_arg_id() -> Result<()> {
    check_extraction_key::<CustomIdArgs>(&["custom-id", "--punctuation", "?"], "punctuation")
}

/// Verify that extraction keys follow serde renaming rules.
#[derive(Debug, Parser, Serialize, Deserialize, OrthoConfig, PartialEq)]
#[command(name = "rename-all")]
#[ortho_config(prefix = "APP_")]
#[serde(rename_all = "kebab-case")]
struct RenameAllArgs {
    #[arg(long, default_value_t = default_punct::default_punct())]
    #[ortho_config(cli_default_as_absent)]
    verbose_mode: String,
}

impl Default for RenameAllArgs {
    fn default() -> Self {
        Self {
            verbose_mode: default_punct::default_punct(),
        }
    }
}

#[test]
fn test_extract_user_provided_respects_serde_rename_all() -> Result<()> {
    check_extraction_key::<RenameAllArgs>(&["rename-all", "--verbose-mode", "?"], "verbose-mode")
}

/// Test that `extract_user_provided` excludes fields with clap defaults.
#[rstest]
#[case::clap_default_excluded(
    vec!["greet"],
    false, // expect punctuation to be absent
)]
#[case::explicit_cli_included(
    vec!["greet", "--punctuation", "!!!"],
    true, // expect punctuation to be present
)]
fn test_extract_user_provided_respects_value_source(
    #[case] cli_args: Vec<&str>,
    #[case] expect_punctuation: bool,
) -> Result<()> {
    let matches = GreetArgs::command().get_matches_from(cli_args);
    let args = GreetArgs::from_arg_matches(&matches).context("parse CLI args")?;
    let extracted = args
        .extract_user_provided(&matches)
        .context("extract user provided")?;

    let has_punctuation = extracted.get("punctuation").is_some();
    ensure!(
        has_punctuation == expect_punctuation,
        "expected punctuation present={expect_punctuation}, got present={has_punctuation}",
    );
    Ok(())
}

/// Verify that name (an Option field) is handled correctly alongside `cli_default_as_absent`.
#[rstest]
#[case::both_absent(
    vec!["greet"],
    None,
    false, // punctuation absent
)]
#[case::name_provided(
    vec!["greet", "--name", "World"],
    Some("World"),
    false,
)]
#[case::both_provided(
    vec!["greet", "--name", "World", "--punctuation", "?"],
    Some("World"),
    true, // punctuation present
)]
fn test_extract_user_provided_with_mixed_fields(
    #[case] cli_args: Vec<&str>,
    #[case] expected_name: Option<&str>,
    #[case] expect_punctuation: bool,
) -> Result<()> {
    let matches = GreetArgs::command().get_matches_from(cli_args);
    let args = GreetArgs::from_arg_matches(&matches).context("parse CLI args")?;
    let extracted = args
        .extract_user_provided(&matches)
        .context("extract user provided")?;

    let has_punctuation = extracted.get("punctuation").is_some();
    ensure!(
        has_punctuation == expect_punctuation,
        "expected punctuation present={expect_punctuation}, got present={has_punctuation}",
    );

    let extracted_name = extracted.get("name").and_then(|v| v.as_str());
    ensure!(
        extracted_name == expected_name,
        "expected name {expected_name:?}, got {extracted_name:?}",
    );

    Ok(())
}
