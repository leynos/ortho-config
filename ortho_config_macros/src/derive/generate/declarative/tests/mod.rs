//! Tests for declarative merge token generators.
//!
//! Verifies that declarative merge helpers emit deduplicated state storage,
//! trait implementations, and constructors that honour append semantics.

use std::str::FromStr;

use anyhow::{Result, anyhow, ensure};
use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use rstest::rstest;
use syn::parse_str;

use super::{
    generate_declarative_impl, generate_declarative_merge_from_layers_fn,
    generate_declarative_merge_impl, generate_declarative_state_struct, unique_append_fields,
};

fn parse_ident(src: &str) -> Result<syn::Ident> {
    parse_str(src).map_err(|err| anyhow!(err))
}

fn parse_type(src: &str) -> Result<syn::Type> {
    parse_str(src).map_err(|err| anyhow!(err))
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
fn expected_declarative_merge_impl_empty() -> Result<TokenStream2> {
    let fixture = include_str!("fixtures/expected_merge_impl_empty.rs.txt");
    TokenStream2::from_str(fixture).map_err(|err| anyhow!("parse merge impl fixture: {err}"))
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
        filtered
            .first()
            .map(|(ident, _)| ident.to_string())
            .as_deref()
            == Some("items"),
        "expected items as first append field"
    );
    ensure!(
        filtered
            .get(1)
            .map(|(ident, _)| ident.to_string())
            .as_deref()
            == Some("tags"),
        "expected tags as second append field"
    );
    Ok(())
}

#[rstest]
fn generate_declarative_state_struct_emits_storage() -> Result<()> {
    let state_ident = parse_ident("__SampleDeclarativeMergeState")?;
    let tokens = generate_declarative_state_struct(&state_ident, &[]);
    let expected = quote! {
        #[derive(Default)]
        struct __SampleDeclarativeMergeState {
            value: ortho_config::serde_json::Value,
        }
    };
    ensure!(
        tokens.to_string() == expected.to_string(),
        "state struct tokens mismatch"
    );
    Ok(())
}

#[rstest]
fn generate_declarative_state_struct_includes_append_fields() -> Result<()> {
    let state_ident = parse_ident("__SampleDeclarativeMergeState")?;
    let append_fields = vec![(parse_ident("items")?, parse_type("String")?)];
    let tokens = generate_declarative_state_struct(&state_ident, &append_fields);
    let norm = tokens.to_string().replace(' ', "");
    ensure!(
        norm.contains("append_items"),
        "expected append_items storage field"
    );
    Ok(())
}

fn assert_deduplicates_append_fields<F>(generator: F) -> Result<()>
where
    F: Fn(&[(syn::Ident, syn::Type)]) -> TokenStream2,
{
    let duplicate_fields = vec![
        (parse_ident("items")?, parse_type("String")?),
        (parse_ident("items")?, parse_type("String")?),
    ];
    let duplicate_tokens = generator(&duplicate_fields);

    let deduplicated_fields = vec![(parse_ident("items")?, parse_type("String")?)];
    let deduplicated_tokens = generator(&deduplicated_fields);

    ensure!(
        duplicate_tokens.to_string() == deduplicated_tokens.to_string(),
        "duplicate append fields should be deduplicated"
    );
    Ok(())
}

#[rstest]
fn generate_declarative_state_struct_deduplicates_append_fields() -> Result<()> {
    let state_ident = parse_ident("__SampleDeclarativeMergeState")?;
    assert_deduplicates_append_fields(|fields| {
        generate_declarative_state_struct(&state_ident, fields)
    })?;
    Ok(())
}

#[rstest]
fn generate_declarative_merge_impl_emits_trait_impl() -> Result<()> {
    let state_ident = parse_ident("__SampleDeclarativeMergeState")?;
    let config_ident = parse_ident("Sample")?;
    let tokens = generate_declarative_merge_impl(&state_ident, &config_ident, &[]);
    let expected = expected_declarative_merge_impl_empty()?;
    ensure!(
        tokens.to_string() == expected.to_string(),
        "declarative merge impl tokens mismatch"
    );
    Ok(())
}

#[rstest]
fn generate_declarative_merge_impl_handles_append_fields() -> Result<()> {
    let state_ident = parse_ident("__SampleDeclarativeMergeState")?;
    let config_ident = parse_ident("Sample")?;
    let append_fields = vec![(parse_ident("items")?, parse_type("String")?)];
    let tokens = generate_declarative_merge_impl(&state_ident, &config_ident, &append_fields);
    let norm = tokens.to_string().replace(" :: ", "::").replace(' ', "");
    ensure!(
        norm.contains("append_items"),
        "expected append_items merge logic"
    );
    ensure!(
        norm.contains("OrthoResultExt"),
        "expected OrthoResultExt usage"
    );
    ensure!(
        norm.contains("serde_json::Map::new"),
        "expected serde_json map instantiation"
    );
    ensure!(
        norm.contains("Value::Array"),
        "expected Value::Array handling"
    );
    ensure!(
        norm.contains("message.push_str(\"Source:\")"),
        "expected diagnostic source message"
    );
    Ok(())
}

