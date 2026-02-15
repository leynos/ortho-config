//! Coverage for typed collection defaults with `cli_default_as_absent`.

use anyhow::{Context, Result, ensure};
use cap_std::{ambient_authority, fs::Dir};
use clap::{CommandFactory, FromArgMatches, Parser};
use ortho_config::subcommand::Prefix;
use ortho_config::{OrthoConfig, load_and_merge_subcommand_with_matches};
use rstest::{fixture, rstest};
use serde::{Deserialize, Serialize};
use serial_test::serial;
use tempfile::TempDir;
use test_helpers::cwd;

#[fixture]
fn prefix() -> Prefix {
    Prefix::new("APP_")
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

/// Verifies typed clap list defaults (`default_values_t`) are inferred and
/// treated as absent.
#[derive(Debug, Parser, Serialize, Deserialize, OrthoConfig, PartialEq)]
#[command(name = "tags")]
#[ortho_config(prefix = "APP_")]
struct TagsArgs {
    #[arg(long, default_values_t = [String::from("alpha"), String::from("beta")])]
    #[ortho_config(cli_default_as_absent)]
    tags: Vec<String>,
}

impl Default for TagsArgs {
    fn default() -> Self {
        Self {
            tags: vec![String::from("alpha"), String::from("beta")],
        }
    }
}

#[rstest]
#[serial]
fn test_cli_default_as_absent_infers_default_values_t(prefix: Prefix) -> Result<()> {
    let (_temp_dir, _cwd_guard) = config_dir("[cmds.tags]\ntags = [\"file\"]\n")?;

    let matches = TagsArgs::command().get_matches_from(["tags"]);
    let args = TagsArgs::from_arg_matches(&matches).context("parse tags args")?;
    let merged = load_and_merge_subcommand_with_matches(&prefix, &args, &matches)
        .context("merge tags args without explicit CLI value")?;
    ensure!(
        merged.tags == vec!["file"],
        "expected file tags, got {:?}",
        merged.tags
    );

    let explicit_matches = TagsArgs::command().get_matches_from(["tags", "--tags", "cli"]);
    let explicit_args =
        TagsArgs::from_arg_matches(&explicit_matches).context("parse explicit tags args")?;
    let explicit_merged =
        load_and_merge_subcommand_with_matches(&prefix, &explicit_args, &explicit_matches)
            .context("merge tags args with explicit CLI value")?;
    ensure!(
        explicit_merged.tags == vec!["cli"],
        "expected cli tags, got {:?}",
        explicit_merged.tags
    );

    Ok(())
}

/// Verifies numeric typed defaults preserve literal inference when inferred from
/// clap's `default_value_t`.
#[derive(Debug, Parser, Serialize, Deserialize, OrthoConfig, PartialEq)]
#[command(name = "retry")]
#[ortho_config(prefix = "APP_")]
struct RetryArgs {
    #[arg(long, default_value_t = 8)]
    #[ortho_config(cli_default_as_absent)]
    count: u32,
}

impl Default for RetryArgs {
    fn default() -> Self {
        Self { count: 8 }
    }
}

#[rstest]
#[serial]
fn test_cli_default_as_absent_infers_numeric_default_value_t(prefix: Prefix) -> Result<()> {
    let (_temp_dir, _cwd_guard) = config_dir("[cmds.retry]\ncount = 5\n")?;

    let matches = RetryArgs::command().get_matches_from(["retry"]);
    let args = RetryArgs::from_arg_matches(&matches).context("parse retry args")?;
    let merged = load_and_merge_subcommand_with_matches(&prefix, &args, &matches)
        .context("merge retry args without explicit CLI value")?;
    ensure!(
        merged.count == 5,
        "expected file count, got {}",
        merged.count
    );

    let explicit_matches = RetryArgs::command().get_matches_from(["retry", "--count", "9"]);
    let explicit_args =
        RetryArgs::from_arg_matches(&explicit_matches).context("parse explicit retry args")?;
    let explicit_merged =
        load_and_merge_subcommand_with_matches(&prefix, &explicit_args, &explicit_matches)
            .context("merge retry args with explicit CLI value")?;
    ensure!(
        explicit_merged.count == 9,
        "expected cli count, got {}",
        explicit_merged.count
    );

    Ok(())
}
