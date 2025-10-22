//! Tests for declarative merge token generators.
//!
//! Verifies that declarative merge helpers emit deduplicated state storage,
//! trait implementations, and constructors that honour append semantics.

use super::{
    generate_declarative_impl, generate_declarative_merge_from_layers_fn,
    generate_declarative_merge_impl, generate_declarative_state_struct, unique_append_fields,
};
use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use rstest::rstest;
use syn::parse_str;

/// Returns the expected `DeclarativeMerge` impl for an empty `append_fields`
/// case.
///
/// # Examples
///
/// ```rust,ignore
/// let tokens = expected_declarative_merge_impl_empty();
/// assert!(tokens.to_string().contains("DeclarativeMerge"));
/// ```
fn expected_declarative_merge_impl_empty() -> TokenStream2 {
    quote! {
        impl ortho_config::DeclarativeMerge for __SampleDeclarativeMergeState {
            type Output = Sample;

            fn merge_layer(
                &mut self,
                layer: ortho_config::MergeLayer<'_>
            ) -> ortho_config::OrthoResult<()> {
                use ortho_config::OrthoResultExt as _;

                let provenance = layer.provenance();
                let path = layer.path().map(|p| p.to_owned());
                let value = layer.into_value();
                let mut map = match value {
                    ortho_config::serde_json::Value::Object(map) => map,
                    other => {
                        let provenance_label = match provenance {
                            ortho_config::MergeProvenance::Defaults => "defaults",
                            ortho_config::MergeProvenance::File => "file",
                            ortho_config::MergeProvenance::Environment => "environment",
                            ortho_config::MergeProvenance::Cli => "CLI",
                            _ => "unknown",
                        };
                        let value_kind = match other {
                            ortho_config::serde_json::Value::Null => "null",
                            ortho_config::serde_json::Value::Bool(_) => "a boolean",
                            ortho_config::serde_json::Value::Number(_) => "a number",
                            ortho_config::serde_json::Value::String(_) => "a string",
                            ortho_config::serde_json::Value::Array(_) => "an array",
                            ortho_config::serde_json::Value::Object(_) => unreachable!(
                                "objects handled by earlier match arm"
                            ),
                        };
                        let mut message = format!(
                            concat!(
                                "Declarative merge for ",
                                stringify!(Sample),
                                " expects JSON objects but the ",
                                "{provenance_label} layer supplied {value_kind}. "
                            ),
                            provenance_label = provenance_label,
                            value_kind = value_kind,
                        );
                        if let Some(path) = path {
                            message.push_str("Source: ");
                            message.push_str(path.as_str());
                            message.push_str(". ");
                        }
                        message.push_str("Non-object layers would overwrite accumulated state.");
                        return Err(std::sync::Arc::new(ortho_config::OrthoError::merge(
                            ortho_config::figment::Error::from(message),
                        )));
                    }
                };
                if !map.is_empty() {
                    ortho_config::declarative::merge_value(
                        &mut self.value,
                        ortho_config::serde_json::Value::Object(map),
                    );
                }
                Ok(())
            }

            fn finish(self) -> ortho_config::OrthoResult<Self::Output> {
                let __SampleDeclarativeMergeState { mut value, } = self;
                let mut appended = ortho_config::serde_json::Map::new();
                if !appended.is_empty() {
                    ortho_config::declarative::merge_value(
                        &mut value,
                        ortho_config::serde_json::Value::Object(appended),
                    );
                }
                ortho_config::declarative::from_value(value)
            }
        }
    }
}

#[rstest]
fn unique_append_fields_filters_duplicates() {
    let append_fields = vec![
        (
            parse_str("items").expect("first field ident"),
            parse_str("String").expect("first field type"),
        ),
        (
            parse_str("items").expect("second field ident"),
            parse_str("String").expect("second field type"),
        ),
        (
            parse_str("tags").expect("third field ident"),
            parse_str("String").expect("third field type"),
        ),
    ];

    let filtered = unique_append_fields(&append_fields);
    assert_eq!(filtered.len(), 2);
    assert_eq!(filtered[0].0.to_string(), "items");
    assert_eq!(filtered[1].0.to_string(), "tags");
}

#[rstest]
fn generate_declarative_state_struct_emits_storage() {
    let state_ident = parse_str("__SampleDeclarativeMergeState").expect("state ident");
    let tokens = generate_declarative_state_struct(&state_ident, &[]);
    let expected = quote! {
        #[derive(Default)]
        struct __SampleDeclarativeMergeState {
            value: ortho_config::serde_json::Value,
        }
    };
    assert_eq!(tokens.to_string(), expected.to_string());
}

#[rstest]
fn generate_declarative_state_struct_includes_append_fields() {
    let state_ident = parse_str("__SampleDeclarativeMergeState").expect("state ident");
    let append_fields = vec![(
        parse_str("items").expect("field ident"),
        parse_str("String").expect("field type"),
    )];
    let tokens = generate_declarative_state_struct(&state_ident, &append_fields);
    let norm = tokens.to_string().replace(' ', "");
    assert!(norm.contains("append_items"));
}

