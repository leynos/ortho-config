//! `OrthoConfig` trait implementation generation helpers.
//!
//! These functions emit the runtime glue that wires the derive macro outputs
//! into the library traits. Keeping the generation logic separate from the
//! procedural macro entrypoint makes the flow easier to audit and test.

use proc_macro2::TokenStream;
use quote::quote;
use syn::Ident;

use crate::{CliFieldInfo, MacroComponents};

use super::structs::{generate_cli_struct, generate_defaults_struct};

/// Generate the `OrthoConfig` trait implementation.
pub(crate) fn generate_ortho_impl(
    config_ident: &Ident,
    components: &MacroComponents,
) -> TokenStream {
    let MacroComponents {
        cli_ident,
        load_impl,
        prefix_fn,
        ..
    } = components;
    let prefix_tokens = prefix_fn.clone().unwrap_or_else(|| quote! {});
    quote! {
        #load_impl

        impl ortho_config::OrthoConfig for #config_ident {
            fn load_from_iter<I, T>(iter: I) -> ortho_config::OrthoResult<Self>
            where
                I: IntoIterator<Item = T>,
                T: Into<std::ffi::OsString> + Clone,
            {
                #cli_ident::load_from_iter(iter)
            }

            #prefix_tokens
        }

        const _: () = {
            fn _assert_deser<T: serde::de::DeserializeOwned>() {}
            let _ = _assert_deser::<#config_ident>;
        };
    }
}

/// Generate the extraction logic for a single field.
///
/// Produces a `TokenStream` that extracts the field value from the base map
/// and inserts it into the output map. For fields with `cli_default_as_absent`,
/// checks `value_source()` to ensure only explicitly provided values are included.
fn generate_field_extraction(field: &CliFieldInfo) -> TokenStream {
    let field_key = &field.serialized_key;
    let arg_id = &field.arg_id;

    // Common logic for moving a value from base_map to the output map
    let move_value = quote! {
        if let Some(value) = base_map.remove(#field_key) {
            map.insert(#field_key.to_owned(), value);
        }
    };

    if field.is_default_as_absent {
        // Check value_source before including this field
        quote! {
            if matches.value_source(#arg_id)
                == Some(clap::parser::ValueSource::CommandLine)
            {
                #move_value
            }
        }
    } else {
        // Include normally (already in sanitised base)
        move_value
    }
}

/// Generate helper functions for pruning null values from `serde_json` trees.
///
/// This returns a `TokenStream` containing helper functions that remove `null`
/// values from objects and arrays, and collapses empty nested objects to `null`.
fn generate_prune_nulls_helpers() -> TokenStream {
    quote! {
        fn prune_nulls_inner(value: &mut serde_json::Value, is_root: bool) {
            match value {
                serde_json::Value::Object(map) => {
                    for value in map.values_mut() {
                        prune_nulls_inner(value, false);
                    }
                    map.retain(|_, value| !value.is_null());
                    if !is_root && map.is_empty() {
                        *value = serde_json::Value::Null;
                    }
                }
                serde_json::Value::Array(values) => {
                    for value in values.iter_mut() {
                        prune_nulls_inner(value, false);
                    }
                    values.retain(|value| !value.is_null());
                }
                _ => {}
            }
        }

        fn prune_nulls(value: &mut serde_json::Value) {
            prune_nulls_inner(value, true);
        }
    }
}

/// Generate the `CliValueExtractor` trait implementation.
///
/// This generates code that uses clap's `ArgMatches::value_source()` to
/// distinguish between values explicitly provided on the CLI and clap defaults.
/// Fields marked with `cli_default_as_absent` are only included when the user
/// explicitly provided them.
///
/// If no fields have `cli_default_as_absent`, no implementation is generated
/// because the blanket impl in `ortho_config` covers the default case.
fn generate_cli_value_extractor_impl(
    config_ident: &Ident,
    cli_field_info: &[CliFieldInfo],
) -> TokenStream {
    // Check if any field has cli_default_as_absent
    let has_default_as_absent = cli_field_info.iter().any(|f| f.is_default_as_absent);

    if !has_default_as_absent {
        // No fields with cli_default_as_absent; the blanket impl in ortho_config
        // handles this case. No specialized impl needed.
        return quote! {};
    }

    // Generate field extraction logic
    let field_extractions: Vec<TokenStream> = cli_field_info
        .iter()
        .map(generate_field_extraction)
        .collect();

    let prune_nulls_helpers = generate_prune_nulls_helpers();

    quote! {
        impl ortho_config::CliValueExtractor for #config_ident {
            fn extract_user_provided(
                &self,
                matches: &clap::ArgMatches,
            ) -> ortho_config::OrthoResult<ortho_config::serde_json::Value> {
                use ortho_config::OrthoResultExt;
                use ortho_config::serde_json;

                #prune_nulls_helpers

                // Serialise self (the parsed CLI struct) and strip null values so
                // absent options do not clobber file/environment defaults.
                let mut base = serde_json::to_value(self).into_ortho()?;
                prune_nulls(&mut base);

                let mut base_map = match base {
                    serde_json::Value::Object(m) => m,
                    other => {
                        return Err(std::sync::Arc::new(ortho_config::OrthoError::Validation {
                            key: String::from("cli"),
                            message: format!(
                                "expected parsed CLI values to serialize to an object, got {other:?}",
                            ),
                        }));
                    }
                };

                let mut map = serde_json::Map::new();

                #(#field_extractions)*

                Ok(serde_json::Value::Object(map))
            }
        }
    }
}

/// Compose the complete trait implementation output.
///
/// Combines the CLI parser struct, defaults struct, and `OrthoConfig` impl into
/// a single token stream ready for procedural macro expansion.
pub(crate) fn generate_trait_implementation(
    config_ident: &Ident,
    components: &MacroComponents,
) -> TokenStream {
    let cli_struct = generate_cli_struct(config_ident, components);
    let defaults_struct = generate_defaults_struct(config_ident, components);
    let ortho_impl = generate_ortho_impl(config_ident, components);
    let cli_extractor_impl =
        generate_cli_value_extractor_impl(config_ident, &components.cli_field_info);
    quote! {
        #cli_struct
        #defaults_struct
        #ortho_impl
        #cli_extractor_impl
    }
}
