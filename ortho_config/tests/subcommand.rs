//! Tests for subcommand configuration helpers.

#![allow(non_snake_case)]
#![allow(deprecated)]
//! Utilities for subcommand test setup and loading.
mod util;

use clap::Parser;
use ortho_config::OrthoConfig;
use serde::{Deserialize, Serialize};
use util::{with_merged_subcommand_cli, with_subcommand_config, with_typed_subcommand_config};

#[derive(Debug, Deserialize, Default, PartialEq)]
struct CmdCfg {
    foo: Option<String>,
    bar: Option<bool>,
}

#[test]
fn file_and_env_loading() {
    let cfg: CmdCfg = with_subcommand_config(|j| {
        j.create_file(".app.toml", "[cmds.test]\nfoo = \"file\"\nbar = true")?;
        j.set_env("APP_CMDS_TEST_FOO", "env");
        Ok(())
    })
    .expect("config");
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
    })
    .expect("config");
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
    })
    .expect("config");
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
    })
    .expect("config");
    assert_eq!(cfg.foo.as_deref(), Some("xdg"));
}

#[derive(Debug, Deserialize, Serialize, Parser, OrthoConfig, Default, PartialEq)]
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
    })
    .expect("config");
    assert_eq!(cfg.foo.as_deref(), Some("env"));
}

#[cfg(feature = "yaml")]
#[test]
fn loads_yaml_file() {
    let cfg: CmdCfg = with_subcommand_config(|j| {
        j.create_file(".app.yml", "cmds:\n  test:\n    foo: yaml")?;
        Ok(())
    })
    .expect("config");
    assert_eq!(cfg.foo.as_deref(), Some("yaml"));
}

#[derive(Debug, Deserialize, serde::Serialize, Default, PartialEq, Parser, OrthoConfig, Clone)]
#[command(name = "test")]
#[ortho_config(prefix = "APP_")]
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

#[test]
fn merge_wrapper_respects_prefix() {
    let cli = MergeArgs {
        foo: None,
        bar: None,
    };
    let merged = with_merged_subcommand_cli(
        |j| {
            j.create_file(".app.toml", "[cmds.test]\nfoo = \"file\"")?;
            Ok(())
        },
        &cli,
    )
    .expect("merge");
    assert_eq!(merged.foo.as_deref(), Some("file"));
}

#[derive(Debug, Deserialize, serde::Serialize, Parser, Default, PartialEq, OrthoConfig, Clone)]
#[command(name = "test")]
#[ortho_config(prefix = "APP_")]
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
#[derive(Debug, Deserialize, Default, PartialEq)]
struct NestedCfg {
    #[serde(default)]
    nested: Nested,
}

#[derive(Debug, Deserialize, Default, PartialEq)]
struct Nested {
    host: Option<String>,
    port: Option<u16>,
}

#[derive(Debug, Deserialize, Default, PartialEq)]
struct DeepNestedCfg {
    #[serde(default)]
    deep: DeepLevel,
}

#[derive(Debug, Deserialize, Default, PartialEq)]
struct DeepLevel {
    #[serde(default)]
    nest: DeepNest,
}

#[derive(Debug, Deserialize, Default, PartialEq)]
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
    let cfg: NestedCfg = with_subcommand_config(|j| {
        if let Some((k, v)) = host_kv {
            j.set_env(k, v);
        }
        if let Some((k, v)) = port_kv {
            j.set_env(k, v);
        }
        Ok(())
    })
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
    let cfg: DeepNestedCfg = with_subcommand_config(|j| {
        if let Some((k, v)) = kv {
            j.set_env(k, v);
        }
        Ok(())
    })
    .expect("config");
    assert_eq!(cfg.deep.nest.host.as_deref(), expect_host);
}
