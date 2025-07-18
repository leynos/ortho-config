//! Tests for subcommand configuration helpers.

#![allow(non_snake_case)]
#![allow(deprecated)]
mod util;

use clap::Parser;
use ortho_config::subcommand::Prefix;
use ortho_config::{OrthoConfig, load_and_merge_subcommand, load_and_merge_subcommand_for};
use serde::Deserialize;
use util::{with_subcommand_config, with_typed_subcommand_config};

#[derive(Debug, Deserialize, Default, PartialEq)]
struct CmdCfg {
    foo: Option<String>,
    bar: Option<bool>,
}

/// Loads the `CmdCfg` configuration for the "test" subcommand after applying a custom setup to a jailed environment.
///
/// The provided closure is used to configure the environment (such as creating files or setting environment variables) within a `figment::Jail`. The function then loads the configuration for the "test" subcommand using the "APP_" prefix, returning the resulting `CmdCfg`.
///
/// # Panics
///
/// Panics if configuration loading fails.
///
/// # Examples
///
/// ```
/// let cfg = with_subcommand_config(|jail| {
///     jail.create_file(".app.toml", "[cmds.test]\nfoo = \"bar\"")?;
///     Ok(())
/// });
/// assert_eq!(cfg.foo, Some("bar".to_string()));
/// ```

#[test]
fn file_and_env_loading() {
    let cfg: CmdCfg = with_subcommand_config(|j| {
        j.create_file(".app.toml", "[cmds.test]\nfoo = \"file\"\nbar = true")?;
        j.set_env("APP_CMDS_TEST_FOO", "env");
        Ok(())
    });
    assert_eq!(cfg.foo.as_deref(), Some("env"));
    assert_eq!(cfg.bar, Some(true));
}

#[test]
fn loads_from_home() {
    let cfg: CmdCfg = with_subcommand_config(|j| {
        let home = j.create_dir("home")?;
        j.create_file(home.join(".app.toml"), "[cmds.test]\nfoo = \"home\"")?;
        j.set_env("HOME", home.to_str().unwrap());
        #[cfg(windows)]
        j.set_env("USERPROFILE", home.to_str().unwrap());
        Ok(())
    });
    assert_eq!(cfg.foo.as_deref(), Some("home"));
}

#[test]
fn local_overrides_home() {
    let cfg: CmdCfg = with_subcommand_config(|j| {
        let home = j.create_dir("home")?;
        j.create_file(home.join(".app.toml"), "[cmds.test]\nfoo = \"home\"")?;
        j.set_env("HOME", home.to_str().unwrap());
        #[cfg(windows)]
        j.set_env("USERPROFILE", home.to_str().unwrap());
        j.create_file(".app.toml", "[cmds.test]\nfoo = \"local\"")?;
        Ok(())
    });
    assert_eq!(cfg.foo.as_deref(), Some("local"));
}

// Windows lacks XDG support
#[cfg(any(unix, target_os = "redox"))]
#[test]
fn loads_from_xdg_config() {
    let cfg: CmdCfg = with_subcommand_config(|j| {
        let xdg = j.create_dir("xdg")?;
        let abs = std::fs::canonicalize(&xdg).unwrap();
        j.create_dir(abs.join("app"))?;
        j.create_file(abs.join("app/config.toml"), "[cmds.test]\nfoo = \"xdg\"")?;
        j.set_env("XDG_CONFIG_HOME", abs.to_str().unwrap());
        Ok(())
    });
    assert_eq!(cfg.foo.as_deref(), Some("xdg"));
}

#[derive(Debug, Deserialize, OrthoConfig, Default, PartialEq)]
#[allow(non_snake_case)]
#[ortho_config(prefix = "APP_")]
struct PrefixedCfg {
    foo: Option<String>,
}

#[test]
fn wrapper_uses_struct_prefix() {
    let cfg: PrefixedCfg = with_typed_subcommand_config(|j| {
        j.create_file(".app.toml", "[cmds.test]\nfoo = \"val\"")?;
        j.set_env("APP_CMDS_TEST_FOO", "env");
        Ok(())
    });
    assert_eq!(cfg.foo.as_deref(), Some("env"));
}

#[cfg(feature = "yaml")]
#[test]
fn loads_yaml_file() {
    let cfg: CmdCfg = with_subcommand_config(|j| {
        j.create_file(".app.yml", "cmds:\n  test:\n    foo: yaml")?;
        Ok(())
    });
    assert_eq!(cfg.foo.as_deref(), Some("yaml"));
}

#[derive(Debug, Deserialize, serde::Serialize, Default, PartialEq, Parser)]
#[command(name = "test")]
struct MergeArgs {
    #[serde(skip_serializing_if = "Option::is_none")]
    foo: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    bar: Option<bool>,
}

#[test]
/// Tests that merging CLI arguments with configuration file values prioritises CLI values and preserves unset fields.
///
/// This test creates a configuration file with a default value for `foo`, then merges it with CLI arguments that override `foo` and leave `bar` unset. It asserts that the merged configuration uses the CLI value for `foo` and retains `None` for `bar`.
///
/// # Examples
///
/// ```
/// merge_helper_combines_defaults_and_cli();
/// ```
fn merge_helper_combines_defaults_and_cli() {
    figment::Jail::expect_with(|j| {
        j.create_file(".app.toml", "[cmds.test]\nfoo = \"file\"")?;
        let cli = MergeArgs {
            foo: Some("cli".into()),
            bar: None,
        };
        let merged: MergeArgs =
            load_and_merge_subcommand(&Prefix::new("APP_"), &cli).expect("merge");
        assert_eq!(merged.foo.as_deref(), Some("cli"));
        assert_eq!(merged.bar, None);
        Ok(())
    });
}

#[derive(Debug, Deserialize, serde::Serialize, OrthoConfig, Default, PartialEq, Parser)]
#[command(name = "test")]
#[ortho_config(prefix = "APP_")]
struct MergePrefixed {
    #[serde(skip_serializing_if = "Option::is_none")]
    foo: Option<String>,
}

#[test]
fn merge_wrapper_respects_prefix() {
    figment::Jail::expect_with(|j| {
        j.create_file(".app.toml", "[cmds.test]\nfoo = \"file\"")?;
        let cli = MergePrefixed { foo: None };
        let merged = load_and_merge_subcommand_for::<MergePrefixed>(&cli).expect("merge");
        assert_eq!(merged.foo.as_deref(), Some("file"));
        Ok(())
    });
}
