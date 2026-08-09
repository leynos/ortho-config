//! Entry-point assembly for the generated load methods.
//!
//! Builds `load_from_iter`, the config-struct delegates, and the profile-aware
//! entry points on the CLI and config structs.

use quote::quote;
use syn::Ident;

use super::load_impl_profiles::build_config_profile_delegates;
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

/// Assemble the final `load_from_iter` method using the helper snippets.
#[expect(
    clippy::too_many_lines,
    reason = "The generated impl block enumerates the public entry points; splitting would obscure the surface"
)]
pub(crate) fn build_load_impl(args: &LoadImplArgs<'_>) -> proc_macro2::TokenStream {
    let idents = &args.idents;
    let krate = args.tokens.krate;
    let LoadImplIdents {
        cli_ident,
        config_ident,
        ..
    } = idents;
    let compose_layers_impl = build_compose_layers_impl(args);
    let load_from_iter_impl = build_load_from_iter_impl(config_ident);
    let config_impl = build_config_impl_delegates(krate, cli_ident, config_ident);

    if args.profiles {
        let config_profile_impl = build_config_profile_delegates(krate, cli_ident, config_ident);
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
            #config_impl
            #config_profile_impl
        }
    } else {
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
            #config_impl
        }
    }
}
