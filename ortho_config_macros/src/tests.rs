//! Unit tests for the procedural macro token generators.

use super::{MacroComponentArgs, MacroComponents, build_macro_components};
use crate::derive::build::CollectionStrategies;
use crate::derive::generate::structs::{
    generate_cli_struct, generate_defaults_struct, generate_struct,
};
use crate::derive::parse::parse_input;
use anyhow::{Context, Result, anyhow, ensure};
use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use rstest::rstest;
use syn::{DeriveInput, parse_quote, parse_str};

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

/// Test case for generated struct validation.
struct GeneratedStructCase {
    default_fields: Vec<TokenStream2>,
    cli_fields: Vec<TokenStream2>,
    generator: fn(&syn::Ident, &MacroComponents) -> TokenStream2,
    struct_name: &'static str,
    doc_fragment: &'static str,
    derive_variants: &'static [&'static str],
}

#[rstest]
#[case::cli_struct(GeneratedStructCase {
    default_fields: vec![quote! { pub value: u32 }],
    cli_fields: vec![quote! { #[clap(long)] pub value: Option<u32> }],
    generator: generate_cli_struct,
    struct_name: "CliStruct",
    doc_fragment: "CLI parser struct generated",
    derive_variants: &["clap :: Parser", "clap::Parser"],
})]
#[case::defaults_struct(GeneratedStructCase {
    default_fields: Vec::new(),
    cli_fields: vec![quote! { #[clap(long)] pub value: Option<u32> }],
    generator: generate_defaults_struct,
    struct_name: "DefaultsStruct",
    doc_fragment: "Defaults storage struct generated",
    derive_variants: &["serde :: Serialize", "serde::Serialize"],
})]
fn generated_struct_emits_expected_tokens(#[case] case: GeneratedStructCase) -> Result<()> {
    let tokens = test_generated_struct(case.default_fields, case.cli_fields, case.generator)?;
    ensure!(
        tokens.contains(case.struct_name),
        "struct name should render"
    );
    ensure!(
        tokens.contains(case.doc_fragment) && tokens.contains("Config"),
        "doc comment should cite role and config name: {tokens}"
    );
    ensure!(
        case.derive_variants
            .iter()
            .any(|variant| tokens.contains(variant)),
        "expected derive should be present: {tokens}"
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
#[case::explicit_true(Some(true), true, "post_merge_hook should be true when set")]
#[case::explicit_false(Some(false), false, "post_merge_hook should be false when not set")]
#[case::default_false(None, false, "post_merge_hook should default to false")]
fn macro_components_propagates_post_merge_hook(
    #[case] hook_input: Option<bool>,
    #[case] expected: bool,
    #[case] error_msg: &str,
) -> Result<()> {
    let components = hook_input.map_or_else(
        || build_components(Vec::new(), Vec::new()),
        build_components_with_hook,
    )?;
    ensure!(components.post_merge_hook == expected, "{error_msg}");
    Ok(())
}

/// Build `MacroComponents` from a parsed `DeriveInput` using the full parsing pipeline.
fn build_components_from_input(input: &DeriveInput) -> Result<MacroComponents> {
    let (ident, fields, struct_attrs, field_attrs) =
        parse_input(input).map_err(|err| anyhow!(err))?;
    let args = MacroComponentArgs {
        ident: &ident,
        fields: &fields,
        struct_attrs: &struct_attrs,
        field_attrs: &field_attrs,
        serde_rename_all: None,
    };
    build_macro_components(&args).map_err(|err| anyhow!(err))
}

#[rstest]
#[case::short_form(
    parse_quote! {
        #[ortho_config(prefix = "TEST_", post_merge_hook)]
        struct Config { value: String }
    },
    true,
    "post_merge_hook should be true when parsed from #[ortho_config(post_merge_hook)]"
)]
#[case::explicit_true(
    parse_quote! {
        #[ortho_config(prefix = "TEST_", post_merge_hook = true)]
        struct Config { value: String }
    },
    true,
    "post_merge_hook should be true when parsed from #[ortho_config(post_merge_hook = true)]"
)]
#[case::explicit_false(
    parse_quote! {
        #[ortho_config(prefix = "TEST_", post_merge_hook = false)]
        struct Config { value: String }
    },
    false,
    "post_merge_hook should be false when parsed from #[ortho_config(post_merge_hook = false)]"
)]
#[case::default_false(
    parse_quote! {
        #[ortho_config(prefix = "TEST_")]
        struct Config { value: String }
    },
    false,
    "post_merge_hook should default to false when not specified in attributes"
)]
fn parsing_pipeline_propagates_post_merge_hook(
    #[case] input: DeriveInput,
    #[case] expected: bool,
    #[case] error_msg: &str,
) -> Result<()> {
    let components = build_components_from_input(&input)?;
    ensure!(components.post_merge_hook == expected, "{error_msg}");
    Ok(())
}
