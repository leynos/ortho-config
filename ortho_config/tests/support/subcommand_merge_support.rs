//! Shared scaffolding for the `SubcmdConfigMerge` behavioural suite.
//!
//! The suite in `subcommand_merge_methods.rs` drives the public trait methods,
//! and every case needs the same things: subcommand structs whose per-layer
//! defaults are distinguishable, staged configuration directories, and a way to
//! run the process-backed methods without changing this process.
//!
//! # Scope and re-use policy
//!
//! This module is owned by the `subcommand_merge_methods` integration target
//! and is reachable through `#[path]`. It is a *support file*, named by
//! `tests/support/` convention rather than by module semantics, so the suite
//! file stays inside the repository's 400-line limit without its coverage being
//! trimmed to get there.
//!
//! # Why the methods run in a child process
//!
//! `load_and_merge` and the `_with_sources` forms resolve configuration files
//! through `ProcessEnv` and the working directory by design, so a case cannot
//! inject them. Changing this process's environment or directory instead would
//! race every other test in the binary, so each case runs the method in a child
//! started with an exact environment and working directory (see
//! `process_child.rs`). The hermetic `_with_env` forms need no child.

use anyhow::{Context, Result};
use cap_std::{ambient_authority, fs::Dir};
use clap::{ArgMatches, CommandFactory, FromArgMatches, Parser};
use ortho_config::{MapEnv, OrthoConfig, SharedScanEnvSource, SubcmdConfigMerge};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tempfile::TempDir;

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

    /// Encode the row so it can travel to a child process as one variable.
    pub fn encode(self) -> String {
        let method = match self.method {
            MatchesMethod::Plain => "plain",
            MatchesMethod::WithSources => "with_sources",
        };
        let cli = match self.cli {
            CliMode::Absent => "absent",
            CliMode::Explicit => "explicit",
        };
        let injected = self
            .injected
            .map_or_else(|| "none".to_owned(), |value| value.to_string());
        format!("{method},{cli},{injected},{}", self.expected)
    }

    /// Decode a row produced by [`Self::encode`].
    pub fn decode(encoded: &str) -> Result<Self> {
        let parts: Vec<&str> = encoded.split(',').collect();
        let [method, cli, injected, expected] = parts.as_slice() else {
            anyhow::bail!("malformed row `{encoded}`");
        };
        Ok(Self {
            method: match *method {
                "plain" => MatchesMethod::Plain,
                "with_sources" => MatchesMethod::WithSources,
                other => anyhow::bail!("unknown method `{other}`"),
            },
            cli: match *cli {
                "absent" => CliMode::Absent,
                "explicit" => CliMode::Explicit,
                other => anyhow::bail!("unknown CLI mode `{other}`"),
            },
            injected: match *injected {
                "none" => None,
                value => Some(value.parse().context("parse the injected value")?),
            },
            expected: expected.parse().context("parse the expected value")?,
        })
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

/// A temporary directory holding a staged `.vk.toml`, plus an empty tree for
/// every home and platform rung.
///
/// Nothing enters the directory: the case hands it to a child as its working
/// directory.
pub struct StagedDir {
    /// Holds the staged file; the child's working directory.
    pub dir: TempDir,
    /// Empty tree that every home and platform-configuration variable names.
    pub empty_root: TempDir,
}

/// Writes `cfg` to `.vk.toml` in a fresh temporary directory.
fn staged_dir(cfg: &str) -> Result<StagedDir> {
    let dir = tempfile::tempdir().context("create temp dir")?;
    let cap = Dir::open_ambient_dir(dir.path(), ambient_authority()).context("open temp dir")?;
    cap.write(".vk.toml", cfg.as_bytes())
        .context("write config")?;
    let empty_root = tempfile::tempdir().context("create the empty configuration tree")?;
    Ok(StagedDir { dir, empty_root })
}

/// A directory holding no subcommand configuration.
pub fn empty_dir() -> Result<StagedDir> {
    staged_dir("")
}

/// A directory whose only file supplies `FILE_REFERENCE`.
pub fn pr_dir() -> Result<StagedDir> {
    staged_dir(&format!("[cmds.pr]\nreference = \"{FILE_REFERENCE}\"\n"))
}

/// A directory whose only file supplies `FILE_RETRIES`.
pub fn issue_dir() -> Result<StagedDir> {
    staged_dir(&format!("[cmds.issue]\nretries = {FILE_RETRIES}\n"))
}
