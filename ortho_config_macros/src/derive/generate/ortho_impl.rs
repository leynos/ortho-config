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
    let has_default_as_absent = cli_field_info.iter().any(|f| f.default_as_absent);

    if !has_default_as_absent {
        // No fields with cli_default_as_absent; the blanket impl in ortho_config
        // handles this case. No specialized impl needed.
        return quote! {};
    }

    // Generate field extraction logic
    let field_extractions: Vec<TokenStream> = cli_field_info
        .iter()
        .map(|field| {
            let field_name = &field.name;
            let field_name_str = field_name.to_string();
            let arg_id = &field.arg_id;

            if field.default_as_absent {
                // Check value_source before including this field
                quote! {
                    if matches.value_source(#arg_id)
                        == Some(clap::parser::ValueSource::CommandLine)
                    {
                        if let Some(value) = base_map.remove(#field_name_str) {
                            map.insert(#field_name_str.to_owned(), value);
                        }
                    }
                }
            } else {
                // Include normally (already in sanitised base)
                quote! {
                    if let Some(value) = base_map.remove(#field_name_str) {
                        map.insert(#field_name_str.to_owned(), value);
                    }
                }
            }
        })
        .collect();

    quote! {
        impl ortho_config::CliValueExtractor for #config_ident {
            fn extract_user_provided(
                &self,
                matches: &clap::ArgMatches,
            ) -> ortho_config::OrthoResult<ortho_config::serde_json::Value> {
                use ortho_config::OrthoResultExt;
                use ortho_config::serde_json;

                // Serialise self (the parsed CLI struct) and strip None fields
                let base = serde_json::to_value(self)
                    .into_ortho()?;

                let mut base_map = match base {
                    serde_json::Value::Object(m) => m,
                    _ => serde_json::Map::new(),
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
