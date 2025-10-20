use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use std::collections::HashSet;

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
                let mut incoming: Vec<_> =
                    ortho_config::serde_json::from_value(value).into_ortho()?;
                let acc = self.#state_field_ident.get_or_insert_default();
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

pub(crate) fn generate_declarative_merge_from_layers_fn(
    state_ident: &syn::Ident,
    config_ident: &syn::Ident,
) -> TokenStream {
    quote! {
        impl #config_ident {
            /// Merge the configuration struct from declarative layers.
            ///
            /// See the
            /// [declarative merging design](https://github.com/leynos/ortho-config/blob/main/docs/design.md#introduce-declarative-configuration-merging)
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
mod tests {
    //! Unit tests for declarative merge token generators.

    use super::{
        generate_declarative_impl, generate_declarative_merge_from_layers_fn,
        generate_declarative_merge_impl, generate_declarative_state_struct, unique_append_fields,
    };
    use quote::quote;
    use rstest::rstest;
    use syn::parse_str;

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
        let rendered = tokens.to_string();
        assert!(rendered.contains("append_items"));
    }

    #[rstest]
    fn generate_declarative_state_struct_deduplicates_append_fields() {
        let state_ident = parse_str("__SampleDeclarativeMergeState").expect("state ident");
        let append_fields = vec![
            (
                parse_str("items").expect("first field ident"),
                parse_str("String").expect("first field type"),
            ),
            (
                parse_str("items").expect("second field ident"),
                parse_str("String").expect("second field type"),
            ),
        ];
        let tokens = generate_declarative_state_struct(&state_ident, &append_fields);
        let deduped = vec![(
            parse_str("items").expect("deduped field ident"),
            parse_str("String").expect("deduped field type"),
        )];
        let expected = generate_declarative_state_struct(&state_ident, &deduped);
        assert_eq!(tokens.to_string(), expected.to_string());
    }

    #[rstest]
    fn generate_declarative_merge_impl_emits_trait_impl() {
        let state_ident = parse_str("__SampleDeclarativeMergeState").expect("state ident");
        let config_ident = parse_str("Sample").expect("config ident");
        let tokens = generate_declarative_merge_impl(&state_ident, &config_ident, &[]);
        let expected = quote! {
            impl ortho_config::DeclarativeMerge for __SampleDeclarativeMergeState {
                type Output = Sample;

                fn merge_layer(
                    &mut self,
                    layer: ortho_config::MergeLayer<'_>
                ) -> ortho_config::OrthoResult<()> {
                    use ortho_config::OrthoResultExt as _;

                    match layer.into_value() {
                        ortho_config::serde_json::Value::Object(mut map) => {
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
        };
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
        let rendered = tokens.to_string();
        assert!(rendered.contains("append_items"));
        assert!(rendered.contains("OrthoResultExt"));
        assert!(rendered.contains("serde_json :: Map"));
        assert!(rendered.contains("Value :: Array"));
    }

    #[rstest]
    fn generate_declarative_merge_impl_deduplicates_append_fields() {
        let state_ident = parse_str("__SampleDeclarativeMergeState").expect("state ident");
        let config_ident = parse_str("Sample").expect("config ident");
        let append_fields = vec![
            (
                parse_str("items").expect("first field ident"),
                parse_str("String").expect("first field type"),
            ),
            (
                parse_str("items").expect("second field ident"),
                parse_str("String").expect("second field type"),
            ),
        ];
        let tokens = generate_declarative_merge_impl(&state_ident, &config_ident, &append_fields);
        let deduped = vec![(
            parse_str("items").expect("deduped field ident"),
            parse_str("String").expect("deduped field type"),
        )];
        let expected = generate_declarative_merge_impl(&state_ident, &config_ident, &deduped);
        assert_eq!(tokens.to_string(), expected.to_string());
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
                /// [declarative merging design](https://github.com/leynos/ortho-config/blob/main/docs/design.md#introduce-declarative-configuration-merging)
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
}
