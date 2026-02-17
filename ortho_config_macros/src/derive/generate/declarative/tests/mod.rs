//! Tests for declarative merge token generators.
//!
//! Verifies that declarative merge helpers emit deduplicated state storage,
//! trait implementations, and constructors that honour collection semantics.

use anyhow::{Result, ensure};
use quote::quote;
use rstest::rstest;

use crate::derive::build::CollectionStrategies;
use crate::derive::generate::declarative::{
    generate_declarative_impl, generate_declarative_merge_from_layers_fn,
    generate_declarative_merge_impl, generate_declarative_state_struct, unique_append_fields,
};

mod helpers;
mod merge_fn;
mod merge_impl;
mod state_struct;

use helpers::{
    TokenGenerator, append_strategies, merge_impl_tokens, parse_ident, parse_type,
    state_struct_tokens,
};

fn default_krate() -> proc_macro2::TokenStream {
    quote! { ortho_config }
}

#[rstest]
fn unique_append_fields_filters_duplicates() -> Result<()> {
    let append_fields = vec![
        (parse_ident("items")?, parse_type("String")?),
        (parse_ident("items")?, parse_type("String")?),
        (parse_ident("tags")?, parse_type("String")?),
    ];

    let filtered = unique_append_fields(&append_fields);
    ensure!(filtered.len() == 2, "expected two unique append fields");
    ensure!(
        filtered.first().map(|(ident, _)| ident.to_string()) == Some("items".to_owned()),
        "expected items as first append field"
    );
    ensure!(
        filtered.get(1).map(|(ident, _)| ident.to_string()) == Some("tags".to_owned()),
        "expected tags as second append field"
    );
    Ok(())
}

#[rstest(generator => [state_struct_tokens as TokenGenerator, merge_impl_tokens as TokenGenerator])]
fn collection_generators_deduplicate_append_fields(generator: TokenGenerator) -> Result<()> {
    let duplicate_fields = vec![
        (parse_ident("items")?, parse_type("String")?),
        (parse_ident("items")?, parse_type("String")?),
    ];
    let duplicate_strategies = append_strategies(duplicate_fields);
    let duplicate_tokens = generator(&duplicate_strategies)?;

    let deduplicated_fields = vec![(parse_ident("items")?, parse_type("String")?)];
    let deduplicated_strategies = append_strategies(deduplicated_fields);
    let deduplicated_tokens = generator(&deduplicated_strategies)?;

    ensure!(
        duplicate_tokens.to_string() == deduplicated_tokens.to_string(),
        "duplicate append fields should be deduplicated"
    );
    Ok(())
}

#[rstest]
fn generate_declarative_impl_uses_custom_krate_alias() -> Result<()> {
    let krate = quote! { my_alias };
    let config_ident = parse_ident("Sample")?;
    let strategies = CollectionStrategies::default();
    let tokens = generate_declarative_impl(&config_ident, &strategies, false, &krate);
    let output = tokens.to_string();
    ensure!(
        output.contains("my_alias :: DeclarativeMerge"),
        "expected custom krate alias in DeclarativeMerge impl, got: {output}"
    );
    ensure!(
        output.contains("my_alias :: serde_json :: Value"),
        "expected custom krate alias in serde_json usage, got: {output}"
    );
    ensure!(
        !output.contains("ortho_config ::"),
        "generated code should not contain hardcoded ortho_config:: paths: {output}"
    );
    Ok(())
}

#[rstest]
fn generate_declarative_impl_composes_helpers() -> Result<()> {
    let krate = default_krate();
    let config_ident = parse_ident("Sample")?;
    let strategies = CollectionStrategies::default();
    let tokens = generate_declarative_impl(&config_ident, &strategies, false, &krate);
    let state_ident = parse_ident("__SampleDeclarativeMergeState")?;
    let state_struct =
        generate_declarative_state_struct(&state_ident, &config_ident, &strategies, &krate);
    let merge_impl =
        generate_declarative_merge_impl(&state_ident, &config_ident, &strategies, &krate);
    let merge_fn =
        generate_declarative_merge_from_layers_fn(&state_ident, &config_ident, false, &krate);
    let expected = quote! {
        #state_struct
        #merge_impl
        #merge_fn
    };
    let actual = tokens.to_string();
    let expected_rendered = expected.to_string();
    ensure!(
        actual == expected_rendered,
        "declarative impl composition mismatch\nactual:\n{actual}\nexpected:\n{expected_rendered}"
    );
    Ok(())
}
