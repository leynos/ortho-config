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

fn test_generated_struct<F>(
    default_fields: Vec<TokenStream2>,
    cli_fields: Vec<TokenStream2>,
    generator: F,
) -> Result<String>
where
    F: FnOnce(&syn::Ident, &MacroComponents) -> TokenStream2,
{
    let components = build_components(default_fields, cli_fields)?;
    let config_ident = parse_str("Config").context("config ident")?;
    Ok(generator(&config_ident, &components).to_string())
}

#[rstest]
fn generate_cli_struct_emits_expected_tokens() -> Result<()> {
    let tokens = test_generated_struct(
        vec![quote! { pub value: u32 }],
        vec![quote! { #[clap(long)] pub value: Option<u32> }],
        generate_cli_struct,
    )?;
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
    let tokens = test_generated_struct(
        Vec::new(),
        vec![quote! { #[clap(long)] pub value: Option<u32> }],
        generate_defaults_struct,
    )?;
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

fn build_components_with_hook(post_merge_hook: bool) -> Result<MacroComponents> {
    Ok(MacroComponents {
        defaults_ident: parse_str("DefaultsStruct").context("defaults ident")?,
        default_struct_fields: vec![quote! { pub value: u32 }],
        cli_ident: parse_str("CliStruct").context("cli ident")?,
        cli_struct_fields: vec![quote! { #[clap(long)] pub value: Option<u32> }],
        load_impl: quote! {},
        prefix_fn: Some(quote! { "TEST_" }),
        collection_strategies: CollectionStrategies::default(),
        cli_field_info: Vec::new(),
        post_merge_hook,
    })
}

#[rstest]
fn macro_components_propagates_post_merge_hook_true() -> Result<()> {
    let components = build_components_with_hook(true)?;
    ensure!(
        components.post_merge_hook,
        "post_merge_hook should be true when set"
    );
    Ok(())
}

#[rstest]
fn macro_components_propagates_post_merge_hook_false() -> Result<()> {
    let components = build_components_with_hook(false)?;
    ensure!(
        !components.post_merge_hook,
        "post_merge_hook should be false when not set"
    );
    Ok(())
}

#[rstest]
fn macro_components_default_post_merge_hook_is_false() -> Result<()> {
    let components = build_components(Vec::new(), Vec::new())?;
    ensure!(
        !components.post_merge_hook,
        "post_merge_hook should default to false"
    );
    Ok(())
}
