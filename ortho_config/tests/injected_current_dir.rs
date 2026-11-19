//! The injected environment also supplies the working directory.
//!
//! The derived loaders search the working directory for project configuration.
//! `EnvSource::current_dir` lets a test name that directory on the injected
//! object, so these cases load project files from temporary directories without
//! changing the process's own and run concurrently with everything else.

use anyhow::{Context as _, Result, anyhow, ensure};
use cap_std::{ambient_authority, fs::Dir};
use clap::{CommandFactory, FromArgMatches, Parser};
use ortho_config::{EnvSource, MapEnv, OrthoConfig, ScanEnvSource, SubcmdConfigMerge};
use rstest::rstest;
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::sync::Arc;
use tempfile::TempDir;

#[path = "support/strict_required.rs"]
mod strict_required;

use strict_required::{STRICT_REFERENCE_VAR, StrictCmd, assert_reports_the_absent_reference};

/// Derived configuration found through its project dotfile.
#[derive(Debug, Deserialize, Serialize, OrthoConfig)]
#[ortho_config(prefix = "CWDSEAM_")]
struct SeamConfig {
    #[ortho_config(default = 1)]
    port: u16,
}

/// Subcommand configuration found through its project dotfile.
#[derive(Debug, Parser, Serialize, Deserialize, OrthoConfig, Default, PartialEq)]
#[command(name = "seam")]
#[ortho_config(prefix = "CWDSEAM_")]
struct SeamCmd {
    #[arg(long)]
    reference: Option<String>,
}

/// Subcommand whose clap default must not reach the merge unless explicit.
#[derive(Debug, Parser, Serialize, Deserialize, OrthoConfig, PartialEq)]
#[command(name = "matched")]
#[ortho_config(prefix = "CWDSEAM_")]
struct MatchedCmd {
    #[arg(long, default_value = "2")]
    #[ortho_config(default = 5, cli_default_as_absent)]
    retries: u32,
}

impl Default for MatchedCmd {
    /// Mirrors the `#[ortho_config(default = ...)]` value, not clap's.
    fn default() -> Self {
        Self { retries: 5 }
    }
}

/// A temporary directory holding one configuration file.
fn dir_with(file: &str, body: &str) -> Result<TempDir> {
    let dir = tempfile::tempdir().context("create the temporary directory")?;
    Dir::open_ambient_dir(dir.path(), ambient_authority())
        .context("open the temporary directory")?
        .write(file, body)
        .context("write the fixture")?;
    Ok(dir)
}

/// A closed-set environment whose working directory is `dir`.
///
/// XDG and home are named explicitly, so no host directory can contribute.
fn env_in(dir: &Path) -> MapEnv {
    MapEnv::new()
        .with_current_dir(dir)
        .with_var("XDG_CONFIG_HOME", dir.join("xdg-empty"))
        .with_var("XDG_CONFIG_DIRS", dir.join("xdg-dirs-empty"))
        .with_var("HOME", dir.join("home-empty"))
}

/// The map reports the directory it was given and not the process's.
#[test]
fn map_env_reports_its_pinned_directory() -> Result<()> {
    let process_dir = std::env::current_dir().context("read the process directory")?;
    let pinned = MapEnv::new().with_current_dir("/pinned/by/the/test");
    ensure!(pinned.current_dir()? == Path::new("/pinned/by/the/test"));
    ensure!(
        pinned.current_dir()? != process_dir,
        "the pinned directory must not be the process directory"
    );
    Ok(())
}

/// An unpinned map keeps reporting the process directory, so callers that
/// never set one see no change.
#[test]
fn unpinned_map_env_reports_the_process_directory() -> Result<()> {
    let process_dir = std::env::current_dir().context("read the process directory")?;
    ensure!(MapEnv::new().current_dir()? == process_dir);
    Ok(())
}

