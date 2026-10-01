//! Shared scaffolding for the `SubcmdConfigMerge` behavioural suite.
//!
//! The suite in `subcommand_merge_methods.rs` drives the three public trait
//! methods, and every case needs the same three things: subcommand structs
//! whose per-layer defaults are distinguishable, staged configuration
//! directories to enter, and a neutralised process environment.
//!
//! # Scope and re-use policy
//!
//! This module is owned by the `subcommand_merge_methods` integration target
//! and is reachable through `#[path]`. It is a *support file*, named by
//! `tests/support/` convention rather than by module semantics, so the suite
//! file stays inside the repository's 400-line limit without its coverage being
//! trimmed to get there.
//!
//! # Why the environment is neutralised rather than injected
//!
//! The methods under test take a `SharedScanEnvSource` for the merge layer but
//! hard-code `ProcessEnv` and the working directory for file discovery, so the
//! ambient environment cannot be injected away. [`isolated_env`] therefore
//! points every rung discovery consults — `HOME`, `USERPROFILE`,
//! `XDG_CONFIG_HOME`, `XDG_CONFIG_DIRS`, `APPDATA` — at an empty temporary
//! tree, wrapped in the shared `test_helpers` guards that AGENTS.md requires for
//! any environment mutation.
//!
//! Setting `HOME` is deliberate. `ProcessEnv::home_fallback` consults it through
//! `dirs::home_dir`, which honours the variable when present and falls back to
//! the real user database (`getpwuid_r`, via `passwd`) when it is absent or
//! empty. Clearing it would leave the rung open rather than closing it.

use anyhow::{Context, Result, ensure};
use cap_std::{ambient_authority, fs::Dir};
use clap::{ArgMatches, CommandFactory, FromArgMatches, Parser};
use ortho_config::{MapEnv, OrthoConfig, SharedScanEnvSource, SubcmdConfigMerge};
use rstest::fixture;
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::sync::Arc;
use tempfile::TempDir;
use test_helpers::cwd;
use test_helpers::env::EnvScope;

/// Subcommand used by the plain injected-source cases.
#[derive(Debug, Parser, Serialize, Deserialize, OrthoConfig, Default, PartialEq)]
#[command(name = "pr")]
#[ortho_config(prefix = "VK_")]
pub struct PrArgs {
    /// The reference the injected source, the file, and the CLI all supply.
    #[arg(long)]
    pub reference: Option<String>,
}

/// Subcommand whose clap default must not reach the merge unless the user
/// supplies the value explicitly.
#[derive(Debug, Parser, Serialize, Deserialize, OrthoConfig, PartialEq)]
#[command(name = "issue")]
#[ortho_config(prefix = "VK_")]
pub struct IssueArgs {
    /// The explicit default deliberately differs from the clap default, so a
    /// leaked clap default is observable in the merged result.
    #[arg(long, default_value = "2")]
    #[ortho_config(default = 5, cli_default_as_absent)]
    pub retries: u32,
}

impl Default for IssueArgs {
    /// Mirrors the `#[ortho_config(default = ...)]` value, not clap's.
    fn default() -> Self {
        Self { retries: 5 }
    }
}

/// Environment key holding the injected reference for [`PrArgs`].
pub const PR_REFERENCE_KEY: &str = "VK_CMDS_PR_REFERENCE";
/// Injected environment value for `PR_REFERENCE_KEY`.
pub const INJECTED_REFERENCE: &str = "injected_ref";
/// File value written to the `[cmds.pr] reference` key.
pub const FILE_REFERENCE: &str = "file_ref";
/// Explicit CLI value passed as `--reference`.
pub const CLI_REFERENCE: &str = "cli_ref";

/// Environment key holding the injected retry count for [`IssueArgs`].
pub const ISSUE_RETRIES_KEY: &str = "VK_CMDS_ISSUE_RETRIES";
/// Injected environment value for `ISSUE_RETRIES_KEY`.
pub const INJECTED_RETRIES: u32 = 7;
/// File value written to the `[cmds.issue] retries` key.
pub const FILE_RETRIES: u32 = 4;
/// Explicit CLI value passed as `--retries`.
pub const CLI_RETRIES: u32 = 9;
/// What clap's own `default_value` supplies, which must stay absent.
pub const CLAP_DEFAULT_RETRIES: u32 = 2;
/// What the explicit `#[ortho_config(default = ...)]` supplies.
pub const STRUCT_DEFAULT_RETRIES: u32 = 5;

