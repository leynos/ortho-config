//! Baseline loading behaviour for subcommand configuration.

use anyhow::{Context as _, Result, ensure};
use cap_std::{ambient_authority, fs::Dir};
use clap::Parser;
#[cfg(any(unix, target_os = "redox"))]
use ortho_config::OrthoError;
use ortho_config::subcommand::Prefix;
use ortho_config::{
    EnvSource, MapEnv, ProcessEnv, SubcommandFileContext, load_and_merge_subcommand,
    load_and_merge_subcommand_with_sources_at,
};
use serde::{Deserialize, Serialize};
use std::ffi::OsStr;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;

use super::fixtures::close_discovery;
use super::to_anyhow::ToAnyhow as _;

const PROCESS_PROBE_MARKER: &str = "ORTHO_SUBCOMMAND_PROCESS_PROBE_OK";

#[derive(Debug, Serialize, Deserialize, Default, PartialEq, Parser)]
#[command(name = "test")]
struct CmdCfg {
    #[serde(skip_serializing_if = "Option::is_none")]
    #[arg(long)]
    foo: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[arg(long)]
    bar: Option<bool>,
}

/// Write one fixture below `root` through an explicitly scoped capability.
///
/// Paths here are nested (`home/.app.toml`, `xdg/app/config.toml`), so the
/// parent directories are created first; `cap_std::fs::Dir::write` opens with
/// `O_CREAT` but never creates intermediate components.
///
/// This stays private to the baseline-loading module: its callers need the
/// same simple file shape, while the other subcommand suites exercise distinct
/// configuration boundaries and should keep those arrangements explicit.
fn write_config(root: &Path, relative: &Path, contents: &str) -> Result<()> {
    let directory = Dir::open_ambient_dir(root, ambient_authority())
        .context("open basic subcommand fixture directory")?;
    if let Some(parent) = relative
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
    {
        directory
            .create_dir_all(parent)
            .context("create basic subcommand fixture parent")?;
    }
    directory
        .write(relative, contents.as_bytes())
        .context("write basic subcommand fixture")?;
    Ok(())
}

/// Load and merge `CmdCfg` from `base`, `discovery`, and `merge_env`.
///
/// Every case in this module loads through the same prefix and the same
/// default CLI value, so those stay here rather than being restated at each
/// call site. What varies — the file base, the discovery source, and the
/// values merged on top — stays with the caller.
///
/// The conversion through [`ToAnyhow`] happens here rather than at each call
/// site so the tests can keep attaching their own scenario-specific
/// `.context(...)` while still reporting through `anyhow`.
fn load_cfg(base: &Path, discovery: &MapEnv, merge_env: MapEnv) -> Result<CmdCfg> {
    load_and_merge_subcommand_with_sources_at(
        &Prefix::new("APP_"),
        &CmdCfg::default(),
        SubcommandFileContext::new(base, discovery),
        Arc::new(merge_env),
    )
    .to_anyhow()
}

/// Merge file defaults with an explicit discovery source and no merge values.
///
/// This helper is private to the file-focused scenarios in this module. Tests
/// that need injected merge values continue to use [`load_cfg`].
///
/// # Examples
///
/// `merge_file_defaults(base, &discovery)` returns the command values loaded
/// from the discovered file, without consulting the process environment.
fn merge_file_defaults(base: &Path, discovery: &MapEnv) -> Result<CmdCfg> {
    load_and_merge_subcommand_with_sources_at(
        &Prefix::new("APP_"),
        &CmdCfg::default(),
        SubcommandFileContext::new(base, discovery),
        Arc::new(MapEnv::new()),
    )
    .to_anyhow()
}

/// Resolve an inherited coverage profile path against the wrapper's directory.
///
/// The child runs inside the fixture root, which is deleted as soon as this
/// test returns. A relative `LLVM_PROFILE_FILE` would therefore write its
/// profile into the fixture and lose it with the directory, so the inherited
/// path is anchored to the wrapper's working directory first. Absolute paths
/// already name their destination and pass through unchanged.
///
/// # Examples
///
/// `/tmp/app.profraw` stays `/tmp/app.profraw`; `profiles/app.profraw`
/// becomes `<parent working directory>/profiles/app.profraw`.
fn profile_file_path(profile_file: &OsStr, parent_dir: &Path) -> PathBuf {
    let profile_path = Path::new(profile_file);
    if profile_path.is_absolute() {
        return profile_path.to_path_buf();
    }
    parent_dir.join(profile_path)
}

