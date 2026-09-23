//! Nested environment-key mapping tests for subcommand loading.

use anyhow::{Context as _, Result, ensure};
use clap::Parser;
use ortho_config::subcommand::Prefix;
use ortho_config::{MapEnv, SubcommandFileContext, load_and_merge_subcommand_with_sources_at};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

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
    let root = tempfile::tempdir().context("create nested environment fixture")?;
    let merge: MapEnv = host_kv.into_iter().chain(port_kv).collect();
    #[cfg(any(unix, target_os = "redox"))]
    let discovery = MapEnv::new().with_var("XDG_CONFIG_DIRS", root.path());
    #[cfg(not(any(unix, target_os = "redox")))]
    let discovery = MapEnv::new();
    let cfg = load_and_merge_subcommand_with_sources_at(
        &Prefix::new("APP_"),
        &NestedCfg::default(),
        SubcommandFileContext::new(root.path(), &discovery),
        Arc::new(merge),
    )
    .context("merge nested injected environment defaults")?;
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
    let root = tempfile::tempdir().context("create deep nested environment fixture")?;
    let merge: MapEnv = kv.into_iter().collect();
    #[cfg(any(unix, target_os = "redox"))]
    let discovery = MapEnv::new().with_var("XDG_CONFIG_DIRS", root.path());
    #[cfg(not(any(unix, target_os = "redox")))]
    let discovery = MapEnv::new();
    let cfg = load_and_merge_subcommand_with_sources_at(
        &Prefix::new("APP_"),
        &DeepNestedCfg::default(),
        SubcommandFileContext::new(root.path(), &discovery),
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
