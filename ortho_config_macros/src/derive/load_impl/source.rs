//! Source-aware fragments for generated configuration loading.

use quote::quote;
use syn::Ident;

use super::{LoadImplArgs, LoadImplIdents, LoadImplTokens, build_compose_layers_impl};

/// Runtime names used by a generated source-aware loading method.
pub(crate) struct LoadSourceTokens<'a> {
    /// Token resolving to the lookup-only source used by discovery.
    pub discovery: &'a proc_macro2::TokenStream,
    /// Token resolving to the scanning source used by the merge layer.
    pub merge: &'a proc_macro2::TokenStream,
}

/// Build the composition body for a generated source-aware loading method.
pub(crate) fn build_source_aware_compose_layers_impl(
    args: &LoadImplArgs<'_>,
) -> proc_macro2::TokenStream {
    let discovery_source = quote! { discovery_source };
    let merge_source = quote! { merge_source };
    let source_aware_args = LoadImplArgs {
        idents: LoadImplIdents {
            cli_ident: args.idents.cli_ident,
            config_ident: args.idents.config_ident,
            defaults_ident: args.idents.defaults_ident,
        },
        tokens: LoadImplTokens {
            env_provider: args.tokens.env_provider,
            default_struct_init: args.tokens.default_struct_init,
            config_env_var: args.tokens.config_env_var,
            dotfile_name: args.tokens.dotfile_name,
            legacy_app_name: args.tokens.legacy_app_name.clone(),
            discovery: args.tokens.discovery,
            sources: Some(LoadSourceTokens {
                discovery: &discovery_source,
                merge: &merge_source,
            }),
            krate: args.tokens.krate,
        },
        has_config_path: args.has_config_path,
    };
    build_compose_layers_impl(&source_aware_args)
}

/// Build a generated load method that calls its selected composition method.
pub(crate) fn build_load_from_iter_impl(
    config_ident: &Ident,
    compose_method: &Ident,
) -> proc_macro2::TokenStream {
    quote! {
        let composition = Self::#compose_method(iter);
        composition.into_merge_result(|layers| #config_ident::merge_from_layers(layers))
    }
}

/// Build a generated load method that forwards both injected source types.
pub(crate) fn build_load_from_iter_with_sources_impl(
    config_ident: &Ident,
    krate: &proc_macro2::TokenStream,
) -> proc_macro2::TokenStream {
    quote! {
        #krate::__private::source_aware_derived_load_started();
        let composition = Self::compose_layers_from_iter_with_sources(
            iter,
            discovery_source,
            merge_source,
        );
        let result = composition.into_merge_result(|layers| #config_ident::merge_from_layers(layers));
        #krate::__private::source_aware_derived_load_finished(&result);
        result
    }
}
