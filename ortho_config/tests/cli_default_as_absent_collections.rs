//! Coverage for typed collection defaults with `cli_default_as_absent`.

use anyhow::{Context, Result, anyhow, ensure};
use camino::Utf8PathBuf;
use cap_std::{ambient_authority, fs::Dir};
use clap::{CommandFactory, FromArgMatches, Parser};
use ortho_config::subcommand::Prefix;
use ortho_config::{OrthoConfig, load_and_merge_subcommand_with_matches};
use serde::{Deserialize, Serialize};
use serial_test::serial;
use std::sync::{LazyLock, Mutex, MutexGuard};
use tempfile::TempDir;

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

fn config_dir(cfg: &str) -> Result<(TempDir, DirGuard)> {
    let dir = tempfile::tempdir().context("create temp dir")?;
    let cap = Dir::open_ambient_dir(dir.path(), ambient_authority()).context("open temp dir")?;
    cap.write(".app.toml", cfg.as_bytes())
        .context("write config")?;
    let guard = set_dir(&dir)?;
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

#[test]
#[serial]
fn test_cli_default_as_absent_infers_default_values_t() -> Result<()> {
    let (_temp_dir, _cwd_guard) = config_dir("[cmds.tags]\ntags = [\"file\"]\n")?;
    let prefix = Prefix::new("APP_");

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

#[test]
#[serial]
fn test_cli_default_as_absent_infers_numeric_default_value_t() -> Result<()> {
    let (_temp_dir, _cwd_guard) = config_dir("[cmds.retry]\ncount = 5\n")?;
    let prefix = Prefix::new("APP_");

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