/// Builds a scan source whose only variable is `key = value`.
pub fn scan_source(key: &str, value: impl std::fmt::Display) -> SharedScanEnvSource {
    Arc::new(MapEnv::new().with_var(key, value.to_string()))
}

/// Folds an optional injected value into the scan source the merge layer takes.
///
/// `None` means the process environment is the row's only environment layer,
/// which [`MatchesMethod::Plain`] already supplies; a [`MatchesMethod::WithSources`]
/// row without a value would be vacuous, so it is reported rather than run.
pub fn fold_source(value: Option<u32>) -> Result<SharedScanEnvSource> {
    value
        .map(|injected| scan_source(ISSUE_RETRIES_KEY, injected))
        .context("a with_sources row must supply an injected value")
}

/// Which match-aware trait method a table row exercises.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MatchesMethod {
    /// `load_and_merge_with_matches`, whose only environment layer is the
    /// process environment.
    Plain,
    /// `load_and_merge_with_matches_with_sources`, which additionally merges a
    /// caller-supplied scan source.
    WithSources,
}

impl MatchesMethod {
    /// Merges `parsed` by this method, with `source` as the injected value.
    ///
    /// A [`Self::Plain`] row supplies no injected value; a [`Self::WithSources`]
    /// row must supply one, because the method it names has no other use.
    fn merge(self, parsed: &ParsedIssue, source: Option<u32>) -> Result<IssueArgs> {
        match self {
            Self::Plain => parsed
                .args
                .load_and_merge_with_matches(&parsed.matches)
                .context("merge with matches"),
            Self::WithSources => {
                let injected = fold_source(source)?;
                parsed
                    .args
                    .load_and_merge_with_matches_with_sources(&parsed.matches, injected)
                    .context("merge with matches and sources")
            }
        }
    }
}

/// Whether a row passes `--retries` explicitly or leaves clap's default in
/// place.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CliMode {
    /// No flag: clap supplies its own `default_value`, which the merge must
    /// treat as absent.
    Absent,
    /// `--retries <CLI_RETRIES>` passed by the user.
    Explicit,
}

/// An `IssueArgs` together with the matches it was parsed from.
pub struct ParsedIssue {
    /// The raw clap matches, which carry explicit-versus-default metadata.
    pub matches: ArgMatches,
    /// The parsed subcommand values.
    pub args: IssueArgs,
}

/// Parses `IssueArgs` in the argv shape `cli` names.
pub fn parse_issue(cli: CliMode) -> Result<ParsedIssue> {
    let command_line: Vec<String> = match cli {
        CliMode::Absent => vec!["issue".to_owned()],
        CliMode::Explicit => vec![
            "issue".to_owned(),
            "--retries".to_owned(),
            CLI_RETRIES.to_string(),
        ],
    };
    let matches = IssueArgs::command().get_matches_from(command_line);
    let args = IssueArgs::from_arg_matches(&matches).context("parse row arguments")?;
    Ok(ParsedIssue { matches, args })
}

/// One row of the match-aware layer table.
///
/// The four fields are what a row states about the merge it expects; the staged
/// directory stays a case-level fixture rather than a row field, because which
/// configuration file a case runs against is a property of the case, not of the
/// precedence it asserts.
#[derive(Clone, Copy)]
pub struct LayerRow {
    /// Which match-aware method the row drives.
    pub method: MatchesMethod,
    /// Whether the caller supplied `--retries` or left clap's default in place.
    pub cli: CliMode,
    /// The value the caller-supplied scan source carries, if the row supplies
    /// one. A `WithSources` row must.
    pub injected: Option<u32>,
    /// The retry count a correctly wired merge must produce.
    pub expected: u32,
}

impl LayerRow {
    /// Builds a row. Short enough to keep the case list readable, where the
    /// field names would otherwise be repeated for every row.
    pub const fn new(
        method: MatchesMethod,
        cli: CliMode,
        injected: Option<u32>,
        expected: u32,
    ) -> Self {
        Self {
            method,
            cli,
            injected,
            expected,
        }
    }
}

/// Drives one match-aware row and returns the merged retry count.
///
/// Both match-aware methods share this shape — parse, merge, read the field —
/// so the table in `subcommand_merge_methods.rs` carries only what actually
/// distinguishes its rows, and a row cannot accidentally drive a different
/// method from the one it names.
pub fn merged_retries(method: MatchesMethod, cli: CliMode, source: Option<u32>) -> Result<u32> {
    let parsed = parse_issue(cli)?;
    Ok(method.merge(&parsed, source)?.retries)
}

