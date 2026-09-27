//! Behavioural coverage for the `SubcmdConfigMerge` method family.
//!
//! Every case drives one public trait method and stages its sources so that a
//! mis-wired forwarding fails rather than passing by accident: an injected
//! environment value must beat a file value, an explicit CLI value must beat
//! the injected environment, and a field marked
//! `#[ortho_config(cli_default_as_absent)]` must stay absent when clap only
//! supplied its own default.
//!
//! The methods resolve configuration files relative to the working directory,
//! so each case enters a temporary directory of its own instead of depending on
//! the ambient one. No case mutates the process environment.

use anyhow::{Context, Result, ensure};
use cap_std::{ambient_authority, fs::Dir};
use clap::{CommandFactory, FromArgMatches, Parser};
use ortho_config::{MapEnv, OrthoConfig, SharedScanEnvSource, SubcmdConfigMerge};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tempfile::TempDir;
use test_helpers::cwd;

/// Subcommand used by the plain injected-source cases.
#[derive(Debug, Parser, Serialize, Deserialize, OrthoConfig, Default, PartialEq)]
#[command(name = "pr")]
#[ortho_config(prefix = "VK_")]
struct PrArgs {
    #[arg(long)]
    reference: Option<String>,
}

/// Subcommand whose clap default must not reach the merge unless the user
/// supplies the value explicitly.
#[derive(Debug, Parser, Serialize, Deserialize, OrthoConfig, PartialEq)]
#[command(name = "issue")]
#[ortho_config(prefix = "VK_")]
struct IssueArgs {
    /// The explicit default deliberately differs from the clap default, so a
    /// leaked clap default is observable in the merged result.
    #[arg(long, default_value = "2")]
    #[ortho_config(default = 5, cli_default_as_absent)]
    retries: u32,
}

impl Default for IssueArgs {
    /// Mirrors the `#[ortho_config(default = ...)]` value, not clap's.
    fn default() -> Self {
        Self { retries: 5 }
    }
}

/// Environment key holding the injected reference for [`PrArgs`].
const PR_REFERENCE_KEY: &str = "VK_CMDS_PR_REFERENCE";
/// Injected environment value for `PR_REFERENCE_KEY`.
const INJECTED_REFERENCE: &str = "injected_ref";
/// File value written to the `[cmds.pr] reference` key.
const FILE_REFERENCE: &str = "file_ref";
/// Explicit CLI value passed as `--reference`.
const CLI_REFERENCE: &str = "cli_ref";

/// Environment key holding the injected retry count for [`IssueArgs`].
const ISSUE_RETRIES_KEY: &str = "VK_CMDS_ISSUE_RETRIES";
/// Injected environment value for `ISSUE_RETRIES_KEY`.
const INJECTED_RETRIES: u32 = 7;
/// File value written to the `[cmds.issue] retries` key.
const FILE_RETRIES: u32 = 4;
/// Explicit CLI value passed as `--retries`.
const CLI_RETRIES: u32 = 9;
/// What clap's own `default_value` supplies, which must stay absent.
const CLAP_DEFAULT_RETRIES: u32 = 2;
/// What the explicit `#[ortho_config(default = ...)]` supplies.
const STRUCT_DEFAULT_RETRIES: u32 = 5;

/// Builds a scan source whose only variable is `key = value`.
fn scan_source(key: &str, value: impl std::fmt::Display) -> SharedScanEnvSource {
    Arc::new(MapEnv::new().with_var(key, value.to_string()))
}

/// Fails when `key` is visible in the process environment.
///
/// An exported variable reaches every merge through the process-backed
/// environment layer, so it can supply an expected value whether or not the
/// source under test contributes anything. Requiring the key to be unset keeps
/// each case decisive.
fn ensure_key_is_unset(key: &str) -> Result<()> {
    ensure!(
        std::env::var_os(key).is_none(),
        "{key} must be unset for this case to be meaningful"
    );
    Ok(())
}

/// Writes `cfg` to `.vk.toml` inside a fresh temporary directory.
fn config_dir(cfg: &str) -> Result<TempDir> {
    let dir = tempfile::tempdir().context("create temp dir")?;
    let cap = Dir::open_ambient_dir(dir.path(), ambient_authority()).context("open temp dir")?;
    cap.write(".vk.toml", cfg.as_bytes())
        .context("write config")?;
    Ok(dir)
}

