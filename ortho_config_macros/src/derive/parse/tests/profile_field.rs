//! Tests for `profile_field` parsing and its span-local diagnostics.

use super::super::*;
use anyhow::{Result, anyhow, ensure};
use proc_macro2::TokenStream;
use quote::quote;
use rstest::rstest;
use syn::DeriveInput;

/// Parses a struct declaration and returns its field attributes.
fn parse_fields(tokens: TokenStream) -> Result<Vec<FieldAttrs>> {
    let input: DeriveInput = syn::parse2(tokens).map_err(|err| anyhow!(err))?;
    let (_, _, _, attrs) = parse_input(&input).map_err(|err| anyhow!(err))?;
    Ok(attrs)
}

/// Returns the parse error message for a declaration expected to fail.
fn parse_error(tokens: TokenStream) -> Result<String> {
    match parse_fields(tokens) {
        Ok(_) => Err(anyhow!("expected a parse error")),
        Err(err) => Ok(err.to_string()),
    }
}

#[rstest]
#[case("secret", ProfileFieldLabel::Secret)]
#[case("reference_only", ProfileFieldLabel::ReferenceOnly)]
fn parses_known_labels(#[case] label: &str, #[case] expected: ProfileFieldLabel) -> Result<()> {
    let literal = syn::LitStr::new(label, proc_macro2::Span::call_site());
    let attrs = parse_fields(quote! {
        struct Demo {
            #[ortho_config(profile_field = #literal)]
            token: String,
        }
    })?;

    let annotation = attrs
        .first()
        .and_then(|field| field.profile_field)
        .ok_or_else(|| anyhow!("missing profile_field annotation"))?;

    ensure!(annotation.label == expected, "wrong label for {label}");
    Ok(())
}

#[test]
fn absent_annotation_leaves_the_field_ordinary() -> Result<()> {
    let attrs = parse_fields(quote! {
        struct Demo {
            #[ortho_config(cli_long = "token")]
            token: String,
        }
    })?;

    ensure!(
        attrs
            .first()
            .is_some_and(|field| field.profile_field.is_none()),
        "ordinary fields must not carry a classification"
    );
    Ok(())
}

#[test]
fn unknown_label_is_rejected() -> Result<()> {
    let message = parse_error(quote! {
        struct Demo {
            #[ortho_config(profile_field = "public")]
            token: String,
        }
    })?;

    ensure!(
        message.contains("unknown profile_field label"),
        "unexpected diagnostic: {message}"
    );
    Ok(())
}

#[test]
fn duplicate_annotation_is_rejected() -> Result<()> {
    let message = parse_error(quote! {
        struct Demo {
            #[ortho_config(profile_field = "secret", profile_field = "reference_only")]
            token: String,
        }
    })?;

    ensure!(
        message.contains("duplicate profile_field"),
        "unexpected diagnostic: {message}"
    );
    Ok(())
}

#[test]
fn clap_flattened_container_is_rejected() -> Result<()> {
    let message = parse_error(quote! {
        struct Demo {
            #[command(flatten)]
            #[ortho_config(profile_field = "secret")]
            common: CommonArgs,
        }
    })?;

    ensure!(
        message.contains("flattened container"),
        "unexpected diagnostic: {message}"
    );
    Ok(())
}

#[test]
fn serde_flattened_container_is_rejected() -> Result<()> {
    let message = parse_error(quote! {
        struct Demo {
            #[serde(flatten)]
            #[ortho_config(profile_field = "reference_only")]
            common: CommonArgs,
        }
    })?;

    ensure!(
        message.contains("flattened container"),
        "unexpected diagnostic: {message}"
    );
    Ok(())
}
