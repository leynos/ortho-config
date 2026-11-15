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
//!   file through the quoted spelling the report advertises.

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
