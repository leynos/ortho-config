//! V1: requiredness observed through real generated loaders.
//!
//! Each case supplies values exactly as a user would — command-line flags and
//! environment variables — and asserts which canonical paths are reported.
//! Documentation overrides are absent on purpose: they must never influence a
//! runtime decision.

use anyhow::{Result, anyhow, ensure};
use ortho_config::{MapEnv, MissingRequiredValues, OrthoConfig, OrthoError, SupplyRoute};
use rstest::rstest;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[path = "support/missing_required_supply.rs"]
mod supply;

/// The invoked program name every case uses.
const PROGRAM: &str = "missing-required-values";

/// A configuration mixing every requiredness rule the derive supports.
#[derive(Debug, Deserialize, Serialize, OrthoConfig)]
#[ortho_config(prefix = "MRQ_")]
struct Reported {
    /// Required scalar with no default.
    reference: String,
    /// Required scalar declared second, proving order is declaration order.
    region: String,
    /// Required scalar accepted under an alias.
    #[serde(alias = "alt_zone")]
    zone_alias: String,
    /// Required collection: Serde rejects a missing `Vec` without a default.
    tags: Vec<String>,
    /// Optional because an `ortho_config` default exists.
    #[ortho_config(default = 8080)]
    port: u16,
    /// Optional because a Serde default exists.
    #[serde(default)]
    note: String,
    /// Optional because the type is `Option`.
    optional: Option<String>,
}

/// A nested type that only derives the schema, never a CLI or a loader.
#[derive(Debug, Deserialize, Serialize, ortho_config::RequiredConfigSchema)]
struct Database {
    /// Required leaf inside the nested object.
    host: String,
    /// Optional leaf inside the nested object.
    #[serde(default)]
    port: u16,
}

/// A configuration whose only required value is nested.
#[derive(Debug, Deserialize, Serialize, OrthoConfig)]
#[ortho_config(prefix = "MRN_")]
struct NestedRoot {
    /// Opted into schema composition; no flag can carry an object.
    #[ortho_config(nested, skip_cli)]
    database: Database,
}

/// A configuration with an optional nested object.
#[derive(Debug, Deserialize, Serialize, OrthoConfig)]
#[ortho_config(prefix = "MRO_")]
struct OptionalNestedRoot {
    /// An absent optional object deactivates its subtree.
    #[ortho_config(nested, skip_cli)]
    database: Option<Database>,
}

/// A configuration whose required field is renamed at deserialization.
#[derive(Debug, Deserialize, Serialize, OrthoConfig)]
#[ortho_config(prefix = "MRR_")]
struct RenamedRoot {
    /// Serde renames both directions, so the CLI key no longer matches.
    #[serde(rename = "ref")]
    reference: String,
}

/// A configuration whose required field opts out of CLI parsing.
#[derive(Debug, Deserialize, Serialize, OrthoConfig)]
#[ortho_config(prefix = "MRS_")]
struct SkippedCliRoot {
    /// Required, but no flag can supply it.
    #[ortho_config(skip_cli)]
    secret: String,
}

/// A configuration supplying a field only through clap's inferred default.
#[derive(Debug, Deserialize, Serialize, OrthoConfig, clap::Parser)]
#[command(name = "inferred-default")]
#[ortho_config(prefix = "MRI_")]
struct InferredDefaultRoot {
    /// The clap default is treated as absent during merge but replayed after.
    #[arg(long, default_value_t = 8)]
    #[ortho_config(cli_default_as_absent)]
    count: u32,
}

/// A type that composes itself through an optional boxed child.
#[derive(Debug, Deserialize, Serialize, ortho_config::RequiredConfigSchema)]
struct Recursive {
    /// Required leaf that makes the recursive schema non-empty.
    name: String,
    /// Optional recursion: composing it must stop with a typed error.
    #[ortho_config(nested)]
    child: Option<Box<Self>>,
}

