//! Tests ensuring flattened CLI structs merge without overriding defaults.
use anyhow::{Result, anyhow, ensure};
use figment::{Figment, providers::Serialized};
use ortho_config::sanitized_provider;
use rstest::rstest;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Default, PartialEq)]
struct Inner {
    val: Option<u32>,
}

#[derive(Debug, Serialize, Deserialize, Default, PartialEq)]
struct Outer {
    inner: Inner,
}

fn merge(defaults: &Outer, cli: &Outer) -> Result<Outer> {
    let sanitized = sanitized_provider(cli).map_err(|err| anyhow!(err))?;
    let merged = Figment::from(Serialized::defaults(defaults))
        .merge(sanitized)
        .extract()
        .map_err(|err| anyhow!(err))?;
    Ok(merged)
}

#[rstest]
fn empty_flatten_like_struct_preserves_defaults() -> Result<()> {
    let defaults = Outer {
        inner: Inner { val: Some(7) },
    };
    let cli = Outer::default();
    let merged = merge(&defaults, &cli)?;
    ensure!(
        merged == defaults,
        "expected defaults {defaults:?}, got {merged:?}"
    );
    Ok(())
}
