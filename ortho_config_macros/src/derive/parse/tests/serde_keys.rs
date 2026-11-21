//! Tests for the cached configuration key and flatten validation.

use anyhow::{Result, anyhow, ensure};
use rstest::rstest;
use syn::parse_quote;

use crate::derive::parse::{parse_input, reject_incompatible_flatten};

/// Return the cached configuration key for the single field in `input`.
fn single_key(input: &syn::DeriveInput) -> Result<String> {
    let (_, _, _, attrs) = parse_input(input).map_err(|err| anyhow!(err))?;
    attrs
        .first()
        .map(|attrs| attrs.serialized_key.clone())
        .ok_or_else(|| anyhow!("expected the fixture to declare one field"))
}

/// The Rust identifier is the key when no rename applies.
#[test]
fn unrenamed_field_keeps_the_identifier() -> Result<()> {
    let input: syn::DeriveInput = parse_quote! {
        struct Demo { retry_count: u32 }
    };
    ensure!(
        single_key(&input)? == "retry_count",
        "an unrenamed field should keep its identifier"
    );
    Ok(())
}

/// Per-field, container, and directional forms all resolve to the key the
/// configuration type deserializes.
#[rstest]
#[case::field_rename(
    parse_quote! {
        struct Demo {
            #[serde(rename = "retry-count")]
            retry_count: u32,
        }
    },
    "retry-count"
)]
#[case::container_rename_all(
    parse_quote! {
        #[serde(rename_all = "kebab-case")]
        struct Demo { retry_count: u32 }
    },
    "retry-count"
)]
#[case::deserialize_side_rename(
    parse_quote! {
        struct Demo {
            #[serde(rename(deserialize = "retry-count"))]
            retry_count: u32,
        }
    },
    "retry-count"
)]
#[case::serialize_side_rename_is_not_a_source_key(
    parse_quote! {
        struct Demo {
            #[serde(rename(serialize = "retry-count"))]
            retry_count: u32,
        }
    },
    "retry_count"
)]
fn key_model_follows_deserialization(
    #[case] input: syn::DeriveInput,
    #[case] expected: &str,
) -> Result<()> {
    ensure!(
        single_key(&input)? == expected,
        "expected the configuration key {expected}"
    );
    Ok(())
}

/// A flattened field is rejected when a generated layer would need its key.
#[rstest]
#[case::cli_option(
    parse_quote! {
        struct Demo {
            #[serde(flatten)]
            extra: Child,
        }
    },
    "generated CLI"
)]
#[case::declared_default(
    parse_quote! {
        struct Demo {
            #[serde(flatten)]
            #[ortho_config(skip_cli, default = Child { host: String::new() })]
            extra: Child,
        }
    },
    "default would be layered"
)]
#[case::merge_strategy(
    parse_quote! {
        struct Demo {
            #[serde(flatten)]
            #[ortho_config(skip_cli, merge_strategy = "replace")]
            extra: std::collections::BTreeMap<String, String>,
        }
    },
    "merge strategy"
)]
#[case::vec_append(
    parse_quote! {
        struct Demo {
            #[serde(flatten)]
            #[ortho_config(skip_cli)]
            extra: Vec<String>,
        }
    },
    "Vec field"
)]
fn incompatible_flatten_is_rejected(
    #[case] input: syn::DeriveInput,
    #[case] expected: &str,
) -> Result<()> {
    let (_, fields, _, attrs) = parse_input(&input).map_err(|err| anyhow!(err))?;
    let Err(err) = reject_incompatible_flatten(&fields, &attrs) else {
        return Err(anyhow!("expected the flatten to be rejected"));
    };
    ensure!(
        err.to_string().contains(expected),
        "unexpected error: {err}"
    );
    Ok(())
}

/// A flattened field excluded from generated layers remains accepted.
#[test]
fn schema_only_flatten_is_accepted() -> Result<()> {
    let input: syn::DeriveInput = parse_quote! {
        struct Demo {
            #[serde(flatten)]
            #[ortho_config(skip_cli, nested)]
            extra: Option<Child>,
        }
    };
    let (_, fields, _, attrs) = parse_input(&input).map_err(|err| anyhow!(err))?;
    reject_incompatible_flatten(&fields, &attrs).map_err(|err| anyhow!(err))
}
