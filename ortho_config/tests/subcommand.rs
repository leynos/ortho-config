#![allow(non_snake_case)]
#![allow(deprecated)]
use ortho_config::OrthoConfig;
use ortho_config::load_subcommand_config;
use ortho_config::load_subcommand_config_for;
use ortho_config::{load_and_merge_subcommand, load_and_merge_subcommand_for};
use serde::Deserialize;

#[derive(Debug, Deserialize, Default, PartialEq)]
struct CmdCfg {
    foo: Option<String>,
    bar: Option<bool>,
}

#[test]
fn file_and_env_loading() {
    figment::Jail::expect_with(|j| {
        j.create_file(".app.toml", "[cmds.test]\nfoo = \"file\"\nbar = true")?;
        j.set_env("APP_CMDS_TEST_FOO", "env");
        let cfg: CmdCfg = load_subcommand_config("APP_", "test").expect("load");
        assert_eq!(cfg.foo.as_deref(), Some("env"));
        assert_eq!(cfg.bar, Some(true));
        Ok(())
    });
}

#[test]
fn loads_from_home() {
    figment::Jail::expect_with(|j| {
        let home = j.create_dir("home")?;
        j.create_file(home.join(".app.toml"), "[cmds.test]\nfoo = \"home\"")?;
        j.set_env("HOME", home.to_str().unwrap());
        let cfg: CmdCfg = load_subcommand_config("APP_", "test").expect("load");
        assert_eq!(cfg.foo.as_deref(), Some("home"));
        Ok(())
    });
}

#[test]
fn local_overrides_home() {
    figment::Jail::expect_with(|j| {
        let home = j.create_dir("home")?;
        j.create_file(home.join(".app.toml"), "[cmds.test]\nfoo = \"home\"")?;
        j.set_env("HOME", home.to_str().unwrap());
        j.create_file(".app.toml", "[cmds.test]\nfoo = \"local\"")?;
        let cfg: CmdCfg = load_subcommand_config("APP_", "test").expect("load");
        assert_eq!(cfg.foo.as_deref(), Some("local"));
        Ok(())
    });
}

#[test]
fn loads_from_xdg_config() {
    figment::Jail::expect_with(|j| {
        let xdg = j.create_dir("xdg")?;
        let abs = std::fs::canonicalize(&xdg).unwrap();
        j.create_dir(abs.join("app"))?;
        j.create_file(abs.join("app/config.toml"), "[cmds.test]\nfoo = \"xdg\"")?;
        j.set_env("XDG_CONFIG_HOME", abs.to_str().unwrap());
        let cfg: CmdCfg = load_subcommand_config("APP_", "test").expect("load");
        assert_eq!(cfg.foo.as_deref(), Some("xdg"));
        Ok(())
    });
}

#[derive(Debug, Deserialize, OrthoConfig, Default, PartialEq)]
#[allow(non_snake_case)]
#[ortho_config(prefix = "APP_")]
struct PrefixedCfg {
    foo: Option<String>,
}

#[test]
fn wrapper_uses_struct_prefix() {
    figment::Jail::expect_with(|j| {
        j.create_file(".app.toml", "[cmds.test]\nfoo = \"val\"")?;
        j.set_env("APP_CMDS_TEST_FOO", "env");
        let cfg: PrefixedCfg = load_subcommand_config_for("test").expect("load");
        assert_eq!(cfg.foo.as_deref(), Some("env"));
        Ok(())
    });
}

#[cfg(feature = "yaml")]
#[test]
fn loads_yaml_file() {
    figment::Jail::expect_with(|j| {
        j.create_file(".app.yml", "cmds:\n  test:\n    foo: yaml")?;
        let cfg: CmdCfg = load_subcommand_config("APP_", "test").expect("load");
        assert_eq!(cfg.foo.as_deref(), Some("yaml"));
        Ok(())
    });
}

#[derive(Debug, Deserialize, serde::Serialize, Default, PartialEq)]
struct MergeArgs {
    #[serde(skip_serializing_if = "Option::is_none")]
    foo: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    bar: Option<bool>,
}

#[test]
fn merge_helper_combines_defaults_and_cli() {
    figment::Jail::expect_with(|j| {
        j.create_file(".app.toml", "[cmds.test]\nfoo = \"file\"")?;
        let cli = MergeArgs {
            foo: Some("cli".into()),
            bar: None,
        };
        let merged: MergeArgs = load_and_merge_subcommand("APP_", "test", &cli).expect("merge");
        assert_eq!(merged.foo.as_deref(), Some("cli"));
        assert_eq!(merged.bar, None);
        Ok(())
    });
}

#[derive(Debug, Deserialize, serde::Serialize, OrthoConfig, Default, PartialEq)]
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
        let merged = load_and_merge_subcommand_for::<MergePrefixed>("test", &cli).expect("merge");
        assert_eq!(merged.foo.as_deref(), Some("file"));
        Ok(())
    });
}
