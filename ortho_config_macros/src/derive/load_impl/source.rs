//! Source-aware fragments for generated configuration loading.

use super::{LoadImplArgs, LoadImplIdents, LoadImplTokens, build_compose_layers_impl};
use quote::quote;
use syn::Ident;

/// Runtime names used by a generated source-aware loading method.
pub(crate) struct LoadSourceTokens<'a> {
    /// Token resolving to the lookup-only source used by discovery.
    pub discovery: &'a proc_macro2::TokenStream,
    /// Token resolving to the scanning source used by the merge layer.
    pub merge: &'a proc_macro2::TokenStream,
}

/// Build the compose body with both injected runtime names bound.
///
/// The two injected names are in scope for the whole method body, so the
/// discovery and merge fragments can refer to `discovery_source` and
/// `merge_source` directly.
fn build_source_aware_compose_layers(args: &LoadImplArgs<'_>) -> proc_macro2::TokenStream {
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
        profiles: args.profiles,
        profile_env_var: args.profile_env_var.clone(),
        cli_arg_ids: args.cli_arg_ids.clone(),
        cli_bool_fields: args.cli_bool_fields.clone(),
    };
    build_compose_layers_impl(&source_aware_args)
}

/// Build the composition body for a generated source-aware loading method.
pub(crate) fn build_source_aware_compose_layers_impl(
    args: &LoadImplArgs<'_>,
) -> proc_macro2::TokenStream {
    let composition = build_source_aware_compose_layers(args);
    if args.profiles {
        // The profile compose body opens with `use` items, which only a block
        // can contain, so the tuple's composition half is projected via a
        // braced block rather than a parenthesized expression.
        quote! {{ #composition }.0 }
    } else {
        composition
    }
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

/// Build the `(composition, selection)` body for the source-aware profile path.
///
/// Only opted-in structs reach this builder, matching the way the profile
/// compose body itself is gated: the body's tail expression is already the
/// tuple, so a braced block yields it unchanged. A legacy struct has no
/// selection, so no placeholder is invented for it.
pub(crate) fn build_source_aware_compose_layers_with_selection_impl(
    args: &LoadImplArgs<'_>,
) -> proc_macro2::TokenStream {
    let composition = build_source_aware_compose_layers(args);
    quote! {{ #composition }}
}

/// Build a generated load method that forwards both injected source types.
///
/// The emitted telemetry depends on the struct: an opted-in struct resolves a
/// profile selection on this boundary, while a legacy struct does not. Both
/// are injected, so only the operation label differs. Reporting the legacy
/// operation for a profile-enabled struct would misattribute the load, because
/// the process-backed twin of this entry point reports `profile_load`.
pub(crate) fn build_load_from_iter_with_sources_impl(
    config_ident: &Ident,
    krate: &proc_macro2::TokenStream,
    profiles: bool,
) -> proc_macro2::TokenStream {
    let (started, finished) = if profiles {
        (
            quote! { #krate::__private::profile_load_injected_started(); },
            quote! { #krate::__private::profile_load_injected_finished(&result); },
        )
    } else {
        (
            quote! { #krate::__private::source_aware_derived_load_started(); },
            quote! { #krate::__private::source_aware_derived_load_finished(&result); },
        )
    };

    quote! {
        #started
        let composition = Self::compose_layers_from_iter_with_sources(
            iter,
            discovery_source,
            merge_source,
        );
        let result = composition.into_merge_result(|layers| #config_ident::merge_from_layers(layers));
        #finished
        result
    }
}

/// Build config-type composition delegates, including the injected-source API.
pub(crate) fn build_config_impl_delegates(
    krate: &proc_macro2::TokenStream,
    cli_ident: &Ident,
    config_ident: &Ident,
) -> proc_macro2::TokenStream {
    quote! {
        impl #config_ident {
            /// Compose merge layers using the current process arguments.
            pub fn compose_layers() -> #krate::declarative::LayerComposition {
                #cli_ident::compose_layers()
            }

            /// Compose merge layers from an iterator of command-line arguments.
            pub fn compose_layers_from_iter<I, T>(iter: I) -> #krate::declarative::LayerComposition
            where
                I: IntoIterator<Item = T>,
                T: Into<std::ffi::OsString> + Clone,
            {
                #cli_ident::compose_layers_from_iter(iter)
            }

            /// Compose layers from arguments and explicit environment sources.
            ///
            /// For example, arguments `["app"]`, a lookup source containing
            /// `APP_CONFIG_PATH=/srv/app.toml`, and a scan source containing
            /// `APP_PORT=9000` produce a composition with that selected file
            /// and injected environment layer. The sources stay separate so
            /// discovery cannot enumerate the merge environment.
            pub fn compose_layers_from_iter_with_sources<I, T>(
                iter: I,
                discovery_source: #krate::SharedEnvSource,
                merge_source: #krate::SharedScanEnvSource,
            ) -> #krate::declarative::LayerComposition
            where
                I: IntoIterator<Item = T>,
                T: Into<std::ffi::OsString> + Clone,
            {
                #cli_ident::compose_layers_from_iter_with_sources(
                    iter,
                    discovery_source,
                    merge_source,
                )
            }
        }
    }
}