/// Prove relative coverage paths are anchored and absolute ones preserved.
#[test]
fn profile_file_path_keeps_absolute_and_anchors_relative() -> Result<()> {
    let parent = PathBuf::from("parent-work-dir");
    ensure!(
        profile_file_path(OsStr::new("profiles/run.profraw"), &parent)
            == parent.join("profiles/run.profraw"),
        "relative profile path should be anchored to the parent working directory"
    );
    let absolute = std::env::temp_dir().join("ortho-config-run.profraw");
    ensure!(
        profile_file_path(absolute.as_os_str(), &parent) == absolute,
        "absolute profile path should pass through unchanged"
    );
    Ok(())
}

/// Check the process-backed default entrypoint without mutating the test process.
#[test]
fn process_backed_wrapper_merges_file_and_environment_defaults() -> Result<()> {
    let root = tempfile::tempdir().context("create process-backed subcommand fixture")?;
    write_config(
        root.path(),
        Path::new(".app.toml"),
        "[cmds.test]\nfoo = \"file\"\nbar = true",
    )?;
    let executable = std::env::current_exe().context("locate subcommand test executable")?;
    let mut child = Command::new(executable);
    child
        .args([
            "--ignored",
            "--exact",
            "--nocapture",
            "basic::process_backed_probe",
        ])
        .current_dir(root.path())
        .env_clear()
        .env("APP_CMDS_TEST_FOO", "env")
        .env("XDG_CONFIG_DIRS", root.path());
    if let Some(profile_file) = ProcessEnv.get("LLVM_PROFILE_FILE") {
        child.env(
            "LLVM_PROFILE_FILE",
            profile_file_path(&profile_file, &std::env::current_dir()?),
        );
    }
    let output = child
        .output()
        .context("run isolated process-backed subcommand probe")?;
    ensure!(
        output.status.success(),
        "process-backed subcommand probe failed: stdout={}, stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    ensure!(
        String::from_utf8_lossy(&output.stdout).contains(PROCESS_PROBE_MARKER),
        "process-backed subcommand probe did not execute: {}",
        String::from_utf8_lossy(&output.stdout)
    );
    Ok(())
}

#[test]
#[ignore = "run by the isolated process-backed wrapper test"]
fn process_backed_probe() -> Result<()> {
    let cfg = load_and_merge_subcommand(&Prefix::new("APP_"), &CmdCfg::default())
        .context("merge process-backed subcommand defaults")?;
    ensure!(
        cfg.foo.as_deref() == Some("env"),
        "expected environment value over file value, got {:?}",
        cfg.foo
    );
    ensure!(
        cfg.bar == Some(true),
        "expected file-backed bar value, got {:?}",
        cfg.bar
    );
    std::io::stdout()
        .write_all(PROCESS_PROBE_MARKER.as_bytes())
        .context("signal process-backed probe completion")?;
    Ok(())
}

#[test]
fn file_and_env_loading() -> Result<()> {
    let root = tempfile::tempdir().context("create file and environment fixture")?;
    write_config(
        root.path(),
        Path::new(".app.toml"),
        "[cmds.test]\nfoo = \"file\"\nbar = true",
    )?;
    let discovery = close_discovery(root.path());
    let cfg = load_cfg(
        root.path(),
        &discovery,
        MapEnv::new().with_var("APP_CMDS_TEST_FOO", "env"),
    )
    .context("merge file and injected environment defaults")?;
    ensure!(
        cfg.foo.as_deref() == Some("env"),
        "expected env, got {:?}",
        cfg.foo
    );
    ensure!(cfg.bar == Some(true), "expected true, got {:?}", cfg.bar);
    Ok(())
}

/// Loads the configuration file under the injected home directory.
#[test]
fn loads_from_home() -> Result<()> {
    let root = tempfile::tempdir().context("create home fixture")?;
    let home = root.path().join("home");
    let base = root.path().join("base");
    write_config(
        root.path(),
        Path::new("home/.app.toml"),
        "[cmds.test]\nfoo = \"home\"",
    )?;
    let discovery = close_discovery(root.path()).with_var("HOME", &home);
    let cfg = merge_file_defaults(&base, &discovery)
        .context("merge home defaults from injected source")?;
    ensure!(
        cfg.foo.as_deref() == Some("home"),
        "expected home, got {:?}",
        cfg.foo
    );
    Ok(())
}

