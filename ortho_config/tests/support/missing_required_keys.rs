//! Key-shape agreement between the schema and Serde, observed through real
//! generated loaders.
//!
//! Split from `missing_required_supply.rs` to keep both files within the
//! repository's 400-line ceiling. Each case supplies a value a user could
//! actually set and asserts that the schema names the key Serde reads:
//!
//! - a defaulted flattened struct still fails on a missing child leaf, because
//!   Serde applies a field default only to a field it never saw;
//! - Serde's `lowercase` field rule leaves an identifier untouched, so the
//!   schema must not apply a general-purpose case conversion;
//! - a key that no bare TOML key can express is recovered from a configuration
//!   file through the quoted spelling the report advertises;
//! - a defaulted flatten composes every child leaf, so supplying one still
//!   exposes the other;
//! - a nested parent ending in `_` merges with the provider's `__` separator,
//!   so it must not advertise an environment route.
//!
//! Fields read through a custom deserializer are covered separately in
//! `missing_required_deserializers.rs`.

use anyhow::{Context, Result, anyhow, ensure};
use cap_std::{ambient_authority, fs::Dir};
use ortho_config::OrthoConfig;
use serde::{Deserialize, Serialize};

use super::{PROGRAM, load, paths, report, routes_for};

/// A child whose required leaf composes at the root.
#[derive(Debug, Deserialize, Serialize, ortho_config::RequiredConfigSchema)]
struct ComposedChild {
    /// Required leaf with no default.
    host: String,
}

/// A root whose flattened child declares a default it never receives.
#[derive(Debug, Deserialize, Serialize, OrthoConfig)]
#[ortho_config(prefix = "MRK_")]
struct DefaultedFlattenRoot {
    /// Serde still deserializes this child from the unmatched keys.
    #[serde(flatten, default)]
    #[ortho_config(nested, skip_cli)]
    extra: ComposedChild,
    /// A sibling requirement, so a partial report is observable.
    region: String,
}

#[expect(
    non_snake_case,
    reason = "the fixture needs an identifier whose case distinguishes Serde's rule"
)]
mod lowercase_rule {
    //! A root whose identifier Serde reads unchanged under `lowercase`.
    //!
    //! The mixed-case identifier is the point of the fixture: Serde's
    //! `lowercase` field rule leaves it untouched, while a general-purpose case
    //! converter would emit `api_url` and name a key Serde never reads. The
    //! expectation covers the whole module because the derive re-emits the
    //! identifier into its generated defaults struct, which sits outside any
    //! attribute on the struct.

    use ortho_config::OrthoConfig;
    use serde::{Deserialize, Serialize};

    /// The root whose identifier keeps its case.
    #[derive(Debug, Deserialize, Serialize, OrthoConfig)]
    #[ortho_config(prefix = "MRR_")]
    #[serde(rename_all = "lowercase")]
    pub(super) struct LowercaseRenameRoot {
        /// The identifier Serde reads unchanged.
        pub(super) api_URL: String,
    }
}

/// A root whose key needs a quoted TOML key in a configuration file.
#[derive(Debug, Deserialize, Serialize, OrthoConfig)]
#[ortho_config(prefix = "MRP_")]
struct QuotedKeyRoot {
    /// A dot inside the key itself, which a bare TOML key cannot express.
    #[serde(rename = "Host.Name")]
    host_name: String,
}

/// A flattened child with two required leaves.
#[derive(Debug, Deserialize, Serialize, ortho_config::RequiredConfigSchema)]
struct TwoLeafChild {
    /// First required leaf.
    host: String,
    /// Second required leaf.
    port: u16,
}

/// A root whose defaulted flatten composes both child leaves.
#[derive(Debug, Deserialize, Serialize, OrthoConfig)]
#[ortho_config(prefix = "MRL_")]
struct TwoLeafFlattenRoot {
    /// Serde still deserializes this child from the unmatched keys.
    #[serde(flatten, default)]
    #[ortho_config(nested, skip_cli)]
    extra: TwoLeafChild,
    /// A sibling that keeps a partial report observable.
    region: String,
}

/// A required leaf reached through a renamed parent.
#[derive(Debug, Deserialize, Serialize, ortho_config::RequiredConfigSchema)]
struct ParentedLeaf {
    /// The leaf every case must still name.
    host: String,
}

