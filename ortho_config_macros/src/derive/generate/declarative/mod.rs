//! Declarative merge code generation for `#[derive(OrthoConfig)]`.
//!
//! Emits merge-state structs, trait implementations, and helper constructors
//! that layer declarative configuration values while honouring collection
//! strategies such as vector appends and map replacements.

use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use std::collections::HashSet;

use crate::derive::build::CollectionStrategies;

struct CollectionTokens {
    merge_logic: Vec<TokenStream>,
    destructured: Vec<TokenStream>,
    inserts: Vec<TokenStream>,
}

struct FinishCollectionTokens<'a> {
    append_destructured: &'a [TokenStream],
    map_destructured: &'a [TokenStream],
    append_inserts: &'a [TokenStream],
    map_inserts: &'a [TokenStream],
}

fn build_collection_tokens<'a, I, F, G>(
    fields: I,
    prefix: &str,
    merge_fn: F,
    insert_fn: G,
) -> CollectionTokens
where
    I: IntoIterator<Item = (&'a syn::Ident, &'a syn::Type)>,
    F: Fn(&syn::Ident, &proc_macro2::Ident, &str) -> TokenStream,
    G: Fn(&syn::Ident, &proc_macro2::Ident, &str) -> TokenStream,
{
    let mut merge_logic = Vec::new();
    let mut destructured = Vec::new();
    let mut inserts = Vec::new();

    for (field_ident, _ty) in fields {
        let state_field_ident = format_ident!("{}{}", prefix, field_ident);
        let field_name = field_ident.to_string();
        merge_logic.push(merge_fn(field_ident, &state_field_ident, &field_name));
        destructured.push(quote! { #state_field_ident });
        inserts.push(insert_fn(field_ident, &state_field_ident, &field_name));
    }

    CollectionTokens {
        merge_logic,
        destructured,
        inserts,
    }
}

fn append_collection_tokens(strategies: &CollectionStrategies) -> CollectionTokens {
    build_collection_tokens(
        unique_append_fields(&strategies.append),
        "append_",
        |_, state_field_ident, field_name| {
            quote! {
                if let Some(value) = map.remove(#field_name) {
                    if value.is_null() {
                        self.#state_field_ident = Some(Vec::new());
                    } else {
                        let normalised = match value {
                            ortho_config::serde_json::Value::Array(_) => value,
                            other => ortho_config::serde_json::Value::Array(vec![other]),
                        };
                        let incoming: Vec<_> =
                            ortho_config::declarative::from_value(normalised)?;
                        let acc = self
                            .#state_field_ident
                            .get_or_insert_with(Default::default);
                        acc.extend(incoming);
                    }
                }
            }
        },
        |_, state_field_ident, field_name| {
            quote! {
                if let Some(values) = #state_field_ident {
                    overlay.insert(
                        #field_name.to_owned(),
                        ortho_config::serde_json::Value::Array(values),
                    );
                }
            }
        },
    )
}

fn map_collection_tokens(strategies: &CollectionStrategies) -> CollectionTokens {
    build_collection_tokens(
        strategies.map_replace.iter().map(|(ident, ty)| (ident, ty)),
        "replace_",
        |_, state_field_ident, field_name| {
            quote! {
                if let Some(value) = map.remove(#field_name) {
                    self.#state_field_ident = Some(value);
                }
            }
        },
        |_, state_field_ident, field_name| {
            quote! {
                if let Some(value) = #state_field_ident {
                    overlay.insert(#field_name.to_owned(), value);
                }
            }
        },
    )
}

/// Deduplicate append fields by identifier.
///
/// Returns a vector of unique `(ident, type)` pairs, preserving the first
/// occurrence for each identifier in the input slice.
///
/// # Examples
///
/// ```rust,ignore
/// use syn::parse_str;
///
/// let fields = vec![
///     (parse_str("items").unwrap(), parse_str("String").unwrap()),
///     (parse_str("items").unwrap(), parse_str("String").unwrap()),
///     (parse_str("tags").unwrap(), parse_str("String").unwrap()),
/// ];
///
/// let unique = unique_append_fields(&fields);
/// assert_eq!(unique.len(), 2);
/// assert_eq!(unique[0].0.to_string(), "items");
/// assert_eq!(unique[1].0.to_string(), "tags");
/// ```
pub(crate) fn unique_append_fields(
    append_fields: &[(syn::Ident, syn::Type)],
) -> Vec<(&syn::Ident, &syn::Type)> {
    let mut seen = HashSet::new();
    append_fields
        .iter()
        .filter_map(|(ident, ty)| {
            let key = ident.to_string();
            seen.insert(key).then_some((ident, ty))
        })
        .collect()
}

/// Generate the declarative merge state struct.
///
/// Emits a `#[derive(Default)]` struct containing a backing `value` field,
/// optional append buffers for each unique vector field using the append
/// strategy, and replace buffers (`Option<Value>`) for each `BTreeMap` field
/// using the replace strategy.
///
/// # Examples
///
/// ```rust,ignore
/// use proc_macro2::TokenStream;
/// use quote::quote;
/// use syn::parse_str;
///
/// let state_ident = parse_str("__SampleDeclarativeMergeState").unwrap();
/// let tokens = generate_declarative_state_struct(&state_ident, &[]);
/// let expected: TokenStream = quote! {
///     #[derive(Default)]
///     struct __SampleDeclarativeMergeState {
///         value: ortho_config::serde_json::Value,
///     }
/// };
/// assert_eq!(tokens.to_string(), expected.to_string());
/// ```
pub(crate) fn generate_declarative_state_struct(
    state_ident: &syn::Ident,
    strategies: &CollectionStrategies,
) -> TokenStream {
    let unique_fields = unique_append_fields(&strategies.append);
    let append_state_fields = unique_fields.iter().map(|(name, _ty)| {
        let state_field_ident = format_ident!("append_{}", name);
        quote! {
            #state_field_ident: Option<Vec<ortho_config::serde_json::Value>>
        }
    });
    let map_replace_fields = strategies.map_replace.iter().map(|(name, _ty)| {
        let state_field_ident = format_ident!("replace_{}", name);
        quote! {
            #state_field_ident: Option<ortho_config::serde_json::Value>
        }
    });

    quote! {
        #[derive(Default)]
        struct #state_ident {
            value: ortho_config::serde_json::Value,
            #( #append_state_fields, )*
            #( #map_replace_fields, )*
        }
    }
}

