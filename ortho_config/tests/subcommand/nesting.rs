//! Nested environment-key mapping tests for subcommand loading.

use anyhow::{Result, anyhow, ensure};
use clap::Parser;
use serde::{Deserialize, Serialize};

use super::util::with_merged_subcommand_cli;

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
) -> Result<()> {
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
    .map_err(|err| anyhow!(err))?;
    ensure!(
        cfg.nested.host.as_deref() == expect_host,
        "expected host {:?}, got {:?}",
        expect_host,
        cfg.nested.host
    );
    ensure!(
        cfg.nested.port == expect_port,
        "expected port {:?}, got {:?}",
        expect_port,
        cfg.nested.port
    );
    Ok(())
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
) -> Result<()> {
    let cfg: DeepNestedCfg = with_merged_subcommand_cli(
        |j| {
            if let Some((k, v)) = kv {
                j.set_env(k, v);
            }
            Ok(())
        },
        &DeepNestedCfg::default(),
    )
    .map_err(|err| anyhow!(err))?;
    ensure!(
        cfg.deep.nest.host.as_deref() == expect_host,
        "expected host {:?}, got {:?}",
        expect_host,
        cfg.deep.nest.host
    );
    Ok(())
}