fn assert_deduplicates_append_fields<F>(generator: F)
where
    F: Fn(&[(syn::Ident, syn::Type)]) -> TokenStream2,
{
    let duplicate_fields = vec![
        (
            parse_str("items").expect("first duplicate field ident"),
            parse_str("String").expect("first duplicate field type"),
        ),
        (
            parse_str("items").expect("second duplicate field ident"),
            parse_str("String").expect("second duplicate field type"),
        ),
    ];
    let duplicate_tokens = generator(&duplicate_fields);

    let deduplicated_fields = vec![(
        parse_str("items").expect("deduplicated field ident"),
        parse_str("String").expect("deduplicated field type"),
    )];
    let deduplicated_tokens = generator(&deduplicated_fields);

    assert_eq!(
        duplicate_tokens.to_string(),
        deduplicated_tokens.to_string()
    );
}

#[rstest]
fn generate_declarative_state_struct_deduplicates_append_fields() {
    let state_ident = parse_str("__SampleDeclarativeMergeState").expect("state ident");
    assert_deduplicates_append_fields(|fields| {
        generate_declarative_state_struct(&state_ident, fields)
    });
}

#[rstest]
fn generate_declarative_merge_impl_emits_trait_impl() {
    let state_ident = parse_str("__SampleDeclarativeMergeState").expect("state ident");
    let config_ident = parse_str("Sample").expect("config ident");
    let tokens = generate_declarative_merge_impl(&state_ident, &config_ident, &[]);
    let expected = expected_declarative_merge_impl_empty();
    assert_eq!(tokens.to_string(), expected.to_string());
}

#[rstest]
fn generate_declarative_merge_impl_handles_append_fields() {
    let state_ident = parse_str("__SampleDeclarativeMergeState").expect("state ident");
    let config_ident = parse_str("Sample").expect("config ident");
    let append_fields = vec![(
        parse_str("items").expect("field ident"),
        parse_str("String").expect("field type"),
    )];
    let tokens = generate_declarative_merge_impl(&state_ident, &config_ident, &append_fields);
    let norm = tokens.to_string().replace(" :: ", "::").replace(' ', "");
    assert!(norm.contains("append_items"));
    assert!(norm.contains("OrthoResultExt"));
    assert!(norm.contains("serde_json::Map::new"));
    assert!(norm.contains("Value::Array"));
    assert!(norm.contains("message.push_str(\"Source:\")"));
}

#[rstest]
fn generate_declarative_merge_impl_emits_non_object_error_context() {
    let state_ident = parse_str("__SampleDeclarativeMergeState").expect("state ident");
    let config_ident = parse_str("Sample").expect("config ident");
    let norm = generate_declarative_merge_impl(&state_ident, &config_ident, &[])
        .to_string()
        .replace(" :: ", "::")
        .replace(' ', "");
    assert!(
        norm.contains("\"{provenance_label}layersupplied{value_kind}.\""),
        "guard must cite provenance and value kind: {norm}"
    );
    assert!(
        norm.contains("message.push_str(\"Source:\")"),
        "guard must mention source paths: {norm}"
    );
    assert!(
        norm.contains("Non-objectlayerswouldoverwriteaccumulatedstate"),
        "guard must warn about overwriting state: {norm}"
    );
}

#[rstest]
fn generate_declarative_merge_impl_deduplicates_append_fields() {
    let state_ident = parse_str("__SampleDeclarativeMergeState").expect("state ident");
    let config_ident = parse_str("Sample").expect("config ident");
    assert_deduplicates_append_fields(|fields| {
        generate_declarative_merge_impl(&state_ident, &config_ident, fields)
    });
}

#[rstest]
fn generate_declarative_merge_from_layers_fn_emits_constructor() {
    let state_ident = parse_str("__SampleDeclarativeMergeState").expect("state ident");
    let config_ident = parse_str("Sample").expect("config ident");
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
    assert_eq!(tokens.to_string(), expected.to_string());
}

#[rstest]
fn generate_declarative_impl_composes_helpers() {
    let config_ident = parse_str("Sample").expect("config ident");
    let append_fields: Vec<(syn::Ident, syn::Type)> = Vec::new();
    let tokens = generate_declarative_impl(&config_ident, &append_fields);
    let expected = {
        let state_ident = parse_str("__SampleDeclarativeMergeState").expect("state ident");
        let state_struct = generate_declarative_state_struct(&state_ident, &append_fields);
        let merge_impl =
            generate_declarative_merge_impl(&state_ident, &config_ident, &append_fields);
        let merge_fn = generate_declarative_merge_from_layers_fn(&state_ident, &config_ident);
        quote! {
            #state_struct
            #merge_impl
            #merge_fn
        }
    };
    assert_eq!(tokens.to_string(), expected.to_string());
}
