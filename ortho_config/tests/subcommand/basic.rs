//! Baseline loading behaviour for subcommand configuration.

use anyhow::{Result, anyhow, ensure};
use clap::Parser;
#[cfg(any(unix, target_os = "redox"))]
use figment::Error as FigmentError;
use serde::{Deserialize, Serialize};

use super::util::{path_to_utf8_string, with_merged_subcommand_cli};

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

#[test]
fn file_and_env_loading() -> Result<()> {
    let cfg: CmdCfg = with_merged_subcommand_cli(
        |j| {
            j.create_file(".app.toml", "[cmds.test]\nfoo = \"file\"\nbar = true")?;
            j.set_env("APP_CMDS_TEST_FOO", "env");
            Ok(())
        },
        &CmdCfg::default(),
    )
    .map_err(|err| anyhow!(err))?;
    ensure!(
        cfg.foo.as_deref() == Some("env"),
        "expected env, got {:?}",
        cfg.foo
    );
    ensure!(cfg.bar == Some(true), "expected true, got {:?}", cfg.bar);
    Ok(())
}

#[test]
fn loads_from_home() -> Result<()> {
    let cfg: CmdCfg = with_merged_subcommand_cli(
        |j| {
            let home = j.create_dir("home")?;
            j.create_file(home.join(".app.toml"), "[cmds.test]\nfoo = \"home\"")?;
            let home_str = path_to_utf8_string(&home, "home")?;
            j.set_env("HOME", &home_str);
            #[cfg(windows)]
            j.set_env("USERPROFILE", &home_str);
            Ok(())
        },
        &CmdCfg::default(),
    )
    .map_err(|err| anyhow!(err))?;
    ensure!(
        cfg.foo.as_deref() == Some("home"),
        "expected home, got {:?}",
        cfg.foo
    );
    Ok(())
}

#[test]
fn local_overrides_home() -> Result<()> {
    let cfg: CmdCfg = with_merged_subcommand_cli(
        |j| {
            let home = j.create_dir("home")?;
            j.create_file(home.join(".app.toml"), "[cmds.test]\nfoo = \"home\"")?;
            let home_str = path_to_utf8_string(&home, "home")?;
            j.set_env("HOME", &home_str);
            #[cfg(windows)]
            j.set_env("USERPROFILE", &home_str);
            j.create_file(".app.toml", "[cmds.test]\nfoo = \"local\"")?;
            Ok(())
        },
        &CmdCfg::default(),
    )
    .map_err(|err| anyhow!(err))?;
    ensure!(
        cfg.foo.as_deref() == Some("local"),
        "expected local, got {:?}",
        cfg.foo
    );
    Ok(())
}

// Windows lacks XDG support.
#[cfg(any(unix, target_os = "redox"))]
#[test]
fn loads_from_xdg_config() -> Result<()> {
    let cfg: CmdCfg = with_merged_subcommand_cli(
        |j| {
            let xdg = j.create_dir("xdg")?;
            let abs = ortho_config::file::canonicalise(&xdg)
                .map_err(|err| FigmentError::from(err.to_string()))?;
            j.create_dir(abs.join("app"))?;
            j.create_file(abs.join("app/config.toml"), "[cmds.test]\nfoo = \"xdg\"")?;
            let xdg_path = path_to_utf8_string(&abs, "xdg config")?;
            j.set_env("XDG_CONFIG_HOME", &xdg_path);
            Ok(())
        },
        &CmdCfg::default(),
    )
    .map_err(|err| anyhow!(err))?;
    ensure!(
        cfg.foo.as_deref() == Some("xdg"),
        "expected xdg, got {:?}",
        cfg.foo
    );
    Ok(())
}

#[cfg(feature = "yaml")]
#[test]
fn loads_yaml_file() -> Result<()> {
    let cfg: CmdCfg = with_merged_subcommand_cli(
        |j| {
            j.create_file(".app.yml", "cmds:\n  test:\n    foo: yaml")?;
            Ok(())
        },
        &CmdCfg::default(),
    )
    .map_err(|err| anyhow!(err))?;
    ensure!(
        cfg.foo.as_deref() == Some("yaml"),
        "expected yaml, got {:?}",
        cfg.foo
    );
    Ok(())
}