/// Confirms a local configuration file takes precedence over the home file.
#[test]
fn local_overrides_home() -> Result<()> {
    let root = tempfile::tempdir().context("create local-overrides-home fixture")?;
    let home = root.path().join("home");
    let base = root.path().join("base");
    write_config(
        root.path(),
        Path::new("home/.app.toml"),
        "[cmds.test]\nfoo = \"home\"",
    )?;
    write_config(
        root.path(),
        Path::new("base/.app.toml"),
        "[cmds.test]\nfoo = \"local\"",
    )?;
    let discovery = close_discovery(root.path()).with_var("HOME", &home);
    let cfg = merge_file_defaults(&base, &discovery)
        .context("merge local defaults after injected home defaults")?;
    ensure!(
        cfg.foo.as_deref() == Some("local"),
        "expected local, got {:?}",
        cfg.foo
    );
    Ok(())
}

// Windows lacks XDG support.
/// Loads the configuration file from the injected XDG config directory.
#[cfg(any(unix, target_os = "redox"))]
#[test]
fn loads_from_xdg_config() -> Result<()> {
    let root = tempfile::tempdir().context("create XDG fixture")?;
    let xdg = root.path().join("xdg");
    let base = root.path().join("base");
    write_config(
        root.path(),
        Path::new("xdg/app/config.toml"),
        "[cmds.test]\nfoo = \"xdg\"",
    )?;
    let discovery = close_discovery(root.path()).with_var("XDG_CONFIG_HOME", &xdg);
    let cfg = merge_file_defaults(&base, &discovery)
        .context("merge XDG defaults from injected source")?;
    ensure!(
        cfg.foo.as_deref() == Some("xdg"),
        "expected xdg, got {:?}",
        cfg.foo
    );
    Ok(())
}

/// The public loader returns non-missing XDG metadata failures with their path.
#[cfg(any(unix, target_os = "redox"))]
#[test]
fn xdg_metadata_error_reaches_public_subcommand_loader() -> Result<()> {
    let root = tempfile::tempdir().context("create XDG error fixture")?;
    write_config(root.path(), Path::new("not-a-directory"), "")?;
    let not_a_directory = root.path().join("not-a-directory");
    let candidate = not_a_directory.join("app/config.toml");
    let discovery = close_discovery(root.path()).with_var("XDG_CONFIG_HOME", &not_a_directory);

    let error = load_and_merge_subcommand_with_sources_at(
        &Prefix::new("APP_"),
        &CmdCfg::default(),
        SubcommandFileContext::new(root.path(), &discovery),
        Arc::new(MapEnv::new()),
    )
    .expect_err("a failed XDG metadata probe must reach the public loader");
    let OrthoError::File {
        path,
        source: error_source,
    } = error.as_ref()
    else {
        anyhow::bail!("expected a file error, got {error:?}");
    };
    ensure!(
        path == &candidate,
        "reported candidate path differs: {path:?}"
    );
    let io_error = error_source
        .downcast_ref::<std::io::Error>()
        .context("metadata error source was not an I/O error")?;
    ensure!(
        io_error.kind() != std::io::ErrorKind::NotFound,
        "a non-directory parent must not be treated as a missing candidate"
    );
    Ok(())
}

/// Loads YAML configuration from the explicit discovery base.
#[cfg(feature = "yaml")]
#[test]
fn loads_yaml_file() -> Result<()> {
    let root = tempfile::tempdir().context("create YAML fixture")?;
    write_config(
        root.path(),
        Path::new(".app.yml"),
        "cmds:\n  test:\n    foo: yaml",
    )?;
    let discovery = close_discovery(root.path());
    let cfg = merge_file_defaults(root.path(), &discovery)
        .context("merge YAML defaults from explicit base")?;
    ensure!(
        cfg.foo.as_deref() == Some("yaml"),
        "expected yaml, got {:?}",
        cfg.foo
    );
    Ok(())
}
