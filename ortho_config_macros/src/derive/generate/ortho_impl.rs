//! `OrthoConfig` trait implementation generation helpers.
//!
//! These functions emit the runtime glue that wires the derive macro outputs
//! into the library traits. Keeping the generation logic separate from the
//! procedural macro entrypoint makes the flow easier to audit and test.

use proc_macro2::TokenStream;
use quote::quote;
use syn::Ident;

use crate::MacroComponents;

use super::structs::{generate_cli_struct, generate_defaults_struct};

/// Generate the `OrthoConfig` trait implementation.
pub(crate) fn generate_ortho_impl(
    config_ident: &Ident,
    components: &MacroComponents,
) -> TokenStream {
    let MacroComponents {
        cli_ident,
        override_struct_ts,
        load_impl,
        prefix_fn,
        ..
    } = components;
    let prefix_tokens = prefix_fn.clone().unwrap_or_else(|| quote! {});
    quote! {
        #override_struct_ts

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

/// Compose the complete trait implementation output.
///
/// Combines the CLI parser struct, defaults struct, and `OrthoConfig` impl into
/// a single token stream ready for procedural macro expansion.
pub(crate) fn generate_trait_implementation(
    config_ident: &Ident,
    components: &MacroComponents,
) -> TokenStream {
    let cli_struct = generate_cli_struct(components);
    let defaults_struct = generate_defaults_struct(components);
    let ortho_impl = generate_ortho_impl(config_ident, components);
    quote! {
        #cli_struct
        #defaults_struct
        #ortho_impl
    }
}
