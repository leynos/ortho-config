//! Requiredness of fields Serde reads through a custom deserializer.
//!
//! Serde's `expr_is_missing` consults a field default, then a container
//! default, and only then the field's deserializer: when one is installed it
//! returns `missing_field` instead of the helper that turns an absent value
//! into `None`. Two consequences are observable through a real loader:
//!
//! - an `Option` field with a custom deserializer is required unless a default
//!   applies, and `#[serde(with = "...")]` installs one just as
//!   `deserialize_with` does;
//! - a `nested` field with a custom deserializer is an opaque leaf, because the
//!   child's key model is no longer what the loader reads.

use anyhow::{Result, anyhow, ensure};
use ortho_config::OrthoConfig;
use serde::{Deserialize, Deserializer, Serialize};

use super::{PROGRAM, load, paths, report, routes_for};

/// Read the field through Serde's own `Option` deserialization.
///
/// Naming a custom deserializer is what matters: Serde stops using its
/// `missing_field` helper, so an absent key becomes an error even though the
/// declared type can represent absence.
fn parse_optional_host<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: Deserializer<'de>,
{
    Option::<String>::deserialize(deserializer)
}

mod host_module {
    //! The `#[serde(with = "...")]` spelling of the same replacement.
    //!
    //! Serde reads a module's `deserialize` exactly as it reads a bare
    //! `deserialize_with` function, so both spellings install a custom
    //! deserializer. The module form also has to supply `serialize`.

    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    /// Deserialize through the field's own type.
    pub(super) fn deserialize<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
    where
        D: Deserializer<'de>,
    {
        Option::<String>::deserialize(deserializer)
    }

    /// Serialize through the field's own type.
    ///
    /// The `&Option<String>` shape is Serde's own contract for a `with`
    /// module: the generated code passes `&self.field`, so the borrow-friendly
    /// form is not an option here.
    #[expect(
        clippy::ref_option,
        reason = "Serde's `with` module contract passes the field by reference"
    )]
    pub(super) fn serialize<S>(value: &Option<String>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        value.serialize(serializer)
    }
}

/// A root whose `Option` leaf is read by a custom deserializer.
#[derive(Debug, Deserialize, Serialize, OrthoConfig)]
#[ortho_config(prefix = "MRC_")]
struct CustomDeserializerOption {
    /// Required despite the `Option` type, because no default applies.
    #[serde(deserialize_with = "parse_optional_host")]
    host: Option<String>,
}

/// A root using the module form of the same custom deserializer.
#[derive(Debug, Deserialize, Serialize, OrthoConfig)]
#[ortho_config(prefix = "MRW_")]
struct WithDeserializerOption {
    /// Required for the same reason as [`CustomDeserializerOption::host`].
    #[serde(with = "host_module")]
    host: Option<String>,
}

/// A root whose custom deserializer is paired with a Serde default.
#[derive(Debug, Deserialize, Serialize, OrthoConfig)]
#[ortho_config(prefix = "MCD_")]
struct DefaultedDeserializerOption {
    /// Optional, because Serde consults the default before the deserializer.
    #[serde(default, deserialize_with = "parse_optional_host")]
    host: Option<String>,
}

/// A child whose key model the schema must not assume.
#[derive(Debug, Deserialize, Serialize, ortho_config::RequiredConfigSchema)]
struct OpaqueChild {
    /// A leaf the custom deserializer fills from its own input.
    host: String,
    /// A second leaf, so composing the child would change the report.
    #[serde(default)]
    port: u16,
}

/// Read the whole child from the single string Serde is given.
fn parse_opaque_child<'de, D>(deserializer: D) -> Result<OpaqueChild, D::Error>
where
    D: Deserializer<'de>,
{
    let host = String::deserialize(deserializer)?;
    Ok(OpaqueChild { host, port: 0 })
}

/// A root whose nested field is read by a custom deserializer.
#[derive(Debug, Deserialize, Serialize, OrthoConfig)]
#[ortho_config(prefix = "MCX_")]
struct CustomDeserializedNested {
    /// The deserializer decides which keys it reads, so no child is composed.
    #[serde(deserialize_with = "parse_opaque_child")]
    #[ortho_config(nested, skip_cli)]
    database: OpaqueChild,
}

/// Assert the only reported path is `host`.
fn assert_only_host_is_reported<T: OrthoConfig>() -> Result<()> {
    let missing = report(load::<T>(&[PROGRAM], &[]))?;
    ensure!(
        paths(&missing) == ["host"],
        "a custom deserializer makes the `Option` field required, got {:?}",
        paths(&missing)
    );
    Ok(())
}

/// A custom deserializer makes an `Option` leaf required.
#[test]
fn a_custom_deserializer_makes_an_option_required() -> Result<()> {
    assert_only_host_is_reported::<CustomDeserializerOption>()?;
    let config = load::<CustomDeserializerOption>(&[PROGRAM], &[("MRC_HOST", "db")])
        .map_err(|error| anyhow!("the advertised route must recover: {error}"))?;
    ensure!(
        config.host.as_deref() == Some("db"),
        "the route must supply the leaf, got {:?}",
        config.host
    );
    Ok(())
}

/// The `#[serde(with = "...")]` spelling is the same replacement.
#[test]
fn the_serde_with_spelling_is_also_a_custom_deserializer() -> Result<()> {
    assert_only_host_is_reported::<WithDeserializerOption>()?;
    let config = load::<WithDeserializerOption>(&[PROGRAM], &[("MRW_HOST", "db")])
        .map_err(|error| anyhow!("the advertised route must recover: {error}"))?;
    ensure!(
        config.host.as_deref() == Some("db"),
        "the route must supply the leaf, got {:?}",
        config.host
    );
    Ok(())
}

/// A Serde default is consulted before the deserializer, so it still applies.
#[test]
fn a_serde_default_keeps_a_custom_deserialized_option_optional() -> Result<()> {
    let config = load::<DefaultedDeserializerOption>(&[PROGRAM], &[])
        .map_err(|error| anyhow!("the default must supply the absent leaf: {error}"))?;
    ensure!(
        config.host.is_none(),
        "the field default must produce `None`, got {:?}",
        config.host
    );
    Ok(())
}

/// A custom-deserialized nested field is an opaque leaf, not an object.
#[test]
fn a_custom_deserialized_nested_field_is_an_opaque_leaf() -> Result<()> {
    let missing = report(load::<CustomDeserializedNested>(&[PROGRAM], &[]))?;
    ensure!(
        paths(&missing) == ["database"],
        "no child requirement may be composed, got {:?}",
        paths(&missing)
    );
    let routes = routes_for(&missing, "database")?;
    ensure!(
        routes.iter().any(|route| route == "env:MCX_DATABASE"),
        "the field's own key is what the deserializer receives: {routes:?}"
    );

    let config = load::<CustomDeserializedNested>(&[PROGRAM], &[("MCX_DATABASE", "db")])
        .map_err(|error| anyhow!("the advertised route must recover: {error}"))?;
    ensure!(
        config.database.host == "db",
        "the deserializer must read the advertised value, got {}",
        config.database.host
    );
    Ok(())
}
