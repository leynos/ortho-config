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

use anyhow::{Context, Result, anyhow, ensure};
use camino::Utf8PathBuf;
use cap_std::{ambient_authority, fs::Dir};
use clap::{CommandFactory, FromArgMatches, Parser};
use ortho_config::subcommand::Prefix;
use ortho_config::{
    CliValueExtractor, OrthoConfig, load_and_merge_subcommand,
    load_and_merge_subcommand_with_matches,
};
use rstest::{fixture, rstest};
use serde::{Deserialize, Serialize};
use serial_test::serial;
use std::sync::{LazyLock, Mutex, MutexGuard};
use tempfile::TempDir;
use test_helpers::env;

/// Default punctuation used when neither file nor CLI provides a value.
fn default_punct() -> String {
    "!".into()
}

/// Subcommand with `cli_default_as_absent` attribute on a non-Option field.
#[derive(Debug, Parser, Serialize, Deserialize, OrthoConfig, PartialEq)]
#[command(name = "greet")]
#[ortho_config(prefix = "APP_")]
struct GreetArgs {
    /// Punctuation at the end of the greeting.
    #[arg(long, id = "punctuation", default_value_t = default_punct())]
    #[ortho_config(default = default_punct(), cli_default_as_absent)]
    punctuation: String,

    /// Regular Option field for comparison.
    #[arg(long)]
    name: Option<String>,
}

impl Default for GreetArgs {
    fn default() -> Self {
        Self {
            punctuation: default_punct(),
            name: None,
        }
    }
}

static CWD_MUTEX: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));

struct DirGuard {
    old: Utf8PathBuf,
    _lock: MutexGuard<'static, ()>,
}

fn set_dir(dir: &TempDir) -> Result<DirGuard> {
    let lock = CWD_MUTEX
        .lock()
        .map_err(|err| anyhow!("lock current dir mutex: {err}"))?;
    let old = std::env::current_dir().context("read current dir")?;
    std::env::set_current_dir(dir.path()).context("set current dir")?;
    let old_utf8 = Utf8PathBuf::from_path_buf(old)
        .map_err(|path| anyhow!("cwd is not valid UTF-8: {}", path.display()))?;
    Ok(DirGuard {
        old: old_utf8,
        _lock: lock,
    })
}

impl Drop for DirGuard {
    fn drop(&mut self) {
        // PANIC: Drop cannot return Result; failing to restore the cwd would
        // leave the test environment in a broken state, so we panic to fail
        // fast.
        if let Err(err) = std::env::set_current_dir(&self.old) {
            panic!("restore current dir: {err}");
        }
    }
}

#[fixture]
fn config_dir(#[default("")] cfg: &str) -> Result<(TempDir, DirGuard)> {
    let dir = tempfile::tempdir().context("create temp dir")?;
    let cap = Dir::open_ambient_dir(dir.path(), ambient_authority()).context("open temp dir")?;
    cap.write(".app.toml", cfg.as_bytes())
        .context("write config")?;
    let guard = set_dir(&dir)?;
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

    let matches = GreetArgs::command().get_matches_from(case.cli_args);
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
        merged.punctuation == default_punct(),
        "expected punctuation {:?}, got {:?}",
        default_punct(),
        merged.punctuation
    );
    Ok(())
}

/// Verify that `extract_user_provided` uses custom clap argument IDs.
#[derive(Debug, Parser, Serialize, Deserialize, OrthoConfig, PartialEq)]
#[command(name = "custom-id")]
#[ortho_config(prefix = "APP_")]
struct CustomIdArgs {
    #[arg(long, id = "custom_punctuation", default_value_t = default_punct())]
    #[ortho_config(default = default_punct(), cli_default_as_absent)]
    punctuation: String,
}

impl Default for CustomIdArgs {
    fn default() -> Self {
        Self {
            punctuation: default_punct(),
        }
    }
}

#[test]
fn test_extract_user_provided_respects_custom_arg_id() -> Result<()> {
    let matches = CustomIdArgs::command().get_matches_from(["custom-id", "--punctuation", "?"]);
    let args = CustomIdArgs::from_arg_matches(&matches).context("parse CLI args")?;
    let extracted = args
        .extract_user_provided(&matches)
        .context("extract user provided")?;

    ensure!(
        extracted.get("punctuation").is_some(),
        "expected punctuation to be present, but it was absent: {extracted:?}"
    );

    Ok(())
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
#[serial]
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
#[serial]
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
