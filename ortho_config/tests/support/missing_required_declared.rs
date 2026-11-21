//! Declared shapes that decide requiredness, observed through real loaders.
//!
//! Five rows of V1's requiredness matrix turn on the declaration rather than on
//! a supplied key, and V1 requires each to be exercised through a loader rather
//! than a hand-built schema:
//!
//! - a container default supplies every field of its type, so none of them is
//!   required even though no key is present;
//! - an explicit `null` is a present value, and Serde's type error is the
//!   accurate diagnostic rather than a missing-value report;
//! - an empty collection is present, and a collection's merge strategy does not
//!   change whether absence is an error;
//! - a `#[serde(skip)]` field is never inspected, because Serde fills it from
//!   `Default` and reads no key for it;
//! - a documentation override describes a value without deciding whether it is
//!   required;
//! - a default declared by a child's own loader supplies that loader only, so a
//!   parent composing the same type must still report the key.
//!
//! Every shape is paired with a control that names the same path when the value
//! really is absent, so an empty report is a property of the shape and not of
//! the fixture.

use anyhow::{Context, Result, anyhow, ensure};
use cap_std::{ambient_authority, fs::Dir};
use ortho_config::OrthoConfig;
use serde::{Deserialize, Serialize};

use super::{PROGRAM, load, paths, report};

/// A root whose container default supplies every declared field.
#[derive(Debug, Deserialize, Serialize, OrthoConfig)]
#[ortho_config(prefix = "MRY_")]
#[serde(default)]
struct ContainerDefaultRoot {
    /// Supplied by the container default when the key is absent.
    host: String,
    /// Supplied by the same default.
    region: String,
}

impl Default for ContainerDefaultRoot {
    fn default() -> Self {
        Self {
            host: String::from("default-host"),
            region: String::from("default-region"),
        }
    }
}

/// A root whose required collection merges by appending.
#[derive(Debug, Deserialize, Serialize, OrthoConfig)]
#[ortho_config(prefix = "MRZ_")]
struct CollectionRoot {
    /// Required: neither a default nor an overlay supplies the collection.
    #[ortho_config(merge_strategy = "append")]
    tags: Vec<String>,
}

/// A root whose required scalar may also arrive as an explicit null.
#[derive(Debug, Deserialize, Serialize, OrthoConfig)]
#[ortho_config(prefix = "MRJ_")]
struct NullableRoot {
    /// Required when absent, yet a present null is a type error, not absence.
    host: String,
    /// Optional sibling proving the schema still composes.
    #[serde(default)]
    port: u16,
}

/// A root whose skipped field Serde never reads.
#[derive(Debug, Deserialize, Serialize, OrthoConfig)]
#[ortho_config(prefix = "MRSK_")]
struct SkippedFieldRoot {
    /// Filled from `Default`; no configuration key is ever read for it.
    #[serde(skip)]
    internal: String,
    /// Required control reported beside it.
    host: String,
}

/// A root whose documentation override contradicts the runtime requirement.
#[derive(Debug, Deserialize, Serialize, OrthoConfig)]
#[ortho_config(prefix = "MRKA_")]
struct DocumentedRequiredRoot {
    /// The override describes the value; it does not make the value optional.
    #[ortho_config(doc(required = false))]
    host: String,
}

/// A child type that derives a loader of its own and declares a default.
#[derive(Debug, Deserialize, Serialize, OrthoConfig)]
#[ortho_config(prefix = "MRDL_")]
struct LoaderChild {
    /// Required for this type's own loader and for any parent that composes it.
    name: String,
    /// Supplied by this type's loader, which a parent never runs.
    #[ortho_config(default = 8080)]
    port: u16,
}

/// A parent that composes the loader-bearing child.
#[derive(Debug, Deserialize, Serialize, OrthoConfig)]
#[ortho_config(prefix = "MRDP_")]
struct LoaderParent {
    /// No flag can carry the object; the schema is still composed.
    #[ortho_config(nested, skip_cli)]
    child: LoaderChild,
}

/// Write `contents` to a fresh discovery file named `name`.
///
/// The directory is returned beside the path so the file outlives the load.
fn config_file(name: &str, contents: &'static [u8]) -> Result<(tempfile::TempDir, String)> {
    let dir = tempfile::tempdir().context("create temp dir")?;
    let cap = Dir::open_ambient_dir(dir.path(), ambient_authority()).context("open temp dir")?;
    cap.write(name, contents).context("write config file")?;
    let file = dir.path().join(name);
    let path = file
        .to_str()
        .ok_or_else(|| anyhow!("temporary path is not valid UTF-8: {}", file.display()))?;
    Ok((dir, path.to_owned()))
}

/// A container default is not an absence for any of its fields.
#[test]
fn a_container_default_supplies_every_field() -> Result<()> {
    let config = load::<ContainerDefaultRoot>(&[PROGRAM], &[])
        .map_err(|error| anyhow!("a container default must supply every field: {error}"))?;
    ensure!(
        config.host == "default-host" && config.region == "default-region",
        "the container default must reach both fields, got {config:?}"
    );
    Ok(())
}