/// A configuration that reaches the recursive child through a loader.
#[derive(Debug, Deserialize, Serialize, OrthoConfig)]
#[ortho_config(prefix = "MRX_")]
struct RecursiveRoot {
    /// No flag can carry the object; the schema is still composed.
    #[ortho_config(nested, skip_cli)]
    recursive: Option<Recursive>,
}

/// Build an injected environment from literal variable assignments.
fn environment(vars: &[(&str, &str)]) -> Arc<MapEnv> {
    let mut env = MapEnv::new();
    for (key, value) in vars {
        env.insert(*key, *value);
    }
    Arc::new(env)
}

/// Load `T` through the generated source-aware entry point.
fn load<T>(args: &[&str], vars: &[(&str, &str)]) -> ortho_config::OrthoResult<T>
where
    T: OrthoConfig,
{
    let source = environment(vars);
    T::load_from_iter_with_sources(args.iter().copied(), source.clone(), source)
}

/// Extract the report from a load that must fail on missing values.
fn report<T>(result: ortho_config::OrthoResult<T>) -> Result<MissingRequiredValues> {
    let error = result.err().ok_or_else(|| anyhow!("expected a failure"))?;
    match error.as_ref() {
        OrthoError::MissingRequiredValues(report) => Ok(report.as_ref().clone()),
        other => Err(anyhow!("expected missing values, got: {other}")),
    }
}

/// Collect rendered canonical paths in report order.
fn paths(report: &MissingRequiredValues) -> Vec<String> {
    report
        .items()
        .iter()
        .map(|item| item.path().render())
        .collect()
}

/// Collect every rendered route belonging to `path`.
fn routes_for(report: &MissingRequiredValues, path: &str) -> Result<Vec<String>> {
    let item = report
        .items()
        .iter()
        .find(|item| item.path().render() == path)
        .ok_or_else(|| anyhow!("the report does not contain `{path}`"))?;
    Ok(item
        .routes()
        .iter()
        .map(|route| match route {
            SupplyRoute::CommandLine { flag, .. } => format!("cli:{flag}"),
            SupplyRoute::Environment { variable } => format!("env:{variable}"),
            SupplyRoute::ConfigurationFile {
                path: file_path, ..
            } => {
                format!("file:{}", file_path.render())
            }
            _ => String::from("other"),
        })
        .collect())
}

#[rstest]
#[case::all_absent(&[], &[], &["reference", "region", "zone_alias", "tags"])]
#[case::environment_supplies_scalars(
    &[],
    &[("MRQ_REFERENCE", "r"), ("MRQ_REGION", "eu")],
    &["zone_alias", "tags"]
)]
#[case::alias_supplies_zone(
    &[],
    &[("MRQ_ALT_ZONE", "z")],
    &["reference", "region", "tags"]
)]
#[case::cli_supplies_reference(
    &["--reference", "r"],
    &[],
    &["region", "zone_alias", "tags"]
)]
#[case::present_empty_option_is_supplied(
    &[],
    &[("MRQ_OPTIONAL", "")],
    &["reference", "region", "zone_alias", "tags"]
)]
fn requiredness_matches_the_real_loader(
    #[case] args: &[&str],
    #[case] vars: &[(&str, &str)],
    #[case] expected: &[&str],
) -> Result<()> {
    let mut full_args = vec![PROGRAM];
    full_args.extend_from_slice(args);
    let report = report(load::<Reported>(&full_args, vars))?;
    ensure!(
        paths(&report) == expected,
        "expected {:?}, got {:?}",
        expected,
        paths(&report)
    );
    ensure!(
        report.program() == Some(PROGRAM),
        "the report must name the invoked program"
    );
    Ok(())
}

#[test]
fn a_cli_route_names_the_flag_the_parser_accepts() -> Result<()> {
    let report = report(load::<Reported>(&[PROGRAM], &[]))?;
    let routes = routes_for(&report, "reference")?;
    ensure!(
        routes.iter().any(|route| route == "cli:--reference"),
        "expected a CLI route, got {routes:?}"
    );
    ensure!(
        routes.iter().any(|route| route == "env:MRQ_REFERENCE"),
        "expected an environment route, got {routes:?}"
    );
    Ok(())
}