#[rstest]
fn generate_declarative_merge_impl_emits_non_object_error_context() -> Result<()> {
    let state_ident = parse_ident("__SampleDeclarativeMergeState")?;
    let config_ident = parse_ident("Sample")?;
    let norm = generate_declarative_merge_impl(&state_ident, &config_ident, &[])
        .to_string()
        .replace(" :: ", "::")
        .replace(' ', "");
    ensure!(
        norm.contains("\"{provenance_label}layersupplied{value_kind}.\""),
        "guard must cite provenance and value kind: {norm}"
    );
    ensure!(
        norm.contains("message.push_str(\"Source:\")"),
        "guard must mention source paths: {norm}"
    );
    ensure!(
        norm.contains("Non-objectlayerswouldoverwriteaccumulatedstate"),
        "guard must warn about overwriting state: {norm}"
    );
    Ok(())
}

#[rstest]
fn generate_declarative_merge_impl_deduplicates_append_fields() -> Result<()> {
    let state_ident = parse_ident("__SampleDeclarativeMergeState")?;
    let config_ident = parse_ident("Sample")?;
    assert_deduplicates_append_fields(|fields| {
        generate_declarative_merge_impl(&state_ident, &config_ident, fields)
    })?;
    Ok(())
}

#[rstest]
fn generate_declarative_merge_from_layers_fn_emits_constructor() -> Result<()> {
    let state_ident = parse_ident("__SampleDeclarativeMergeState")?;
    let config_ident = parse_ident("Sample")?;
    let tokens = generate_declarative_merge_from_layers_fn(&state_ident, &config_ident);
    let expected = quote! {
        impl Sample {
            /// Merge the configuration struct from declarative layers.
            ///
            /// See the
            /// [declarative merging design](https://github.com/leynos/ortho-config/blob/main/docs/design.md#43-declarative-configuration-merging)
            /// for background and trade-offs.
            ///
            /// # Examples
            ///
            /// ```rust,ignore
            /// use ortho_config::{MergeComposer, OrthoConfig};
            /// use serde::{Deserialize, Serialize};
            /// use serde_json::json;
            ///
            /// #[derive(Debug, Deserialize, Serialize, OrthoConfig)]
            /// #[ortho_config(prefix = "APP")]
            /// struct AppConfig {
            ///     #[ortho_config(default = 8080)]
            ///     port: u16,
            /// }
            ///
            /// let mut composer = MergeComposer::new();
            /// composer.push_defaults(json!({"port": 8080}));
            /// composer.push_environment(json!({"port": 9090}));
            ///
            /// let config = AppConfig::merge_from_layers(composer.layers())
            ///     .expect("layers merge successfully");
            /// assert_eq!(config.port, 9090);
            /// ```
            pub fn merge_from_layers<'a, I>(layers: I) -> ortho_config::OrthoResult<Self>
            where
                I: IntoIterator<Item = ortho_config::MergeLayer<'a>>,
            {
                let mut state = __SampleDeclarativeMergeState::default();
                for layer in layers {
                    ortho_config::DeclarativeMerge::merge_layer(&mut state, layer)?;
                }
                ortho_config::DeclarativeMerge::finish(state)
            }
        }
    };
    ensure!(
        tokens.to_string() == expected.to_string(),
        "merge_from_layers constructor mismatch"
    );
    Ok(())
}

#[rstest]
fn generate_declarative_impl_composes_helpers() -> Result<()> {
    let config_ident = parse_ident("Sample")?;
    let append_fields: Vec<(syn::Ident, syn::Type)> = Vec::new();
    let tokens = generate_declarative_impl(&config_ident, &append_fields);
    let state_ident = parse_ident("__SampleDeclarativeMergeState")?;
    let state_struct = generate_declarative_state_struct(&state_ident, &append_fields);
    let merge_impl = generate_declarative_merge_impl(&state_ident, &config_ident, &append_fields);
    let merge_fn = generate_declarative_merge_from_layers_fn(&state_ident, &config_ident);
    let expected = quote! {
        #state_struct
        #merge_impl
        #merge_fn
    };
    ensure!(
        tokens.to_string() == expected.to_string(),
        "declarative impl composition mismatch"
    );
    Ok(())
}
