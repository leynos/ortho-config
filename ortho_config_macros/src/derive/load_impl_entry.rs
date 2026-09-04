//! Entry-point assembly for the generated load methods.
//!
//! Builds `load_from_iter`, the config-struct delegates, and the profile-aware
//! entry points on the CLI and config structs.

use quote::quote;
use syn::Ident;

use super::load_impl_profiles::build_config_profile_delegates;
use super::source::{
    build_load_from_iter_with_sources_impl, build_source_aware_compose_layers_impl,
};
use super::{LoadImplArgs, LoadImplIdents, build_compose_layers_impl};

fn build_load_from_iter_impl(config_ident: &Ident) -> proc_macro2::TokenStream {
    quote! {
        let composition = Self::compose_layers_from_iter(iter);
        composition.into_merge_result(|layers| #config_ident::merge_from_layers(layers))
    }
}

fn build_config_impl_delegates(
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
        }
    }
}

fn build_source_aware_methods(args: &LoadImplArgs<'_>) -> proc_macro2::TokenStream {
    let LoadImplIdents { config_ident, .. } = &args.idents;
    let krate = args.tokens.krate;
    let compose_layers_impl = build_source_aware_compose_layers_impl(args);
    let load_from_iter_impl = build_load_from_iter_with_sources_impl(config_ident, krate);

    quote! {
        /// Compose layers from arguments and explicit discovery and merge sources.
        ///
        /// Generated code keeps the two source capabilities separate so a
        /// lookup-only discovery source cannot accidentally enumerate the
        /// environment layer.
        #[expect(dead_code, reason = "Generated method may not be used in all builds")]
        pub fn compose_layers_from_iter_with_sources<I, T>(
            iter: I,
            discovery_source: #krate::SharedEnvSource,
            merge_source: #krate::SharedScanEnvSource,
        ) -> #krate::declarative::LayerComposition
        where
            I: IntoIterator<Item = T>,
            T: Into<std::ffi::OsString> + Clone,
        {
            #compose_layers_impl
        }

        /// Load configuration from arguments and explicit environment sources.
        ///
        /// The generated implementation records only bounded merge telemetry:
        /// it never serialises source values, keys, paths, or raw errors.
        pub fn load_from_iter_with_sources<I, T>(
            iter: I,
            discovery_source: #krate::SharedEnvSource,
            merge_source: #krate::SharedScanEnvSource,
        ) -> #krate::OrthoResult<#config_ident>
        where
            I: IntoIterator<Item = T>,
            T: Into<std::ffi::OsString> + Clone,
        {
            #load_from_iter_impl
        }
    }
}

fn build_profile_cli_impl(
    args: &LoadImplArgs<'_>,
    compose_layers_impl: &proc_macro2::TokenStream,
    load_from_iter_impl: &proc_macro2::TokenStream,
) -> proc_macro2::TokenStream {
    let idents = &args.idents;
    let krate = args.tokens.krate;
    let LoadImplIdents {
        cli_ident,
        config_ident,
        ..
    } = idents;
    let source_aware_methods = build_source_aware_methods(args);
    quote! {
        impl #cli_ident {
            /// Compose layers and the resolved selection in one pass.
            fn compose_layers_with_selection_from_iter<I, T>(iter: I) -> (
                #krate::declarative::LayerComposition,
                Vec<#krate::SelectedProfile>,
            )
            where
                I: IntoIterator<Item = T>,
                T: Into<std::ffi::OsString> + Clone,
            {
                #compose_layers_impl
            }

            #[expect(dead_code, reason = "Generated method may not be used in all builds")]
            pub fn compose_layers_from_iter<I, T>(iter: I) -> #krate::declarative::LayerComposition
            where
                I: IntoIterator<Item = T>,
                T: Into<std::ffi::OsString> + Clone,
            {
                Self::compose_layers_with_selection_from_iter(iter).0
            }

            #source_aware_methods

            #[expect(dead_code, reason = "Generated method may not be used in all builds")]
            pub fn compose_layers() -> #krate::declarative::LayerComposition {
                Self::compose_layers_from_iter(std::env::args_os())
            }

            pub fn load_from_iter<I, T>(iter: I) -> #krate::OrthoResult<#config_ident>
            where
                I: IntoIterator<Item = T>,
                T: Into<std::ffi::OsString> + Clone,
            {
                #load_from_iter_impl
            }

            /// Load configuration and report the selected profile.
            pub fn load_with_profile_from_iter<I, T>(iter: I) -> #krate::OrthoResult<#krate::profile::ProfileLoadOutcome<#config_ident>>
            where
                I: IntoIterator<Item = T>,
                T: Into<std::ffi::OsString> + Clone,
            {
                let (composition, selection) =
                    Self::compose_layers_with_selection_from_iter(iter);
                composition
                    .into_merge_result(|layers| #config_ident::merge_from_layers(layers))
                    .map(|config| #krate::profile::ProfileLoadOutcome::new(config, selection))
            }

            /// Load configuration using the current process arguments and
            /// report the selected profile.
            pub fn load_with_profile() -> #krate::OrthoResult<#krate::profile::ProfileLoadOutcome<#config_ident>> {
                Self::load_with_profile_from_iter(std::env::args_os())
            }
        }
    }
}

fn build_legacy_cli_impl(
    args: &LoadImplArgs<'_>,
    compose_layers_impl: &proc_macro2::TokenStream,
    load_from_iter_impl: &proc_macro2::TokenStream,
) -> proc_macro2::TokenStream {
    let idents = &args.idents;
    let krate = args.tokens.krate;
    let LoadImplIdents {
        cli_ident,
        config_ident,
        ..
    } = idents;
    let source_aware_methods = build_source_aware_methods(args);
    quote! {
        impl #cli_ident {
            #[expect(dead_code, reason = "Generated method may not be used in all builds")]
            pub fn compose_layers_from_iter<I, T>(iter: I) -> #krate::declarative::LayerComposition
            where
                I: IntoIterator<Item = T>,
                T: Into<std::ffi::OsString> + Clone,
            {
                #compose_layers_impl
            }

            #source_aware_methods

            #[expect(dead_code, reason = "Generated method may not be used in all builds")]
            pub fn compose_layers() -> #krate::declarative::LayerComposition {
                Self::compose_layers_from_iter(std::env::args_os())
            }

            pub fn load_from_iter<I, T>(iter: I) -> #krate::OrthoResult<#config_ident>
            where
                I: IntoIterator<Item = T>,
                T: Into<std::ffi::OsString> + Clone,
            {
                #load_from_iter_impl
            }
        }
    }
}
pub(crate) fn build_load_impl(args: &LoadImplArgs<'_>) -> proc_macro2::TokenStream {
    let LoadImplIdents {
        cli_ident,
        config_ident,
        ..
    } = &args.idents;
    let krate = args.tokens.krate;
    let compose_layers_impl = build_compose_layers_impl(args);
    let load_from_iter_impl = build_load_from_iter_impl(config_ident);
    let cli_impl = if args.profiles {
        build_profile_cli_impl(args, &compose_layers_impl, &load_from_iter_impl)
    } else {
        build_legacy_cli_impl(args, &compose_layers_impl, &load_from_iter_impl)
    };
    let config_impl = build_config_impl_delegates(krate, cli_ident, config_ident);
    let config_profile_impl = args
        .profiles
        .then(|| build_config_profile_delegates(krate, cli_ident, config_ident));

    quote! {
        #cli_impl
        #config_impl
        #config_profile_impl
    }
}