/// A root whose nested parent ends in the provider's separator character.
#[derive(Debug, Deserialize, Serialize, OrthoConfig)]
#[ortho_config(prefix = "MRU_")]
struct TrailingUnderscoreParentRoot {
    /// Joined to its child with `__`, `database_` reads back as `database._host`.
    #[serde(rename = "database_")]
    #[ortho_config(nested, skip_cli)]
    database: ParentedLeaf,
}

/// A root with the same shape and a parent that leaves the separator intact.
#[derive(Debug, Deserialize, Serialize, OrthoConfig)]
#[ortho_config(prefix = "MRT_")]
struct NestedParentRoot {
    /// An ordinary nested parent, kept as the positive control.
    #[ortho_config(nested, skip_cli)]
    database: ParentedLeaf,
}

/// Write `contents` into a discoverable configuration file.
///
/// The temporary directory is returned with the path so the file outlives the
/// load that reads it.
fn config_file(contents: &str) -> Result<(tempfile::TempDir, String)> {
    let dir = tempfile::tempdir().context("create temp dir")?;
    let cap = Dir::open_ambient_dir(dir.path(), ambient_authority()).context("open temp dir")?;
    cap.write("config.toml", contents.as_bytes())
        .context("write config file")?;
    let file = dir.path().join("config.toml");
    let path = file
        .to_str()
        .ok_or_else(|| anyhow!("temporary path is not valid UTF-8: {}", file.display()))?;
    Ok((dir, path.to_owned()))
}

/// A flattened struct's default never rescues a missing child leaf.
///
/// Serde applies `#[serde(default)]` to a field it never saw; a flattened
/// field is always deserialized from the unmatched keys, so the child's
/// failure propagates. Reporting the child keeps the one-diagnostic contract:
/// supplying `region` alone must not reveal a second missing value later.
#[test]
fn a_defaulted_flatten_keeps_its_child_requirements() -> Result<()> {
    let missing = report(load::<DefaultedFlattenRoot>(&[PROGRAM], &[]))?;
    ensure!(
        paths(&missing) == ["host", "region"],
        "the child leaf composes before its sibling, got {:?}",
        paths(&missing)
    );
    Ok(())
}

/// Supplying the sibling must not hide the flattened leaf.
#[test]
fn supplying_the_sibling_still_exposes_the_flattened_leaf() -> Result<()> {
    let missing = report(load::<DefaultedFlattenRoot>(
        &[PROGRAM],
        &[("MRK_REGION", "eu")],
    ))?;
    ensure!(
        paths(&missing) == ["host"],
        "the child leaf must survive a supplied sibling, got {:?}",
        paths(&missing)
    );
    let config =
        load::<DefaultedFlattenRoot>(&[PROGRAM], &[("MRK_REGION", "eu"), ("MRK_HOST", "db")])
            .map_err(|error| anyhow!("the composed environment route must recover: {error}"))?;
    ensure!(
        config.extra.host == "db" && config.region == "eu",
        "the advertised route must supply both values, got host={} region={}",
        config.extra.host,
        config.region
    );
    Ok(())
}

/// A `lowercase` field rule keeps the identifier Serde reads.
#[test]
fn a_lowercase_rename_all_keeps_the_identifier_serde_reads() -> Result<()> {
    let missing = report(load::<lowercase_rule::LowercaseRenameRoot>(&[PROGRAM], &[]))?;
    ensure!(
        paths(&missing) == ["api_URL"],
        "Serde leaves a struct field untouched under `lowercase`, got {:?}",
        paths(&missing)
    );
    let routes = routes_for(&missing, "api_URL")?;
    ensure!(
        !routes.iter().any(|route| route.starts_with("env:")),
        "the provider lowercases the key, so no environment route is truthful: {routes:?}"
    );
    ensure!(
        routes.iter().any(|route| route.starts_with("file:")),
        "the key is still settable in a file, got {routes:?}"
    );

    let (_dir, path) = config_file("api_URL = \"set\"\n")?;
    let config =
        load::<lowercase_rule::LowercaseRenameRoot>(&[PROGRAM], &[("MRR_CONFIG_PATH", &path)])
            .map_err(|error| anyhow!("the advertised file route must recover: {error}"))?;
    ensure!(
        config.api_URL == "set",
        "the file must supply the leaf, got {}",
        config.api_URL
    );
    Ok(())
}