/// Fails when `key` is visible in the process environment.
///
/// An exported variable reaches every merge through the process-backed
/// environment layer, so it can supply an expected value whether or not the
/// source under test contributes anything. Requiring the key to be unset keeps
/// each case decisive.
pub fn ensure_key_is_unset(key: &str) -> Result<()> {
    ensure!(
        std::env::var_os(key).is_none(),
        "{key} must be unset for this case to be meaningful"
    );
    Ok(())
}

/// Renders `path` as UTF-8 so it can be used as an environment variable value.
fn utf8(path: &Path, what: &str) -> Result<String> {
    path.to_str()
        .map(str::to_owned)
        .with_context(|| format!("{what} is not valid UTF-8: {}", path.display()))
}

/// Holds the neutralised environment for the duration of one case.
pub struct IsolatedEnv {
    /// Holds the process-environment lock and restores every variable on drop.
    _scope: EnvScope,
    /// Owns the empty tree that every discovery rung resolves into.
    _root: TempDir,
}

/// Points every environment rung discovery consults at an empty temporary tree,
/// and removes the subcommand variables the merge layer would otherwise read.
///
/// The methods under test hard-code `ProcessEnv`, so the ambient environment
/// cannot be injected away and is neutralised instead. `HOME` is *set* rather
/// than merely cleared because `ProcessEnv::home_fallback` consults it through
/// `dirs::home_dir`, which honours the variable when it is present and falls
/// back to the real user database when it is not. Every other discovery
/// variable is pointed at the same empty tree, so no rung can contribute a file
/// whatever the host exports and whichever platform the suite runs on.
///
/// Pointing the discovery rungs at an empty tree is not enough on its own. The
/// process environment is also a merge layer, sitting above the file and below
/// the CLI, so a key exported by the host would answer before the file does and
/// the layer the rows assert would be masked. The two keys are removed rather
/// than merely checked, because an exported value has to be made harmless
/// rather than reported; the guard returned restores it on drop, so nothing
/// leaks back into the running process.
#[fixture]
pub fn isolated_env() -> Result<IsolatedEnv> {
    let root = tempfile::tempdir().context("create isolated environment root")?;
    let home = utf8(root.path(), "isolated root")?;
    let config_home = utf8(&root.path().join(".config"), "isolated config home")?;
    let scope = EnvScope::new_with(|lock| {
        vec![
            lock.set_var("HOME", &home),
            lock.set_var("USERPROFILE", &home),
            lock.set_var("XDG_CONFIG_HOME", &config_home),
            lock.set_var("XDG_CONFIG_DIRS", &home),
            lock.set_var("APPDATA", &home),
            lock.remove_var(PR_REFERENCE_KEY),
            lock.remove_var(ISSUE_RETRIES_KEY),
        ]
    });
    Ok(IsolatedEnv {
        _scope: scope,
        _root: root,
    })
}

/// A temporary configuration directory the case has entered.
pub struct ConfigDir {
    /// Declared first so the working directory is restored before the directory
    /// it points at is removed.
    _guard: cwd::CwdGuard,
    /// Owns the directory holding the staged file.
    _dir: TempDir,
}

/// Writes `cfg` to `.vk.toml` in a fresh temporary directory and enters it.
fn staged_dir(cfg: &str) -> Result<ConfigDir> {
    let dir = tempfile::tempdir().context("create temp dir")?;
    let cap = Dir::open_ambient_dir(dir.path(), ambient_authority()).context("open temp dir")?;
    cap.write(".vk.toml", cfg.as_bytes())
        .context("write config")?;
    let guard = cwd::set_dir(dir.path()).context("enter temp config dir")?;
    Ok(ConfigDir {
        _guard: guard,
        _dir: dir,
    })
}

/// Enters a fresh temporary directory holding no subcommand configuration.
#[fixture]
pub fn empty_dir() -> Result<ConfigDir> {
    staged_dir("")
}

/// Enters a temporary directory whose only file supplies `FILE_REFERENCE`.
#[fixture]
pub fn pr_dir() -> Result<ConfigDir> {
    staged_dir(&format!("[cmds.pr]\nreference = \"{FILE_REFERENCE}\"\n"))
}

/// Enters a temporary directory whose only file supplies `FILE_RETRIES`.
#[fixture]
pub fn issue_dir() -> Result<ConfigDir> {
    staged_dir(&format!("[cmds.issue]\nretries = {FILE_RETRIES}\n"))
}
