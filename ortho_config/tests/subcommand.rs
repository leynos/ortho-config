//! Tests for subcommand configuration helpers.
#![expect(
    clippy::expect_used,
    reason = "tests panic to surface configuration mistakes"
)]

//! Utilities for subcommand test setup and loading.
mod util;

use clap::Parser;
use ortho_config::OrthoConfig;
use serde::{Deserialize, Serialize};
use util::{with_merged_subcommand_cli, with_merged_subcommand_cli_for};

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
fn file_and_env_loading() {
    let cfg: CmdCfg = with_merged_subcommand_cli(
        |j| {
            j.create_file(".app.toml", "[cmds.test]\nfoo = \"file\"\nbar = true")?;
            j.set_env("APP_CMDS_TEST_FOO", "env");
            Ok(())
        },
        &CmdCfg::default(),
    )
    .expect("config");
    assert_eq!(cfg.foo.as_deref(), Some("env"));
    assert_eq!(cfg.bar, Some(true));
}

#[test]
fn loads_from_home() {
    let cfg: CmdCfg = with_merged_subcommand_cli(
        |j| {
            let home = j.create_dir("home")?;
            j.create_file(home.join(".app.toml"), "[cmds.test]\nfoo = \"home\"")?;
            j.set_env("HOME", home.to_str().expect("home path utf-8"));
            #[cfg(windows)]
            j.set_env(
                "USERPROFILE",
                home.to_str().expect("user profile path utf-8"),
            );
            Ok(())
        },
        &CmdCfg::default(),
    )
    .expect("config");
    assert_eq!(cfg.foo.as_deref(), Some("home"));
}

#[test]
fn local_overrides_home() {
    let cfg: CmdCfg = with_merged_subcommand_cli(
        |j| {
            let home = j.create_dir("home")?;
            j.create_file(home.join(".app.toml"), "[cmds.test]\nfoo = \"home\"")?;
            j.set_env("HOME", home.to_str().expect("home path utf-8"));
            #[cfg(windows)]
            j.set_env(
                "USERPROFILE",
                home.to_str().expect("user profile path utf-8"),
            );
            j.create_file(".app.toml", "[cmds.test]\nfoo = \"local\"")?;
            Ok(())
        },
        &CmdCfg::default(),
    )
    .expect("config");
    assert_eq!(cfg.foo.as_deref(), Some("local"));
}

// Windows lacks XDG support
#[cfg(any(unix, target_os = "redox"))]
#[test]
fn loads_from_xdg_config() {
    let cfg: CmdCfg = with_merged_subcommand_cli(
        |j| {
            let xdg = j.create_dir("xdg")?;
            let abs = ortho_config::file::canonicalise(&xdg).expect("canonicalise xdg dir");
            j.create_dir(abs.join("app"))?;
            j.create_file(abs.join("app/config.toml"), "[cmds.test]\nfoo = \"xdg\"")?;
            j.set_env("XDG_CONFIG_HOME", abs.to_str().expect("xdg dir to string"));
            Ok(())
        },
        &CmdCfg::default(),
    )
    .expect("config");
    assert_eq!(cfg.foo.as_deref(), Some("xdg"));
}

#[derive(Debug, Deserialize, Serialize, Parser, OrthoConfig, Default, PartialEq)]
#[ortho_config(prefix = "APP_")]
#[command(name = "test")]
struct PrefixedCfg {
    foo: Option<String>,
}

#[test]
fn wrapper_uses_struct_prefix() {
    let cfg: PrefixedCfg = with_merged_subcommand_cli_for(
        |j| {
            j.create_file(".app.toml", "[cmds.test]\nfoo = \"val\"")?;
            j.set_env("APP_CMDS_TEST_FOO", "env");
            Ok(())
        },
        &PrefixedCfg::default(),
    )
    .expect("config");
    assert_eq!(cfg.foo.as_deref(), Some("env"));
}

#[cfg(feature = "yaml")]
#[test]
fn loads_yaml_file() {
    let cfg: CmdCfg = with_merged_subcommand_cli(
        |j| {
            j.create_file(".app.yml", "cmds:\n  test:\n    foo: yaml")?;
            Ok(())
        },
        &CmdCfg::default(),
    )
    .expect("config");
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
    let cli = MergeArgs {
        foo: Some("cli".into()),
        bar: None,
    };
    let merged: MergeArgs = with_merged_subcommand_cli(
        |j| {
            j.create_file(".app.toml", "[cmds.test]\nfoo = \"file\"")?;
            Ok(())
        },
        &cli,
    )
    .expect("merge");
    assert_eq!(merged.foo.as_deref(), Some("cli"));
    assert_eq!(merged.bar, None);
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
    let cli = MergePrefixed { foo: None };
    let merged = with_merged_subcommand_cli_for(
        |j| {
            j.create_file(".app.toml", "[cmds.test]\nfoo = \"file\"")?;
            Ok(())
        },
        &cli,
    )
    .expect("merge");
    assert_eq!(merged.foo.as_deref(), Some("file"));
}