/// The derived loader reads the project file from the injected directory.
#[rstest]
#[case::file_present(Some("port = 4321\n"), 4321)]
#[case::file_absent(None, 1)]
fn derived_load_reads_the_injected_working_directory(
    #[case] file: Option<&str>,
    #[case] expected: u16,
) -> Result<()> {
    let dir = match file {
        Some(body) => dir_with(".cwdseam.toml", body)?,
        None => tempfile::tempdir().context("create the empty directory")?,
    };
    let env = Arc::new(env_in(dir.path()));

    let config = SeamConfig::load_from_iter_with_env(["seam"], env).map_err(|err| anyhow!(err))?;

    ensure!(
        config.port == expected,
        "expected port {expected}, got {}",
        config.port
    );
    Ok(())
}

/// Two loads in two directories see only their own file, concurrently safe
/// because nothing is shared.
#[test]
fn distinct_directories_do_not_leak_into_each_other() -> Result<()> {
    let first = dir_with(".cwdseam.toml", "port = 1111\n")?;
    let second = dir_with(".cwdseam.toml", "port = 2222\n")?;

    let one = SeamConfig::load_from_iter_with_env(["seam"], Arc::new(env_in(first.path())))
        .map_err(|err| anyhow!(err))?;
    let two = SeamConfig::load_from_iter_with_env(["seam"], Arc::new(env_in(second.path())))
        .map_err(|err| anyhow!(err))?;

    ensure!(
        (one.port, two.port) == (1111, 2222),
        "got {one:?} and {two:?}"
    );
    Ok(())
}

/// The subcommand merge reads `[cmds.seam]` from the injected directory and
/// layers the injected environment above it.
#[test]
fn subcommand_merge_reads_the_injected_working_directory() -> Result<()> {
    let dir = dir_with(".cwdseam.toml", "[cmds.seam]\nreference = \"file\"\n")?;
    let cli = SeamCmd::default();

    let from_file = cli
        .load_and_merge_with_env(Arc::new(env_in(dir.path())))
        .map_err(|err| anyhow!(err))?;
    ensure!(
        from_file.reference.as_deref() == Some("file"),
        "expected the file value, got {:?}",
        from_file.reference
    );

    let above_file = cli
        .load_and_merge_with_env(Arc::new(
            env_in(dir.path()).with_var("CWDSEAM_CMDS_SEAM_REFERENCE", "env"),
        ))
        .map_err(|err| anyhow!(err))?;
    ensure!(
        above_file.reference.as_deref() == Some("env"),
        "expected the environment above the file, got {:?}",
        above_file.reference
    );
    Ok(())
}

/// A source that cannot report a working directory.
#[derive(Debug, Default)]
struct NoWorkingDirectory(MapEnv);

impl EnvSource for NoWorkingDirectory {
    fn get(&self, key: &str) -> Option<std::ffi::OsString> {
        self.0.get(key)
    }

    fn current_dir(&self) -> std::io::Result<std::path::PathBuf> {
        Err(std::io::Error::other("deleted"))
    }
}

impl ScanEnvSource for NoWorkingDirectory {
    fn scan(&self) -> Vec<(std::ffi::OsString, std::ffi::OsString)> {
        self.0.scan()
    }
}

/// A failed directory lookup is reported, not replaced by `.`, so a process
/// relative file can never be read through an injected source.
#[test]
fn a_failed_directory_lookup_is_an_error_for_subcommands() -> Result<()> {
    let outcome =
        SeamCmd::default().load_and_merge_with_env(Arc::new(NoWorkingDirectory::default()));

    let error = outcome.err().ok_or_else(|| anyhow!("expected an error"))?;
    ensure!(
        matches!(error.as_ref(), ortho_config::OrthoError::Validation { key, .. } if key == "current_dir"),
        "unexpected error: {error:?}"
    );
    Ok(())
}

/// The derived loader survives a failed lookup by omitting the implicit
/// project root, so it reads no process-relative file either.
#[test]
fn a_failed_directory_lookup_omits_the_derived_project_root() -> Result<()> {
    let config =
        SeamConfig::load_from_iter_with_env(["seam"], Arc::new(NoWorkingDirectory::default()))
            .map_err(|err| anyhow!(err))?;

    ensure!(
        config.port == 1,
        "expected the default port, got {}",
        config.port
    );
    Ok(())
}

