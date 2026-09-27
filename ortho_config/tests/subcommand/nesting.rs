//! Nested environment-key mapping tests for subcommand loading.

use anyhow::{Context as _, Result, ensure};
use clap::Parser;
use ortho_config::subcommand::Prefix;
use ortho_config::{MapEnv, SubcommandFileContext, load_and_merge_subcommand_with_sources_at};
use rstest::rstest;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use super::fixtures::{IsolatedRoot, isolated_root};

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

/// One nested-key case: the environment entries to inject, and what the
/// loader should make of them.
///
/// The inputs and expectations travel together so `rstest` sees a single case
/// argument; splitting them would push the test past Clippy's argument limit
/// once the `isolated_root` fixture is added.
struct NestedEnvCase {
    host_kv: Option<(&'static str, &'static str)>,
    port_kv: Option<(&'static str, &'static str)>,
    expect_host: Option<&'static str>,
    expect_port: Option<u16>,
}

/// Environment variables with double underscores map to nested fields, and
/// defaults apply when values are absent.
#[rstest]
#[case::both_variables_present(NestedEnvCase {
    host_kv: Some(("APP_CMDS_TEST_NESTED__HOST", "env")),
    port_kv: Some(("APP_CMDS_TEST_NESTED__PORT", "8080")),
    expect_host: Some("env"),
    expect_port: Some(8080u16),
})]
#[case::neither_variable_present(NestedEnvCase {
    host_kv: None,
    port_kv: None,
    expect_host: None,
    expect_port: None,
})]
fn env_values_support_nesting_cases(
    #[case] case: NestedEnvCase,
    isolated_root: Result<IsolatedRoot>,
) -> Result<()> {
    let isolated = isolated_root?;
    let discovery = isolated.discovery();
    let merge: MapEnv = case.host_kv.into_iter().chain(case.port_kv).collect();
    let cfg = load_and_merge_subcommand_with_sources_at(
        &Prefix::new("APP_"),
        &NestedCfg::default(),
        SubcommandFileContext::new(isolated.path(), &discovery),
        Arc::new(merge),
    )
    .context("merge nested injected environment defaults")?;
    ensure!(
        cfg.nested.host.as_deref() == case.expect_host,
        "expected host {:?}, got {:?}",
        case.expect_host,
        cfg.nested.host
    );
    ensure!(
        cfg.nested.port == case.expect_port,
        "expected port {:?}, got {:?}",
        case.expect_port,
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
#[rstest]
#[case(Some(("APP_CMDS_TEST_DEEP__NEST__HOST", "deep")), Some("deep"))]
#[case(None, None)]
fn env_values_support_deeper_nesting(
    #[case] kv: Option<(&str, &str)>,
    #[case] expect_host: Option<&str>,
    isolated_root: Result<IsolatedRoot>,
) -> Result<()> {
    let isolated = isolated_root?;
    let discovery = isolated.discovery();
    let merge: MapEnv = kv.into_iter().collect();
    let cfg = load_and_merge_subcommand_with_sources_at(
        &Prefix::new("APP_"),
        &DeepNestedCfg::default(),
        SubcommandFileContext::new(isolated.path(), &discovery),
        Arc::new(merge),
    )
    .context("merge deeply nested injected environment defaults")?;
    ensure!(
        cfg.deep.nest.host.as_deref() == expect_host,
        "expected host {:?}, got {:?}",
        expect_host,
        cfg.deep.nest.host
    );
    Ok(())
}
