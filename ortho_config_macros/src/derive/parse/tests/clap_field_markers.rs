//! Tests for the clap subcommand and flatten field markers.

use anyhow::{Result, anyhow, ensure};
use proc_macro2::TokenStream;
use quote::quote;
use syn::DeriveInput;

use crate::derive::parse::{clap_field_is_flattened, clap_field_is_subcommand};

type ClapFieldPredicate = fn(&syn::Field) -> syn::Result<bool>;

fn first_field(input: &DeriveInput) -> Result<&syn::Field> {
    let syn::Data::Struct(data) = &input.data else {
        return Err(anyhow!("expected struct"));
    };
    data.fields
        .iter()
        .next()
        .ok_or_else(|| anyhow!("missing first field"))
}

fn assert_clap_field_marker_cases(
    marker_name: &str,
    predicate: ClapFieldPredicate,
    cases: &[(TokenStream, bool)],
) -> Result<()> {
    for (tokens, expected) in cases {
        let input: DeriveInput = syn::parse2(tokens.clone())?;
        let actual = predicate(first_field(&input)?)?;
        ensure!(
            actual == *expected,
            "input `{tokens}`: expected {marker_name}={expected}, got {actual}"
        );
    }
    Ok(())
}

#[test]
fn clap_field_is_subcommand_cases() -> Result<()> {
    assert_clap_field_marker_cases(
        "subcommand",
        clap_field_is_subcommand,
        &[
            (
                quote! { struct Cli { #[command(subcommand)] command: Commands, } },
                true,
            ),
            (
                quote! { struct Cli { #[clap(subcommand)] command: Commands, } },
                true,
            ),
            (
                quote! { struct Cli { #[command(subcommand, long = "cmd")] command: Commands, } },
                true,
            ),
            (quote! { struct Cli { #[arg(long)] name: String, } }, false),
        ],
    )
}

#[test]
fn clap_field_is_flattened_cases() -> Result<()> {
    assert_clap_field_marker_cases(
        "flattened",
        clap_field_is_flattened,
        &[
            (
                quote! { struct Cli { #[command(flatten)] common: CommonArgs, } },
                true,
            ),
            (
                quote! { struct Cli { #[clap(flatten)] common: CommonArgs, } },
                true,
            ),
            (
                quote! { struct Cli { #[command(flatten, help = "common")] common: CommonArgs, } },
                true,
            ),
            (quote! { struct Cli { #[arg(long)] name: String, } }, false),
        ],
    )
}
