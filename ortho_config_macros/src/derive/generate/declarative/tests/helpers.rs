//! Shared parser and token helpers for declarative generator tests.

use std::str::FromStr;

use anyhow::{Result, anyhow};
use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::parse_str;

use crate::derive::build::CollectionStrategies;
use crate::derive::generate::declarative::{
    generate_declarative_merge_impl, generate_declarative_state_struct,
};

pub(super) fn parse_ident(src: &str) -> Result<syn::Ident> {
    parse_str(src).map_err(|err| anyhow!(err))
}

pub(super) fn parse_type(src: &str) -> Result<syn::Type> {
    parse_str(src).map_err(|err| anyhow!(err))
}

pub(super) fn default_krate() -> TokenStream2 {
    quote! { ortho_config }
}

/// Returns the expected `DeclarativeMerge` impl for an empty `append_fields`
/// case.
///
/// # Examples
///
/// ```rust,ignore
/// let tokens = expected_declarative_merge_impl_empty();
/// assert!(tokens.to_string().contains("DeclarativeMerge"));
/// ```
pub(super) fn expected_declarative_merge_impl_empty() -> Result<TokenStream2> {
    let fixture = include_str!("fixtures/expected_merge_impl_empty.rs.txt");
    TokenStream2::from_str(fixture).map_err(|err| anyhow!("parse merge impl fixture: {err}"))
}

pub(super) fn append_strategies(fields: Vec<(syn::Ident, syn::Type)>) -> CollectionStrategies {
    CollectionStrategies {
        append: fields,
        map_replace: Vec::new(),
    }
}

pub(super) type TokenGenerator = fn(&CollectionStrategies) -> Result<TokenStream2>;

pub(super) fn state_struct_tokens(strategies: &CollectionStrategies) -> Result<TokenStream2> {
    let krate = default_krate();
    let state_ident = parse_ident("__SampleDeclarativeMergeState")?;
    let config_ident = parse_ident("SampleConfig")?;
    Ok(generate_declarative_state_struct(
        &state_ident,
        &config_ident,
        strategies,
        &krate,
    ))
}

pub(super) fn merge_impl_tokens(strategies: &CollectionStrategies) -> Result<TokenStream2> {
    let krate = default_krate();
    let state_ident = parse_ident("__SampleDeclarativeMergeState")?;
    let config_ident = parse_ident("Sample")?;
    Ok(generate_declarative_merge_impl(
        &state_ident,
        &config_ident,
        strategies,
        &krate,
    ))
}
