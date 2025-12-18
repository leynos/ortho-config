//! Unit tests for the procedural macro token generators.

use super::MacroComponents;
use crate::derive::build::CollectionStrategies;
use crate::derive::generate::structs::{
    generate_cli_struct, generate_defaults_struct, generate_struct,
};
use anyhow::{Context, Result, ensure};
use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use rstest::rstest;
use syn::parse_str;

fn build_components(
    default_struct_fields: Vec<TokenStream2>,
    cli_struct_fields: Vec<TokenStream2>,
) -> Result<MacroComponents> {
    Ok(MacroComponents {
        defaults_ident: parse_str("DefaultsStruct").context("defaults ident")?,
        default_struct_fields,
        cli_ident: parse_str("CliStruct").context("cli ident")?,
        cli_struct_fields,
        load_impl: quote! {},
        prefix_fn: None,
        collection_strategies: CollectionStrategies::default(),
        cli_field_info: Vec::new(),
        post_merge_hook: false,
    })
}

#[rstest]
fn generate_struct_handles_empty_fields() -> Result<()> {
    let ident = parse_str("Empty").context("parse Empty ident")?;
    let attrs = quote! { #[derive(Default)] };
    let tokens = generate_struct(&ident, &[], &attrs);
    let expected = quote! {
        #[derive(Default)]
        struct Empty {}
    };
    ensure!(
        tokens.to_string() == expected.to_string(),
        "generated tokens differ: {tokens} != {expected}"
    );
    Ok(())
}

#[rstest]
fn generate_struct_renders_fields_with_commas() -> Result<()> {
    let ident = parse_str("WithFields").context("parse WithFields ident")?;
    let fields = vec![quote! { pub value: u32 }, quote! { pub other: String }];
    let attrs = quote! { #[derive(Default)] };
    let tokens = generate_struct(&ident, &fields, &attrs);
    let expected = quote! {
        #[derive(Default)]
        struct WithFields {
            pub value: u32,
            pub other: String,
        }
    };
    ensure!(
        tokens.to_string() == expected.to_string(),
        "generated tokens differ: {tokens} != {expected}"
    );
    Ok(())
}

#[rstest]
fn generate_cli_struct_emits_expected_tokens() -> Result<()> {
    let components = build_components(
        vec![quote! { pub value: u32 }],
        vec![quote! { #[clap(long)] pub value: Option<u32> }],
    )?;
    let config_ident = parse_str("Config").context("config ident")?;
    let tokens = generate_cli_struct(&config_ident, &components).to_string();
    ensure!(tokens.contains("CliStruct"), "struct name should render");
    ensure!(
        tokens.contains("CLI parser struct generated") && tokens.contains("Config"),
        "doc comment should cite role and config name: {tokens}"
    );
    ensure!(
        tokens.contains("clap :: Parser") || tokens.contains("clap::Parser"),
        "derive for clap::Parser should be present: {tokens}"
    );
    Ok(())
}

#[rstest]
fn generate_defaults_struct_supports_empty_fields() -> Result<()> {
    let components = build_components(
        Vec::new(),
        vec![quote! { #[clap(long)] pub value: Option<u32> }],
    )?;
    let config_ident = parse_str("Config").context("config ident")?;
    let tokens = generate_defaults_struct(&config_ident, &components).to_string();
    ensure!(
        tokens.contains("DefaultsStruct"),
        "struct name should render"
    );
    ensure!(
        tokens.contains("Defaults storage struct generated") && tokens.contains("Config"),
        "doc comment should cite role and config name: {tokens}"
    );
    ensure!(
        tokens.contains("serde :: Serialize") || tokens.contains("serde::Serialize"),
        "derive for serde::Serialize should be present: {tokens}"
    );
    Ok(())
}
