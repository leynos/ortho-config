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

/// Every rejection shares one shape: a field carries a fault and the diagnostic
/// names it. A table keeps the four cases from drifting into four copies of the
/// same assertion, and the expected fragment is the assertion's own evidence.
#[rstest]
#[case::unknown_label(
    quote! { #[ortho_config(profile_field = "public")] token: String },
    "unknown profile_field label"
)]
#[case::duplicate_annotation(
    quote! {
        #[ortho_config(profile_field = "secret", profile_field = "reference_only")]
        token: String
    },
    "duplicate profile_field"
)]
#[case::clap_flattened_container(
    quote! {
        #[command(flatten)]
        #[ortho_config(profile_field = "secret")]
        common: CommonArgs
    },
    "flattened container"
)]
#[case::serde_flattened_container(
    quote! {
        #[serde(flatten)]
        #[ortho_config(profile_field = "reference_only")]
        common: CommonArgs
    },
    "flattened container"
)]
#[case::serde_flatten_after_name_value(
    quote! {
        #[serde(rename = "common", flatten)]
        #[ortho_config(profile_field = "secret")]
        common: CommonArgs
    },
    "flattened container"
)]
#[case::serde_flatten_before_name_value(
    quote! {
        #[serde(flatten, rename = "common")]
        #[ortho_config(profile_field = "secret")]
        common: CommonArgs
    },
    "flattened container"
)]
#[case::serde_flatten_after_nested_name_value(
    quote! {
        #[serde(default = "CommonArgs::default", flatten)]
        #[ortho_config(profile_field = "secret")]
        common: CommonArgs
    },
    "flattened container"
)]
#[case::serde_flatten_after_group_value(
    quote! {
        #[serde(rename(serialize = "common"), flatten)]
        #[ortho_config(profile_field = "secret")]
        common: CommonArgs
    },
    "flattened container"
)]
fn invalid_annotations_are_rejected(
    #[case] field: TokenStream,
    #[case] expected: &str,
) -> Result<()> {
    let message = parse_error(quote! {
        struct Demo {
            #field
        }
    })?;

    ensure!(
        message.contains(expected),
        "expected {expected:?} in diagnostic: {message}"
    );
    Ok(())
}

/// A discarded name-value item must leave the rest of the Serde list readable.
///
/// Without the item boundary the value parser consumes `"common", flatten`, so
/// `flatten` never reaches the callback and the field is accepted as ordinary
/// while claiming protection it cannot provide. These cases fail if the
/// boundary moves back to a whole remaining stream.
#[rstest]
#[case::name_value_before_flatten(quote! { rename = "common", flatten })]
#[case::flatten_before_name_value(quote! { flatten, rename = "common" })]
#[case::nested_name_value_before_flatten(
    quote! { rename(serialize = "common"), flatten }
)]
#[case::string_with_comma_before_flatten(quote! { rename = "a,b", flatten })]
fn discard_unknown_leaves_following_serde_items(#[case] serde_body: TokenStream) -> Result<()> {
    let message = parse_error(quote! {
        struct Demo {
            #[serde(#serde_body)]
            #[ortho_config(profile_field = "secret")]
            common: CommonArgs,
        }
    })?;

    ensure!(
        message.contains("flattened container"),
        "a trailing `flatten` must still be seen: {message}"
    );
    Ok(())
}
