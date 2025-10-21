//! Declarative merge code generation for `#[derive(OrthoConfig)]`.
//!
//! Emits merge-state structs, trait implementations, and helper constructors
//! that layer declarative configuration values while honouring append
//! semantics for vector fields.

use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use std::collections::HashSet;

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
/// Emits a `#[derive(Default)]` struct containing a backing `value` field and
/// optional append buffers for each unique vector field configured with the
/// append strategy.
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
    append_fields: &[(syn::Ident, syn::Type)],
) -> TokenStream {
    let unique_fields = unique_append_fields(append_fields);
    let append_state_fields = unique_fields.iter().map(|(name, _ty)| {
        let state_field_ident = format_ident!("append_{}", name);
        quote! {
            #state_field_ident: Option<Vec<ortho_config::serde_json::Value>>
        }
    });

    quote! {
        #[derive(Default)]
        struct #state_ident {
            value: ortho_config::serde_json::Value,
            #( #append_state_fields ),*
        }
    }
}

/// Generate the `DeclarativeMerge` trait implementation.
///
/// Produces merge logic that accumulates append field contributions into
/// per-field JSON buffers and finalises the state into the concrete
/// configuration type.
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
pub(crate) fn generate_declarative_merge_impl(
    state_ident: &syn::Ident,
    config_ident: &syn::Ident,
    append_fields: &[(syn::Ident, syn::Type)],
) -> TokenStream {
    let unique_fields = unique_append_fields(append_fields);
    let append_logic = unique_fields.iter().map(|(field_ident, _ty)| {
        let state_field_ident = format_ident!("append_{}", field_ident);
        let field_name = field_ident.to_string();
        quote! {
            if let Some(value) = map.remove(#field_name) {
                let incoming: Vec<_> =
                    ortho_config::serde_json::from_value(value).into_ortho()?;
                let acc = self
                    .#state_field_ident
                    .get_or_insert_with(Default::default);
                acc.extend(incoming);
                let arr = ortho_config::serde_json::Value::Array(acc.clone());
                let mut object = ortho_config::serde_json::Map::new();
                object.insert(String::from(#field_name), arr);
                ortho_config::declarative::merge_value(
                    &mut self.value,
                    ortho_config::serde_json::Value::Object(object),
                );
            }
        }
    });

    quote! {
        impl ortho_config::DeclarativeMerge for #state_ident {
            type Output = #config_ident;

            fn merge_layer(&mut self, layer: ortho_config::MergeLayer<'_>) -> ortho_config::OrthoResult<()> {
                use ortho_config::OrthoResultExt as _;

                match layer.into_value() {
                    ortho_config::serde_json::Value::Object(mut map) => {
                        #( #append_logic )*
                        if !map.is_empty() {
                            ortho_config::declarative::merge_value(
                                &mut self.value,
                                ortho_config::serde_json::Value::Object(map),
                            );
                        }
                    }
                    other => {
                        ortho_config::declarative::merge_value(&mut self.value, other);
                    }
                }

                Ok(())
            }

            fn finish(self) -> ortho_config::OrthoResult<Self::Output> {
                ortho_config::declarative::from_value(self.value)
            }
        }
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
            /// ```rust
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
    append_fields: &[(syn::Ident, syn::Type)],
) -> TokenStream {
    let state_ident = format_ident!("__{}DeclarativeMergeState", config_ident);
    let state_struct = generate_declarative_state_struct(&state_ident, append_fields);
    let merge_impl = generate_declarative_merge_impl(&state_ident, config_ident, append_fields);
    let merge_fn = generate_declarative_merge_from_layers_fn(&state_ident, config_ident);

    quote! {
        #state_struct
        #merge_impl
        #merge_fn
    }
}

#[cfg(test)]
mod tests;