/// Writes `cfg` to `.vk.toml` and enters the directory holding it.
fn config_dir_with_cwd(cfg: &str) -> Result<(TempDir, cwd::CwdGuard)> {
    let dir = config_dir(cfg)?;
    let guard = cwd::set_dir(dir.path()).context("enter temp config dir")?;
    Ok((dir, guard))
}

/// Enters a fresh temporary directory holding no subcommand configuration.
fn empty_dir_with_cwd() -> Result<(TempDir, cwd::CwdGuard)> {
    config_dir_with_cwd("")
}

/// Enters a temporary directory whose only file supplies `FILE_REFERENCE`.
fn pr_config_dir() -> Result<(TempDir, cwd::CwdGuard)> {
    config_dir_with_cwd(&format!("[cmds.pr]\nreference = \"{FILE_REFERENCE}\"\n"))
}

/// Enters a temporary directory whose only file supplies `FILE_RETRIES`.
fn issue_config_dir() -> Result<(TempDir, cwd::CwdGuard)> {
    config_dir_with_cwd(&format!("[cmds.issue]\nretries = {FILE_RETRIES}\n"))
}

/// `SubcmdConfigMerge::load_and_merge_with_sources` merges the supplied scan
/// source as the environment layer, above the file but below the CLI.
///
/// Catches a forwarding that drops the supplied source and reads the process
/// environment instead, which would leave `FILE_REFERENCE` in place. The
/// precondition keeps the case non-vacuous: a `PR_REFERENCE_KEY` exported into
/// the process environment could otherwise satisfy the assertion without the
/// injected source being read at all.
#[test]
fn sources_method_prefers_the_injected_environment_over_the_file() -> Result<()> {
    ensure_key_is_unset(PR_REFERENCE_KEY)?;
    let (_dir, _cwd_guard) = pr_config_dir()?;
    let source = scan_source(PR_REFERENCE_KEY, INJECTED_REFERENCE);

    let cli = PrArgs::default();
    let merged = cli
        .load_and_merge_with_sources(source)
        .context("merge pr args from the injected source")?;

    ensure!(
        merged.reference.as_deref() == Some(INJECTED_REFERENCE),
        "expected {INJECTED_REFERENCE:?} above the file value, got {:?}",
        merged.reference
    );
    Ok(())
}

/// `SubcmdConfigMerge::load_and_merge_with_sources` keeps explicit CLI values
/// above the injected environment.
///
/// Catches a forwarding that omits the CLI overlay, which would leave
/// `INJECTED_REFERENCE` in place instead of `CLI_REFERENCE`.
#[test]
fn sources_method_keeps_cli_values_above_the_injected_environment() -> Result<()> {
    let (_dir, _cwd_guard) = pr_config_dir()?;
    let source = scan_source(PR_REFERENCE_KEY, INJECTED_REFERENCE);
    let matches = PrArgs::command().get_matches_from(["pr", "--reference", CLI_REFERENCE]);
    let cli = PrArgs::from_arg_matches(&matches).context("parse explicit values")?;

    let merged = cli
        .load_and_merge_with_sources(source)
        .context("merge pr args carrying CLI values")?;

    ensure!(
        merged.reference.as_deref() == Some(CLI_REFERENCE),
        "expected the CLI value above the injected source, got {:?}",
        merged.reference
    );
    Ok(())
}

/// `SubcmdConfigMerge::load_and_merge_with_matches` treats a clap default as
/// absent when no match metadata names it explicit.
///
/// Catches a forwarding that ignores `matches` and serialises the parsed values
/// wholesale, which would leak `CLAP_DEFAULT_RETRIES` over the explicit default
/// of `STRUCT_DEFAULT_RETRIES`.
#[test]
fn matches_method_keeps_clap_defaults_absent() -> Result<()> {
    ensure_key_is_unset(ISSUE_RETRIES_KEY)?;
    let (_dir, _cwd_guard) = empty_dir_with_cwd()?;
    let matches = IssueArgs::command().get_matches_from(["issue"]);
    let args = IssueArgs::from_arg_matches(&matches).context("parse clap defaults")?;

    let merged = args
        .load_and_merge_with_matches(&matches)
        .context("merge absent clap defaults")?;

    ensure!(
        merged.retries == STRUCT_DEFAULT_RETRIES,
        "expected {STRUCT_DEFAULT_RETRIES}, not the clap default {CLAP_DEFAULT_RETRIES}: {}",
        merged.retries
    );
    Ok(())
}