/// Loads in different directories on different threads see only their own file.
#[test]
fn concurrent_loads_in_distinct_directories_stay_independent() -> Result<()> {
    let directories = (1..=4_u16)
        .map(|n| dir_with(".cwdseam.toml", &format!("port = {}\n", n * 1000)).map(|dir| (n, dir)))
        .collect::<Result<Vec<_>>>()?;

    let ports = std::thread::scope(|scope| {
        let handles = directories
            .iter()
            .map(|(_, dir)| {
                scope.spawn(|| {
                    SeamConfig::load_from_iter_with_env(["seam"], Arc::new(env_in(dir.path())))
                        .map(|config| config.port)
                })
            })
            .collect::<Vec<_>>();
        handles
            .into_iter()
            .map(|handle| {
                handle
                    .join()
                    .map_err(|_| anyhow!("a loader thread panicked"))
            })
            .collect::<Result<Vec<_>>>()
    })?;

    for ((n, _), outcome) in directories.iter().zip(ports) {
        let port = outcome.map_err(|err| anyhow!(err))?;
        ensure!(port == n * 1000, "directory {n} loaded port {port}");
    }
    Ok(())
}

/// A clap default must not shadow the injected directory's file value through
/// the matches-aware form, while an explicit CLI value still wins.
#[rstest]
#[case::default_stays_absent(&["matched"], 9)]
#[case::explicit_cli_wins(&["matched", "--retries", "3"], 3)]
fn subcommand_matches_form_keeps_clap_defaults_absent(
    #[case] args: &[&str],
    #[case] expected: u32,
) -> Result<()> {
    let dir = dir_with(".cwdseam.toml", "[cmds.matched]\nretries = 9\n")?;
    let matches = MatchedCmd::command().get_matches_from(args);
    let cli = MatchedCmd::from_arg_matches(&matches).context("parse the CLI values")?;

    let merged = cli
        .load_and_merge_with_matches_with_env(&matches, Arc::new(env_in(dir.path())))
        .map_err(|err| anyhow!(err))?;

    ensure!(
        merged.retries == expected,
        "expected {expected}, got {}",
        merged.retries
    );
    Ok(())
}

/// The single-environment form enumerates an absent required value.
///
/// Without the schema-aware entry point the merged value simply fails to
/// deserialize, so a caller learns that something is missing but not which key
/// or where to supply it.
#[test]
fn single_environment_merge_enumerates_an_absent_required_value() -> Result<()> {
    let dir = tempfile::tempdir().context("create the empty directory")?;

    assert_reports_the_absent_reference(
        StrictCmd::default().load_and_merge_with_env(Arc::new(env_in(dir.path()))),
    )?;

    let supplied = StrictCmd::default()
        .load_and_merge_with_env(Arc::new(
            env_in(dir.path()).with_var(STRICT_REFERENCE_VAR, "from-env"),
        ))
        .map_err(|err| anyhow!(err))?;
    ensure!(
        supplied.reference == "from-env",
        "the offered environment route must supply the value, got {supplied:?}"
    );
    Ok(())
}

/// The matches-aware form enumerates the same absent required value.
#[test]
fn matches_form_enumerates_an_absent_required_value() -> Result<()> {
    let dir = tempfile::tempdir().context("create the empty directory")?;
    let matches = StrictCmd::command().get_matches_from(["strict"]);

    assert_reports_the_absent_reference(
        StrictCmd::default()
            .load_and_merge_with_matches_with_env(&matches, Arc::new(env_in(dir.path()))),
    )?;

    let supplied = StrictCmd::default()
        .load_and_merge_with_matches_with_env(
            &matches,
            Arc::new(env_in(dir.path()).with_var(STRICT_REFERENCE_VAR, "from-env")),
        )
        .map_err(|err| anyhow!(err))?;
    ensure!(
        supplied.reference == "from-env",
        "the matches form must read the same environment layer, got {supplied:?}"
    );
    Ok(())
}
