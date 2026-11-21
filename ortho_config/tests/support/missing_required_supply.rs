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
//!   spelling, so supplying the advertised variable or file key succeeds;
//! - an observed spelling that no variable name can carry keeps only the file
//!   route, and that route is quoted so the file can express it;
//! - a partially supplied optional flatten still reports nothing, because one
//!   failing descendant makes Serde resolve the whole child to absence;
//! - a default that the loader never reads does not hide a requirement, while
//!   one whose identifier is an accepted alias does supply the value.

use anyhow::{Context, Result, anyhow, ensure};
use cap_std::{ambient_authority, fs::Dir};
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

/// A nested object held by a flattened child.
#[derive(Debug, Deserialize, Serialize, ortho_config::RequiredConfigSchema)]
struct FlattenedDatabase {
    /// Required leaf whose absence fails the whole flattened child.
    host: String,
    /// Optional leaf used to supply the object only partially.
    #[serde(default)]
    port: u16,
}

/// A flattened child whose requirement sits one level deeper.
#[derive(Debug, Deserialize, Serialize, ortho_config::RequiredConfigSchema)]
struct NestedFlattenedChild {
    /// The object that holds the required leaf.
    #[ortho_config(nested)]
    database: FlattenedDatabase,
}

/// A root whose optional flatten nests its requirement one level down.
#[derive(Debug, Deserialize, Serialize, OrthoConfig)]
#[ortho_config(prefix = "MRH_")]
struct OptionalNestedFlattenRoot {
    /// A child that cannot deserialize is absence, not a missing leaf.
    #[serde(flatten)]
    #[ortho_config(nested, skip_cli)]
    extra: Option<NestedFlattenedChild>,
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

/// A root whose alias spelling no environment variable can carry.
#[derive(Debug, Deserialize, Serialize, OrthoConfig)]
#[ortho_config(prefix = "MRV_")]
struct EqualsAliasRoot {
    /// `=` separates a variable's name from its value, so only a file can
    /// spell the alias the merged value used.
    #[serde(alias = "Host=Name")]
    #[ortho_config(nested, skip_cli)]
    database: AliasDatabase,
}

/// A root whose declared default is renamed away from the loaded key.
#[derive(Debug, Deserialize, Serialize, OrthoConfig)]
#[ortho_config(prefix = "MRD_")]
struct RenamedDefaultRoot {
    /// The default is serialized under `reference`, but the loader reads `ref`.
    #[serde(rename = "ref")]
    #[ortho_config(default = String::from("defaulted"))]
    reference: String,
    /// A second requirement, so the report must still enumerate siblings.
    region: String,
}

/// A root whose identifier survives the rename as an accepted alias.
#[derive(Debug, Deserialize, Serialize, OrthoConfig)]
#[ortho_config(prefix = "MRDA_")]
struct RenamedAliasedDefaultRoot {
    /// The default's spelling is an alias, so it really supplies the value.
    #[serde(rename = "ref", alias = "reference")]
    #[ortho_config(default = String::from("defaulted"))]
    reference: String,
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

/// A file route must be replayed, not merely spelled correctly.
///
/// The alias report advertises `db.host`. Writing exactly that key into a
/// discovered configuration file must recover the value, which proves the
/// route names a key this loader really reads.
#[test]
fn the_alias_file_route_recovers_the_missing_value() -> Result<()> {
    let (_dir, path) = config_file(b"[db]\nhost = \"db\"\n")?;
    let config = load::<AliasNestedRoot>(
        &[PROGRAM],
        &[("MRA_DB__PORT", "1"), ("MRA_CONFIG_PATH", path.as_str())],
    )
    .map_err(|error| anyhow::anyhow!("the advertised file route must recover: {error}"))?;
    ensure!(
        config.database.host == "db" && config.database.port == 1,
        "the file route must supply the required leaf beside the environment value, got {:?}",
        config.database
    );
    Ok(())
}

/// Write `contents` to a fresh discovery file and return the temporary
/// directory beside its path, so the file outlives the load that reads it.
fn config_file(contents: &'static [u8]) -> Result<(tempfile::TempDir, String)> {
    let dir = tempfile::tempdir().context("create temp dir")?;
    let cap = Dir::open_ambient_dir(dir.path(), ambient_authority()).context("open temp dir")?;
    cap.write("config.toml", contents)
        .context("write config file")?;
    let file = dir.path().join("config.toml");
    let path = file
        .to_str()
        .ok_or_else(|| anyhow!("temporary path is not valid UTF-8: {}", file.display()))?;
    Ok((dir, path.to_owned()))
}

/// A key containing `=` can be supplied only by a quoted TOML table.
///
/// The alias spelling is observed from a configuration file, so the report
/// repeats it for the file route. A variable name cannot contain `=`, so no
/// environment route may be advertised beside it.
#[test]
fn an_alias_spelling_no_variable_can_carry_gets_no_environment_route() -> Result<()> {
    let (_dir, path) = config_file(b"[\"Host=Name\"]\nport = 1\n")?;
    let missing = report(load::<EqualsAliasRoot>(
        &[PROGRAM],
        &[("MRV_CONFIG_PATH", &path)],
    ))?;
    ensure!(
        paths(&missing) == ["database.host"],
        "the canonical identity stays stable, got {:?}",
        paths(&missing)
    );
    let routes = routes_for(&missing, "database.host")?;
    ensure!(
        !routes.iter().any(|route| route.starts_with("env:")),
        "`=` cannot appear in a variable name, got {routes:?}"
    );
    ensure!(
        routes
            .iter()
            .any(|route| route == r#"file:"Host=Name".host"#),
        "the file route must quote the observed spelling, got {routes:?}"
    );

    let (_recovery_dir, recovery_path) =
        config_file(b"[\"Host=Name\"]\nhost = \"db\"\nport = 1\n")?;
    let config = load::<EqualsAliasRoot>(&[PROGRAM], &[("MRV_CONFIG_PATH", &recovery_path)])
        .map_err(|error| anyhow!("the advertised file route must recover: {error}"))?;
    ensure!(
        config.database.host == "db" && config.database.port == 1,
        "the quoted table must supply both leaves, got {:?}",
        config.database
    );
    Ok(())
}

/// A partially supplied optional flatten must not invent a report.
///
/// Serde converts any descendant failure into `None` for a flattened option.
/// Supplying only the defaulted `port` leaves `host` absent, so the loader
/// resolves the child to absence and never raises a missing-value error.
#[test]
fn a_partially_supplied_optional_flatten_reports_nothing() -> Result<()> {
    let config = load::<OptionalNestedFlattenRoot>(&[PROGRAM], &[("MRH_DATABASE__PORT", "1")])
        .map_err(|error| {
            anyhow::anyhow!("a failed optional flatten must resolve to absence: {error}")
        })?;
    ensure!(
        config.extra.is_none(),
        "the partially supplied child must resolve to `None`"
    );
    Ok(())
}

/// A default the loader cannot read must not hide the requirement.
///
/// The generated defaults struct keys every value by its Rust identifier, so a
/// renamed field without a matching alias never receives it. Reporting the
/// renamed key beside an unrelated requirement keeps the report honest.
#[test]
fn a_renamed_default_is_reported_beside_another_missing_field() -> Result<()> {
    let missing = report(load::<RenamedDefaultRoot>(&[PROGRAM], &[]))?;
    ensure!(
        paths(&missing) == ["ref", "region"],
        "the unreachable default must not hide the requirement, got {:?}",
        paths(&missing)
    );
    Ok(())
}

/// An identifier that remains an alias must still supply the default.
#[test]
fn a_default_reaches_a_renamed_key_through_an_alias() -> Result<()> {
    let config = load::<RenamedAliasedDefaultRoot>(&[PROGRAM], &[])
        .map_err(|error| anyhow::anyhow!("the default is accepted through the alias: {error}"))?;
    ensure!(
        config.reference == "defaulted",
        "the default must reach the renamed field, got {:?}",
        config.reference
    );
    Ok(())
}
