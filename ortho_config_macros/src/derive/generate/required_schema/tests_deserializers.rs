//! Requiredness decisions for fields with a Serde custom deserializer.
//!
//! Serde's `expr_is_missing` consults the field default, then the container
//! default, and only then the field's deserializer. When one is installed it
//! returns `missing_field` instead of the helper that turns an absent value
//! into `None` for an `Option`, and it replaces the child's own
//! `Deserialize` implementation when the field is marked `nested`. These cases
//! pin the emitted tokens for both consequences.

use anyhow::{Result, ensure};
use rstest::rstest;
use syn::DeriveInput;

use super::emit_plain;

#[rstest]
#[case::deserialize_with("#[serde(deserialize_with = \"parse_host\")]")]
#[case::with("#[serde(with = \"host_module\")]")]
fn a_custom_deserializer_makes_an_option_required(#[case] attr: &str) -> Result<()> {
    let input: DeriveInput =
        syn::parse_str(&format!("struct Root {{ {attr} host: Option<String> }}"))?;
    let tokens = emit_plain(&input)?;
    ensure!(
        tokens.contains("Activation::Required"),
        "Serde raises `missing_field` before an `Option` can absorb the absence: {tokens}"
    );
    Ok(())
}

#[test]
fn a_serde_default_keeps_a_custom_deserialized_option_optional() -> Result<()> {
    let input: DeriveInput = syn::parse_quote! {
        struct Root {
            #[serde(default, deserialize_with = "parse_host")]
            host: Option<String>,
        }
    };
    let tokens = emit_plain(&input)?;
    ensure!(
        tokens.contains("Activation::Optional"),
        "Serde consults the field default before the deserializer: {tokens}"
    );
    Ok(())
}

#[test]
fn a_custom_deserializer_makes_a_nested_field_an_opaque_leaf() -> Result<()> {
    let input: DeriveInput = syn::parse_quote! {
        struct Root {
            #[serde(deserialize_with = "parse_database")]
            #[ortho_config(nested, skip_cli)]
            database: Database,
        }
    };
    let tokens = emit_plain(&input)?;
    ensure!(
        tokens.contains("SchemaChildren::Leaf"),
        "the child's key model is not what the deserializer reads: {tokens}"
    );
    ensure!(
        !tokens.contains("<Databaseasortho_config::RequiredConfigSchema>"),
        "no child requirement may be composed: {tokens}"
    );
    ensure!(
        tokens.contains(r#"key:::std::string::String::from("database")"#),
        "the field's own key still names the value it supplies: {tokens}"
    );
    Ok(())
}

#[test]
fn a_custom_deserialized_flatten_contributes_no_requirement() -> Result<()> {
    let input: DeriveInput = syn::parse_quote! {
        struct Root {
            #[serde(flatten, deserialize_with = "parse_extra")]
            #[ortho_config(nested, skip_cli)]
            extra: Extra,
        }
    };
    let tokens = emit_plain(&input)?;
    ensure!(
        !tokens.contains("Extraasortho_config::RequiredConfigSchema"),
        "a deserializer that reads the whole map has no child requirements to compose: {tokens}"
    );
    ensure!(
        !tokens.contains(r#"String::from("extra")"#),
        "a flattened field has no key of its own to report: {tokens}"
    );
    Ok(())
}