/// An empty collection is present; an absent one is still reported.
#[test]
fn an_empty_collection_is_supplied() -> Result<()> {
    let missing = report(load::<CollectionRoot>(&[PROGRAM], &[]))?;
    ensure!(
        paths(&missing) == ["tags"],
        "the collection control must report the absent field, got {:?}",
        paths(&missing)
    );

    let (_dir, path) = config_file("config.toml", b"tags = []\n")?;
    let config = load::<CollectionRoot>(&[PROGRAM], &[("MRZ_CONFIG_PATH", &path)])
        .map_err(|error| anyhow!("an empty collection must be accepted: {error}"))?;
    ensure!(
        config.tags.is_empty(),
        "the empty collection must stay empty, got {:?}",
        config.tags
    );
    Ok(())
}

/// A skipped field is not a requirement, so only its sibling is reported.
#[test]
fn a_serde_skipped_field_is_not_inspected() -> Result<()> {
    let missing = report(load::<SkippedFieldRoot>(&[PROGRAM], &[]))?;
    ensure!(
        paths(&missing) == ["host"],
        "a skipped field reads no key, so it cannot be missing: {:?}",
        paths(&missing)
    );

    let config = load::<SkippedFieldRoot>(&[PROGRAM], &[("MRSK_HOST", "db")])
        .map_err(|error| anyhow!("the required sibling must recover: {error}"))?;
    ensure!(
        config.internal.is_empty() && config.host == "db",
        "the skipped field must keep its default, got {config:?}"
    );
    Ok(())
}

/// A documentation override never changes a runtime decision.
#[test]
fn a_documentation_override_does_not_relax_requiredness() -> Result<()> {
    let missing = report(load::<DocumentedRequiredRoot>(&[PROGRAM], &[]))?;
    ensure!(
        paths(&missing) == ["host"],
        "`doc(required = false)` must not silence a real requirement: {:?}",
        paths(&missing)
    );
    Ok(())
}

/// A declared default supplies only the loader of the type that declares it.
///
/// `LoaderChild::load` builds the defaults struct and materialises `port`, so
/// its own report names `name` alone. `LoaderParent` composes the same type but
/// never runs the child's loader, so nothing supplies `child.port` and the
/// report must keep naming it: a report that omitted the key would send the
/// user to fix `child.name` and meet a second failure afterwards. Both loads
/// use the same absent inputs, so the difference is the active loading path.
#[test]
fn a_declared_default_supplies_only_its_own_loader() -> Result<()> {
    let own = report(load::<LoaderChild>(&[PROGRAM], &[]))?;
    ensure!(
        paths(&own) == ["name"],
        "the declaring type's loader must materialise `port`, got {:?}",
        paths(&own)
    );

    let composed = report(load::<LoaderParent>(&[PROGRAM], &[]))?;
    ensure!(
        paths(&composed) == ["child.name", "child.port"],
        "a parent's loader never materialises the child's default, got {:?}",
        paths(&composed)
    );
    Ok(())
}

/// The routes reported for a composed child recover the load.
///
/// `a_declared_default_supplies_only_its_own_loader` insists the parent reports
/// `child.port`; this case insists that reporting it is honest by supplying the
/// value through the environment route the report advertises.
#[test]
fn a_parent_accepts_the_composed_child_it_reports() -> Result<()> {
    let config = load::<LoaderParent>(
        &[PROGRAM],
        &[("MRDP_CHILD__NAME", "db"), ("MRDP_CHILD__PORT", "1")],
    )
    .map_err(|error| anyhow!("the advertised environment routes must recover: {error}"))?;
    ensure!(
        config.child.name == "db" && config.child.port == 1,
        "the supplied child must load unchanged, got {config:?}"
    );
    Ok(())
}

/// An explicit null is present, so the deserializer reports the type error.
///
/// TOML cannot spell null, so the value arrives through a JSON5 file. Without
/// the `json5` feature no file can carry it and the case cannot run.
#[cfg(feature = "json5")]
#[test]
fn an_explicit_null_is_present_not_missing() -> Result<()> {
    let (_dir, path) = config_file("config.json5", b"{ host: null }\n")?;
    let result = load::<NullableRoot>(&[PROGRAM], &[("MRJ_CONFIG_PATH", &path)]);
    let error = result
        .err()
        .ok_or_else(|| anyhow!("a null must not deserialize as a string"))?;
    ensure!(
        !matches!(
            error.as_ref(),
            ortho_config::OrthoError::MissingRequiredValues(_)
        ),
        "a present null is a type error, not a missing value: {error}"
    );
    ensure!(
        error.to_string().contains("invalid type"),
        "the deserializer's type error must surface, got: {error}"
    );
    Ok(())
}
