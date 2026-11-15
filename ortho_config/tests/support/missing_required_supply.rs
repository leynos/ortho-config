//! Supply-route truthfulness observed through real generated loaders.
//!
//! Split from `missing_required_values.rs` to keep both files within the
//! repository's 400-line ceiling. Each case loads a real configuration with
//! values a user could actually set and asserts that the reported routes name
//! inputs the loader really reads:
//!
//! - an optional flattened composition reports nothing, because Serde resolves
//!   an all-absent child to absence rather than erroring;
//! - a key that cannot survive the environment provider's case and separator
//!   transforms gets no environment route, while a plain sibling still does;
//! - a nested requirement reached through an alias repeats the observed
//!   spelling, so supplying the advertised variable or file key succeeds.

use anyhow::{Result, ensure};
use ortho_config::OrthoConfig;
use serde::{Deserialize, Serialize};

use super::{PROGRAM, load, paths, report, routes_for};

/// A child requirement reached only by flattening.
#[derive(Debug, Deserialize, Serialize, ortho_config::RequiredConfigSchema)]
struct FlattenedChild {
    /// Required leaf with no default.
    host: String,
}

/// A root whose flattened child tolerates absence.
#[derive(Debug, Deserialize, Serialize, OrthoConfig)]
#[ortho_config(prefix = "MRF_")]
struct OptionalFlattenRoot {
    /// An absent optional flatten is absence, not a missing required leaf.
    #[serde(flatten)]
    #[ortho_config(nested, skip_cli)]
    extra: Option<FlattenedChild>,
}

/// A root whose flattened child is required.
#[derive(Debug, Deserialize, Serialize, OrthoConfig)]
#[ortho_config(prefix = "MRG_")]
struct RequiredFlattenRoot {
    /// The child's leaves compose at this level, so no wrapper key exists.
    #[serde(flatten)]
    #[ortho_config(nested, skip_cli)]
    extra: FlattenedChild,
}

/// A configuration whose key spellings cannot round-trip the provider.
#[derive(Debug, Deserialize, Serialize, OrthoConfig)]
#[ortho_config(prefix = "MRE_")]
struct InexpressibleEnvRoot {
    /// A literal dot nests under the provider instead of naming one key.
    #[serde(rename = "Host.Name")]
    dotted: String,
    /// The provider lowercases the remainder, so this spelling is unreachable.
    #[serde(rename = "HostName")]
    cased: String,
    /// A plain control key that does have a truthful environment route.
    plain: String,
}

/// A nested child with one required and one optional leaf.
#[derive(Debug, Deserialize, Serialize, ortho_config::RequiredConfigSchema)]
struct AliasDatabase {
    /// Required leaf the report must name.
    host: String,
    /// Optional leaf used to create a partially supplied object.
    #[serde(default)]
    port: u16,
}

/// A root whose nested object is reachable under an alias.
#[derive(Debug, Deserialize, Serialize, OrthoConfig)]
#[ortho_config(prefix = "MRA_")]
struct AliasNestedRoot {
    /// The merged value may spell this key `db`, which Serde accepts.
    #[serde(alias = "db")]
    #[ortho_config(nested, skip_cli)]
    database: AliasDatabase,
}

#[test]
fn an_absent_optional_flatten_reports_nothing() -> Result<()> {
    let config = load::<OptionalFlattenRoot>(&[PROGRAM], &[]).map_err(|error| {
        anyhow::anyhow!("an optional flatten must not require its leaves: {error}")
    })?;
    ensure!(
        config.extra.is_none(),
        "the absent optional flatten must resolve to `None`"
    );
    Ok(())
}

#[test]
fn a_required_flatten_composes_at_the_current_level() -> Result<()> {
    let missing = report(load::<RequiredFlattenRoot>(&[PROGRAM], &[]))?;
    ensure!(
        paths(&missing) == ["host"],
        "the child leaf composes at the root, got {:?}",
        paths(&missing)
    );
    let routes = routes_for(&missing, "host")?;
    ensure!(
        routes.iter().any(|route| route == "env:MRG_HOST"),
        "expected the composed environment route, got {routes:?}"
    );
    let config = load::<RequiredFlattenRoot>(&[PROGRAM], &[("MRG_HOST", "db")])
        .map_err(|error| anyhow::anyhow!("the composed route must recover: {error}"))?;
    ensure!(
        config.extra.host == "db",
        "the recovered value must reach the flattened child"
    );
    Ok(())
}

#[test]
fn keys_the_provider_cannot_express_get_no_environment_route() -> Result<()> {
    let missing = report(load::<InexpressibleEnvRoot>(&[PROGRAM], &[]))?;
    ensure!(
        paths(&missing) == [r#""Host.Name""#, "HostName", "plain"],
        "unexpected report order: {:?}",
        paths(&missing)
    );
    for path in [r#""Host.Name""#, "HostName"] {
        let routes = routes_for(&missing, path)?;
        ensure!(
            !routes.iter().any(|route| route.starts_with("env:")),
            "`{path}` cannot be set through the provider, got {routes:?}"
        );
        ensure!(
            routes.iter().any(|route| route.starts_with("file:")),
            "`{path}` is still settable in a file, got {routes:?}"
        );
    }
    let control = routes_for(&missing, "plain")?;
    ensure!(
        control.iter().any(|route| route == "env:MRE_PLAIN"),
        "an expressible sibling keeps its environment route, got {control:?}"
    );
    Ok(())
}

#[test]
fn supplying_the_plain_key_drops_it_from_the_report() -> Result<()> {
    let missing = report(load::<InexpressibleEnvRoot>(
        &[PROGRAM],
        &[("MRE_PLAIN", "set")],
    ))?;
    ensure!(
        paths(&missing) == [r#""Host.Name""#, "HostName"],
        "a supplied value must not be reported, got {:?}",
        paths(&missing)
    );
    Ok(())
}

#[test]
fn a_route_through_an_alias_repeats_the_observed_spelling() -> Result<()> {
    let missing = report(load::<AliasNestedRoot>(
        &[PROGRAM],
        &[("MRA_DB__PORT", "1")],
    ))?;
    ensure!(
        paths(&missing) == ["database.host"],
        "the canonical identity stays stable, got {:?}",
        paths(&missing)
    );
    let routes = routes_for(&missing, "database.host")?;
    ensure!(
        routes.iter().any(|route| route == "env:MRA_DB__HOST"),
        "the environment route follows the observed key, got {routes:?}"
    );
    ensure!(
        routes.iter().any(|route| route == "file:db.host"),
        "the file route follows the observed key, got {routes:?}"
    );
    Ok(())
}

#[test]
fn the_alias_route_recovers_the_missing_value() -> Result<()> {
    let config =
        load::<AliasNestedRoot>(&[PROGRAM], &[("MRA_DB__PORT", "1"), ("MRA_DB__HOST", "db")])
            .map_err(|error| anyhow::anyhow!("the advertised alias route must recover: {error}"))?;
    ensure!(
        config.database.host == "db" && config.database.port == 1,
        "the advertised route must supply both leaves"
    );
    Ok(())
}