#[derive(Debug, Deserialize, serde::Serialize, Parser, Default, PartialEq)]
#[command(name = "test")]
struct RequiredCli {
    #[arg(long, required = true)]
    #[serde(skip_serializing_if = "Option::is_none")]
    ref_id: Option<String>,
}

#[rstest::rstest]
fn cli_only_values_are_accepted() {
    let cli = RequiredCli {
        ref_id: Some("cli".into()),
    };
    let merged: RequiredCli = with_merged_subcommand_cli(|_j| Ok(()), &cli).expect("merge");
    assert_eq!(merged.ref_id.as_deref(), Some("cli"));
}

#[test]
fn error_when_required_cli_value_missing() {
    let result = RequiredCli::try_parse_from(["test"]);
    assert!(
        result.is_err(),
        "parsing should fail without required value"
    );
}

#[test]
fn conflicting_values_cli_takes_precedence() {
    let cli = RequiredCli {
        ref_id: Some("cli".into()),
    };
    let merged: RequiredCli = with_merged_subcommand_cli(
        |j| {
            j.create_file(".app.toml", "[cmds.test]\nref_id = \"config\"")?;
            j.set_env("APP_CMDS_TEST_REF_ID", "env");
            Ok(())
        },
        &cli,
    )
    .expect("merge");
    assert_eq!(merged.ref_id.as_deref(), Some("cli"));
}

#[test]
fn env_value_used_when_cli_missing() {
    let cli = RequiredCli { ref_id: None };
    let merged: RequiredCli = with_merged_subcommand_cli(
        |j| {
            j.set_env("APP_CMDS_TEST_REF_ID", "from-env");
            Ok(())
        },
        &cli,
    )
    .expect("merge");
    assert_eq!(merged.ref_id.as_deref(), Some("from-env"));
}
#[derive(Debug, Deserialize, Serialize, Default, PartialEq, Parser)]
#[command(name = "test")]
struct NestedCfg {
    #[serde(default)]
    #[arg(skip)]
    nested: Nested,
}

#[derive(Debug, Deserialize, Serialize, Default, PartialEq)]
struct Nested {
    host: Option<String>,
    port: Option<u16>,
}

#[derive(Debug, Deserialize, Serialize, Default, PartialEq, Parser)]
#[command(name = "test")]
struct DeepNestedCfg {
    #[serde(default)]
    #[arg(skip)]
    deep: DeepLevel,
}

#[derive(Debug, Deserialize, Serialize, Default, PartialEq)]
struct DeepLevel {
    #[serde(default)]
    nest: DeepNest,
}

#[derive(Debug, Deserialize, Serialize, Default, PartialEq)]
struct DeepNest {
    host: Option<String>,
}

/// Environment variables with double underscores map to nested fields, and
/// defaults apply when values are absent.
#[rstest::rstest]
#[case(
    Some(("APP_CMDS_TEST_NESTED__HOST", "env")),
    Some(("APP_CMDS_TEST_NESTED__PORT", "8080")),
    Some("env"),
    Some(8080u16)
)]
#[case(None, None, None, None)]
fn env_values_support_nesting_cases(
    #[case] host_kv: Option<(&str, &str)>,
    #[case] port_kv: Option<(&str, &str)>,
    #[case] expect_host: Option<&str>,
    #[case] expect_port: Option<u16>,
) {
    let cfg: NestedCfg = with_merged_subcommand_cli(
        |j| {
            if let Some((k, v)) = host_kv {
                j.set_env(k, v);
            }
            if let Some((k, v)) = port_kv {
                j.set_env(k, v);
            }
            Ok(())
        },
        &NestedCfg::default(),
    )
    .expect("config");
    assert_eq!(cfg.nested.host.as_deref(), expect_host);
    assert_eq!(cfg.nested.port, expect_port);
}

/// Tests multi-level splitting of environment variable keys.
///
/// # Examples
///
/// ```
/// env_values_support_deeper_nesting(
///     Some(("APP_CMDS_TEST_DEEP__NEST__HOST", "deep")),
///     Some("deep"),
/// );
/// env_values_support_deeper_nesting(None, None);
/// ```
#[rstest::rstest]
#[case(Some(("APP_CMDS_TEST_DEEP__NEST__HOST", "deep")), Some("deep"))]
#[case(None, None)]
fn env_values_support_deeper_nesting(
    #[case] kv: Option<(&str, &str)>,
    #[case] expect_host: Option<&str>,
) {
    let cfg: DeepNestedCfg = with_merged_subcommand_cli(
        |j| {
            if let Some((k, v)) = kv {
                j.set_env(k, v);
            }
            Ok(())
        },
        &DeepNestedCfg::default(),
    )
    .expect("config");
    assert_eq!(cfg.deep.nest.host.as_deref(), expect_host);
}