/// A key that no bare TOML key can express is recoverable from a file.
#[test]
fn a_quoted_file_key_recovers_the_missing_value() -> Result<()> {
    let missing = report(load::<QuotedKeyRoot>(&[PROGRAM], &[]))?;
    ensure!(
        paths(&missing) == [r#""Host.Name""#],
        "the report must render the key as a quoted TOML key, got {:?}",
        paths(&missing)
    );
    let routes = routes_for(&missing, r#""Host.Name""#)?;
    ensure!(
        routes.iter().any(|route| route.starts_with("file:")),
        "the key is settable in a file, got {routes:?}"
    );

    let (_dir, path) = config_file("\"Host.Name\" = \"set\"\n")?;
    let config = load::<QuotedKeyRoot>(&[PROGRAM], &[("MRP_CONFIG_PATH", &path)])
        .map_err(|error| anyhow!("the quoted key must recover: {error}"))?;
    ensure!(
        config.host_name == "set",
        "the quoted key must supply the leaf, got {}",
        config.host_name
    );
    Ok(())
}

/// A defaulted flatten keeps both child leaves and exposes each one alone.
#[test]
fn supplying_one_flattened_leaf_exposes_the_other() -> Result<()> {
    let missing = report(load::<TwoLeafFlattenRoot>(&[PROGRAM], &[]))?;
    ensure!(
        paths(&missing) == ["host", "port", "region"],
        "both child leaves compose before the sibling, got {:?}",
        paths(&missing)
    );

    let missing_after_host = report(load::<TwoLeafFlattenRoot>(
        &[PROGRAM],
        &[("MRL_HOST", "db")],
    ))?;
    ensure!(
        paths(&missing_after_host) == ["port", "region"],
        "supplying one child leaf must not hide the other, got {:?}",
        paths(&missing_after_host)
    );

    let config = load::<TwoLeafFlattenRoot>(
        &[PROGRAM],
        &[
            ("MRL_HOST", "db"),
            ("MRL_PORT", "5432"),
            ("MRL_REGION", "eu"),
        ],
    )
    .map_err(|error| anyhow!("the composed routes must recover: {error}"))?;
    ensure!(
        config.extra.host == "db" && config.extra.port == 5432 && config.region == "eu",
        "every route must supply its value, got host={} port={} region={}",
        config.extra.host,
        config.extra.port,
        config.region
    );
    Ok(())
}

/// A parent ending in `_` must not advertise a variable it cannot own.
///
/// The provider splits the joined name rather than the segments, so
/// `database_` plus `__` becomes `database___HOST`, which it reads as
/// `database._host`. The advertised variable would repair nothing. The
/// separator-free root beside it is the positive control: same shape, same
/// leaf, and its advertised variable really does recover the value.
#[test]
fn a_trailing_underscore_parent_has_no_environment_route() -> Result<()> {
    let missing = report(load::<TrailingUnderscoreParentRoot>(&[PROGRAM], &[]))?;
    ensure!(
        paths(&missing) == ["database_.host"],
        "the leaf keeps its canonical path, got {:?}",
        paths(&missing)
    );
    let routes = routes_for(&missing, "database_.host")?;
    ensure!(
        !routes.iter().any(|route| route.starts_with("env:")),
        "`database_` merges with the separator, so no variable can name it: {routes:?}"
    );
    ensure!(
        routes.iter().any(|route| route == "file:database_.host"),
        "the file route remains truthful, got {routes:?}"
    );

    let control = report(load::<NestedParentRoot>(&[PROGRAM], &[]))?;
    let control_routes = routes_for(&control, "database.host")?;
    ensure!(
        control_routes
            .iter()
            .any(|route| route == "env:MRT_DATABASE__HOST"),
        "an expressible nested path must advertise its variable, got {control_routes:?}"
    );

    let (_dir, path) = config_file("[database_]\nhost = \"db\"\n")?;
    let from_file = load::<TrailingUnderscoreParentRoot>(&[PROGRAM], &[("MRU_CONFIG_PATH", &path)])
        .map_err(|error| anyhow!("the advertised file route must recover: {error}"))?;
    ensure!(
        from_file.database.host == "db",
        "the table must supply the leaf, got {}",
        from_file.database.host
    );

    let from_env = load::<NestedParentRoot>(&[PROGRAM], &[("MRT_DATABASE__HOST", "db")])
        .map_err(|error| anyhow!("the advertised variable must recover: {error}"))?;
    ensure!(
        from_env.database.host == "db",
        "the advertised variable must supply the leaf, got {}",
        from_env.database.host
    );
    Ok(())
}
