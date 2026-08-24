//! Coverage for typed collection defaults with `cli_default_as_absent`.

use anyhow::{Context, Result, ensure};
use cap_std::{ambient_authority, fs::Dir};
use clap::Parser;
use ortho_config::subcommand::Prefix;
use ortho_config::{CliValueExtractor, OrthoConfig, load_and_merge_subcommand_with_matches};
use rstest::{fixture, rstest};
use serde::de::DeserializeOwned;
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

/// Merge a subcommand without and with an explicit CLI value.
fn merge_default_and_explicit<T>(
    prefix: &Prefix,
    default_args: &[&str],
    explicit_args: &[&str],
) -> Result<(T, T)>
where
    T: Parser + Serialize + DeserializeOwned + Default + CliValueExtractor,
{
    let matches = T::command().get_matches_from(default_args.iter().copied());
    let args = T::from_arg_matches(&matches).context("parse clap defaults")?;
    let merged = load_and_merge_subcommand_with_matches(prefix, &args, &matches)
        .context("merge clap defaults")?;

    let explicit_matches = T::command().get_matches_from(explicit_args.iter().copied());
    let explicit_cli =
        T::from_arg_matches(&explicit_matches).context("parse explicit CLI values")?;
    let explicit = load_and_merge_subcommand_with_matches(prefix, &explicit_cli, &explicit_matches)
        .context("merge explicit CLI values")?;

    Ok((merged, explicit))
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

/// Verifies string clap list defaults are parsed through clap and treated as
/// absent during layered merging.
#[derive(Debug, Parser, Serialize, Deserialize, OrthoConfig, PartialEq)]
#[command(name = "string-tags")]
#[ortho_config(prefix = "APP_")]
struct StringTagsArgs {
    #[arg(long, default_value = "alpha")]
    #[ortho_config(cli_default_as_absent)]
    tags: Vec<String>,
}

impl Default for StringTagsArgs {
    fn default() -> Self {
        Self {
            tags: vec![String::from("alpha")],
        }
    }
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

/// Verifies typed and string collection defaults preserve merge precedence.
#[rstest]
#[serial]
fn test_cli_default_as_absent_collection_defaults(prefix: Prefix) -> Result<()> {
    {
        let (_temp_dir, _cwd_guard) = config_dir("[cmds.tags]\ntags = [\"file\"]\n")?;
        let (merged, explicit) =
            merge_default_and_explicit::<TagsArgs>(&prefix, &["tags"], &["tags", "--tags", "cli"])?;
        ensure!(merged.tags == vec!["file"]);
        ensure!(explicit.tags == vec!["cli"]);
    }

    {
        let (_temp_dir, _cwd_guard) = config_dir("[cmds.string-tags]\ntags = [\"file\"]\n")?;
        let (merged, explicit) = merge_default_and_explicit::<StringTagsArgs>(
            &prefix,
            &["string-tags"],
            &["string-tags", "--tags", "cli"],
        )?;
        ensure!(merged.tags == vec!["file"]);
        ensure!(explicit.tags == vec!["cli"]);
    }

    {
        let (_temp_dir, _cwd_guard) = config_dir("[cmds.retry]\ncount = 5\n")?;
        let (merged, explicit) = merge_default_and_explicit::<RetryArgs>(
            &prefix,
            &["retry"],
            &["retry", "--count", "9"],
        )?;
        ensure!(merged.count == 5);
        ensure!(explicit.count == 9);
    }

    Ok(())
}