/// A requirement the command line cannot supply.
#[derive(Clone, Copy)]
struct NoCliRoute {
    /// Canonical path the report must name.
    path: &'static str,
    /// Environment route the report must advertise instead.
    environment_route: &'static str,
}

/// Assert `T` reports exactly `case.path`, with no command-line flag and the
/// declared environment route as its alternative.
fn assert_reported_without_cli_route<T: OrthoConfig>(case: NoCliRoute) -> Result<()> {
    let report = report(load::<T>(&[PROGRAM], &[]))?;
    ensure!(
        paths(&report) == [case.path],
        "expected only `{}`, got {:?}",
        case.path,
        paths(&report)
    );
    let routes = routes_for(&report, case.path)?;
    ensure!(
        !routes.iter().any(|route| route.starts_with("cli:")),
        "no generated flag can supply `{}`, got {routes:?}",
        case.path
    );
    ensure!(
        routes.iter().any(|route| route == case.environment_route),
        "expected the environment route {}, got {routes:?}",
        case.environment_route
    );
    Ok(())
}

#[test]
fn an_absent_nested_leaf_is_reported_without_a_cli_route() -> Result<()> {
    assert_reported_without_cli_route::<NestedRoot>(NoCliRoute {
        path: "database.host",
        environment_route: "env:MRN_DATABASE__HOST",
    })
}

#[test]
fn a_nested_value_from_the_environment_is_accepted() -> Result<()> {
    let config = load::<NestedRoot>(&[PROGRAM], &[("MRN_DATABASE__HOST", "db")])
        .map_err(|error| anyhow!("{error}"))?;
    ensure!(config.database.host == "db");
    Ok(())
}

#[test]
fn an_absent_optional_parent_deactivates_its_children() -> Result<()> {
    let result = load::<OptionalNestedRoot>(&[PROGRAM], &[]);
    ensure!(
        result.is_ok(),
        "an optional nested object must not require its leaves"
    );
    Ok(())
}

#[test]
fn a_present_non_object_is_left_to_the_deserializer() -> Result<()> {
    let result = load::<NestedRoot>(&[PROGRAM], &[("MRN_DATABASE", "scalar")]);
    let error = result.expect_err("a scalar cannot deserialize as an object");
    ensure!(
        !matches!(error.as_ref(), OrthoError::MissingRequiredValues(_)),
        "a type error must not be reported as a missing value: {error}"
    );
    Ok(())
}

#[test]
fn a_renamed_field_gets_no_cli_route() -> Result<()> {
    // The parser keys by identifier, so no flag can supply the renamed key.
    assert_reported_without_cli_route::<RenamedRoot>(NoCliRoute {
        path: "ref",
        environment_route: "env:MRR_REF",
    })
}

#[test]
fn a_skipped_cli_field_gets_no_cli_route() -> Result<()> {
    let report = report(load::<SkippedCliRoot>(&[PROGRAM], &[]))?;
    ensure!(paths(&report) == ["secret"]);
    let routes = routes_for(&report, "secret")?;
    ensure!(
        !routes.iter().any(|route| route.starts_with("cli:")),
        "skip_cli removes the flag, so no CLI route is truthful: {routes:?}"
    );
    Ok(())
}

#[test]
fn an_inferred_clap_default_is_not_required() -> Result<()> {
    let config =
        load::<InferredDefaultRoot>(&[PROGRAM], &[]).map_err(|error| anyhow!("{error}"))?;
    ensure!(config.count == 8, "the clap default must be replayed");
    Ok(())
}

#[test]
fn a_recursive_schema_is_a_validation_error_not_a_stack_overflow() -> Result<()> {
    let result = load::<RecursiveRoot>(&[PROGRAM], &[]);
    let error = result.expect_err("the schema cannot be composed");
    ensure!(
        error
            .to_string()
            .contains("recursive required-value schema"),
        "unexpected error: {error}"
    );
    Ok(())
}