/// `SubcmdConfigMerge::load_and_merge_with_matches` still honours a value the
/// user supplied explicitly.
///
/// Catches a forwarding that drops the parsed CLI values or substitutes the
/// struct default, either of which would leave `STRUCT_DEFAULT_RETRIES` instead
/// of `CLI_RETRIES`.
#[test]
fn matches_method_honours_explicit_cli_values() -> Result<()> {
    let (_dir, _cwd_guard) = empty_dir_with_cwd()?;
    let retries = CLI_RETRIES.to_string();
    let matches = IssueArgs::command().get_matches_from(["issue", "--retries", retries.as_str()]);
    let args = IssueArgs::from_arg_matches(&matches).context("parse explicit values")?;

    let merged = args
        .load_and_merge_with_matches(&matches)
        .context("merge explicit CLI values")?;

    ensure!(
        merged.retries == CLI_RETRIES,
        "expected the explicit CLI value {CLI_RETRIES}, got {}",
        merged.retries
    );
    Ok(())
}

/// `SubcmdConfigMerge::load_and_merge_with_matches` lets the file layer supply
/// a value the user did not pass, instead of the clap default.
///
/// Catches a forwarding that ignores `matches`, which would leak
/// `CLAP_DEFAULT_RETRIES` over the staged `FILE_RETRIES`; reaching the file at
/// all also shows that file discovery still runs.
#[test]
fn matches_method_prefers_the_file_over_absent_clap_defaults() -> Result<()> {
    ensure_key_is_unset(ISSUE_RETRIES_KEY)?;
    let (_dir, _cwd_guard) = issue_config_dir()?;
    let matches = IssueArgs::command().get_matches_from(["issue"]);
    let args = IssueArgs::from_arg_matches(&matches).context("parse clap defaults")?;

    let merged = args
        .load_and_merge_with_matches(&matches)
        .context("merge file defaults beneath clap defaults")?;

    ensure!(
        merged.retries == FILE_RETRIES,
        "expected the file value {FILE_RETRIES}, not the clap default {CLAP_DEFAULT_RETRIES}: {}",
        merged.retries
    );
    Ok(())
}

/// `SubcmdConfigMerge::load_and_merge_with_matches_with_sources` merges the
/// supplied scan source beneath absent clap defaults.
///
/// Catches a forwarding that drops the supplied source, which would leave
/// either `STRUCT_DEFAULT_RETRIES` or `CLAP_DEFAULT_RETRIES` in place instead
/// of `INJECTED_RETRIES`. Requiring the key to be unset in the process
/// environment also shows the value came from the injected source rather than
/// from the live environment.
#[test]
fn matches_with_sources_method_reads_the_injected_environment() -> Result<()> {
    ensure_key_is_unset(ISSUE_RETRIES_KEY)?;
    let (_dir, _cwd_guard) = empty_dir_with_cwd()?;
    let matches = IssueArgs::command().get_matches_from(["issue"]);
    let args = IssueArgs::from_arg_matches(&matches).context("parse clap defaults")?;
    let source = scan_source(ISSUE_RETRIES_KEY, INJECTED_RETRIES);

    let merged = args
        .load_and_merge_with_matches_with_sources(&matches, source)
        .context("merge the injected source beneath clap defaults")?;

    ensure!(
        merged.retries == INJECTED_RETRIES,
        "expected the injected {INJECTED_RETRIES}, got {}",
        merged.retries
    );
    Ok(())
}

/// `SubcmdConfigMerge::load_and_merge_with_matches_with_sources` keeps explicit
/// CLI values above the injected environment.
///
/// Catches a forwarding that applies the environment layer last, which would
/// leave `INJECTED_RETRIES` instead of `CLI_RETRIES`.
#[test]
fn matches_with_sources_method_keeps_cli_values_above_the_source() -> Result<()> {
    let (_dir, _cwd_guard) = empty_dir_with_cwd()?;
    let retries = CLI_RETRIES.to_string();
    let matches = IssueArgs::command().get_matches_from(["issue", "--retries", retries.as_str()]);
    let args = IssueArgs::from_arg_matches(&matches).context("parse explicit values")?;
    let source = scan_source(ISSUE_RETRIES_KEY, INJECTED_RETRIES);

    let merged = args
        .load_and_merge_with_matches_with_sources(&matches, source)
        .context("merge explicit CLI values over the injected source")?;

    ensure!(
        merged.retries == CLI_RETRIES,
        "expected the CLI value {CLI_RETRIES}, not the injected {INJECTED_RETRIES}: {}",
        merged.retries
    );
    Ok(())
}