fn generate_non_object_guard(config_ident: &syn::Ident) -> TokenStream {
    quote! {
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
            ortho_config::serde_json::Value::Object(_) => "an object",
        };
        let mut message = format!(
            concat!(
                "Declarative merge for ",
                stringify!(#config_ident),
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
}

/// Generate the `DeclarativeMerge` trait implementation.
///
/// Produces merge logic that accumulates append field contributions into
/// per-field JSON buffers and finalises the state into the concrete
/// configuration type, retaining replace buffers for map strategies along the
/// way.
///
/// # Examples
///
/// ```rust,ignore
/// use proc_macro2::TokenStream;
/// use syn::parse_str;
///
/// let state_ident = parse_str("__SampleDeclarativeMergeState").unwrap();
/// let config_ident = parse_str("SampleConfig").unwrap();
/// let tokens = generate_declarative_merge_impl(&state_ident, &config_ident, &[]);
/// assert!(tokens.to_string().contains("impl ortho_config::DeclarativeMerge"));
/// ```
// The generated merge implementation mirrors the runtime branching structure of
// the declarative merger. Splitting it further would obscure the emitted code.
pub(crate) fn generate_declarative_merge_impl(
    state_ident: &syn::Ident,
    config_ident: &syn::Ident,
    strategies: &CollectionStrategies,
) -> TokenStream {
    let append_tokens = append_collection_tokens(strategies);
    let map_tokens = map_collection_tokens(strategies);
    let CollectionTokens {
        merge_logic: append_merge_logic,
        destructured: append_destructured,
        inserts: append_inserts,
    } = append_tokens;
    let CollectionTokens {
        merge_logic: map_merge_logic,
        destructured: map_destructured,
        inserts: map_inserts,
    } = map_tokens;
    let merge_layer_body = merge_layer_tokens(config_ident, &map_merge_logic, &append_merge_logic);
    let finish_body = finish_tokens(
        state_ident,
        &FinishCollectionTokens {
            append_destructured: &append_destructured,
            map_destructured: &map_destructured,
            append_inserts: &append_inserts,
            map_inserts: &map_inserts,
        },
    );

    quote! {
        impl ortho_config::DeclarativeMerge for #state_ident {
            type Output = #config_ident;

            fn merge_layer(&mut self, layer: ortho_config::MergeLayer<'_>) -> ortho_config::OrthoResult<()> {
                #merge_layer_body
            }

            fn finish(self) -> ortho_config::OrthoResult<Self::Output> {
                #finish_body
            }
        }
    }
}

fn merge_layer_tokens(
    config_ident: &syn::Ident,
    map_merge_logic: &[TokenStream],
    append_merge_logic: &[TokenStream],
) -> TokenStream {
    let non_object_guard = generate_non_object_guard(config_ident);
    quote! {
        use ortho_config::OrthoResultExt as _;

        let provenance = layer.provenance();
        let path = layer.path().map(|p| p.to_owned());
        let value = layer.into_value();
        let mut map = match value {
            ortho_config::serde_json::Value::Object(map) => map,
            other => { #non_object_guard }
        };
        #( #map_merge_logic )*
        #( #append_merge_logic )*
        if !map.is_empty() {
            ortho_config::declarative::merge_value(
                &mut self.value,
                ortho_config::serde_json::Value::Object(map),
            );
        }

        Ok(())
    }
}

fn finish_tokens(
    state_ident: &syn::Ident,
    collections: &FinishCollectionTokens<'_>,
) -> TokenStream {
    let append_destructured = collections.append_destructured;
    let map_destructured = collections.map_destructured;
    let append_inserts = collections.append_inserts;
    let map_inserts = collections.map_inserts;
    quote! {
        let #state_ident {
            mut value,
            #( #append_destructured, )*
            #( #map_destructured, )*
        } = self;
        let mut overlay = ortho_config::serde_json::Map::new();
        #( #append_inserts )*
        #( #map_inserts )*
        if !overlay.is_empty() {
            ortho_config::declarative::merge_value(
                &mut value,
                ortho_config::serde_json::Value::Object(overlay),
            );
        }
        ortho_config::declarative::from_value(value)
    }
}

/// Generate the public `merge_from_layers` constructor.
///
/// Emits an inherent implementation that constructs the declarative state,
/// folds each layer into it, and calls `finish` to produce the configuration
/// value.
///
/// # Examples
///
/// ```rust,ignore
/// use syn::parse_str;
///
/// let state_ident = parse_str("__SampleDeclarativeMergeState").unwrap();
/// let config_ident = parse_str("SampleConfig").unwrap();
/// let tokens = generate_declarative_merge_from_layers_fn(&state_ident, &config_ident);
/// assert!(tokens.to_string().contains("merge_from_layers"));
/// ```
pub(crate) fn generate_declarative_merge_from_layers_fn(
    state_ident: &syn::Ident,
    config_ident: &syn::Ident,
) -> TokenStream {
    quote! {
        impl #config_ident {
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
                let mut state = #state_ident::default();
                for layer in layers {
                    ortho_config::DeclarativeMerge::merge_layer(&mut state, layer)?;
                }
                ortho_config::DeclarativeMerge::finish(state)
            }
        }
    }
}

/// Compose the complete declarative merge implementation.
///
/// Combines the state struct, `DeclarativeMerge` trait implementation, and
/// inherent constructor into a single token stream.
///
/// # Examples
///
/// ```rust,ignore
/// use syn::parse_str;
///
/// let config_ident = parse_str("SampleConfig").unwrap();
/// let tokens = generate_declarative_impl(&config_ident, &[]);
/// assert!(tokens.to_string().contains("DeclarativeMerge"));
/// ```
pub(crate) fn generate_declarative_impl(
    config_ident: &syn::Ident,
    strategies: &CollectionStrategies,
) -> TokenStream {
    let state_ident = format_ident!("__{}DeclarativeMergeState", config_ident);
    let state_struct = generate_declarative_state_struct(&state_ident, strategies);
    let merge_impl = generate_declarative_merge_impl(&state_ident, config_ident, strategies);
    let merge_fn = generate_declarative_merge_from_layers_fn(&state_ident, config_ident);

    quote! {
        #state_struct
        #merge_impl
        #merge_fn
    }
}

#[cfg(test)]
mod tests;
