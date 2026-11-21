//! Requiredness cases for the schema generator.
//!
//! Each case names the supply layer it exercises, together with a positive
//! control inside the same fixture: a field stays required only when no layer
//! can supply it, so a rejection that also changed the control would prove
//! nothing about the layer under test.

use anyhow::{Result, ensure};
use syn::DeriveInput;

use super::{emit, emit_plain};

/// A materialized default is optional for its own loader only.
///
/// `#[ortho_config(default = 8080)]` feeds the defaults struct that
/// `#[derive(OrthoConfig)]` merges, so `port` is absent-tolerant here. It is not
/// a general supply layer, though: the schema marks it `DefaultBacked` so a
/// parent composing the type requires it. The Serde default and the `Option`
/// stay plainly optional, which is the control: only the declared default takes
/// the narrower activation.
#[test]
fn a_materialized_default_is_default_backed() -> Result<()> {
    let input: DeriveInput = syn::parse_quote! {
        struct Nested {
            #[ortho_config(default = 8080)]
            port: u16,
            #[serde(default)]
            note: String,
            optional: Option<String>,
        }
    };
    let tokens = emit(&input, &[])?;
    ensure!(
        tokens.contains(concat!(
            "key:::std::string::String::from(\"port\"),",
            "aliases:::std::vec![],activation:ortho_config::Activation::DefaultBacked"
        )),
        "a materialized default must not claim to be optional everywhere: {tokens}"
    );
    ensure!(
        tokens.matches("Activation::Optional").count() == 2,
        "the Serde default and the Option stay optional: {tokens}"
    );
    ensure!(
        !tokens.contains("Activation::Required"),
        "no field is required of this loader: {tokens}"
    );
    Ok(())
}

/// A declared default is only a supply layer when a loader materializes it.
///
/// `#[ortho_config(default = ...)]` feeds the defaults struct that
/// `#[derive(OrthoConfig)]` merges. A type deriving the schema alone has no
/// loader, so its parent never builds one for it and the declared default never
/// reaches the merged value. The Serde default and the `Option` stay optional,
/// which is the positive control: the rejection is about the missing loader,
/// not about defaults in general.
#[test]
fn a_schema_only_type_does_not_materialize_a_declared_default() -> Result<()> {
    let input: DeriveInput = syn::parse_quote! {
        struct Nested {
            #[ortho_config(default = 8080)]
            port: u16,
            #[serde(default)]
            note: String,
            optional: Option<String>,
        }
    };
    let tokens = emit_plain(&input)?;
    ensure!(
        tokens.contains(concat!(
            "key:::std::string::String::from(\"port\"),",
            "aliases:::std::vec![],activation:ortho_config::Activation::Required"
        )),
        "a declared default needs a loader to materialize it: {tokens}"
    );
    ensure!(
        tokens.matches("Activation::Optional").count() == 2,
        "the Serde default and the Option stay optional: {tokens}"
    );
    Ok(())
}

#[test]
fn container_default_makes_every_field_optional() -> Result<()> {
    let input: DeriveInput = syn::parse_quote! {
        #[serde(default)]
        struct Nested {
            host: String,
            port: u16,
        }
    };
    let tokens = emit_plain(&input)?;
    ensure!(
        tokens.matches("Activation::Optional").count() == 2,
        "expected the container default to apply to every field: {tokens}"
    );
    Ok(())
}
