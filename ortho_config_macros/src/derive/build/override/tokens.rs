//! Token generation helpers for collection merge logic.

use proc_macro2::TokenStream;
use quote::quote;
use syn::{Ident, Type};

use super::CollectionStrategies;

pub(super) fn build_append_blocks(strategies: &CollectionStrategies) -> Vec<TokenStream> {
    strategies
        .append
        .iter()
        .map(|(name, ty)| {
            quote! {
                {
                    let mut vec_acc: Vec<#ty> = Vec::new();
                    // Defaults capture collections as Option<Vec<_>> so we can
                    // distinguish unset layers; fall back to an empty vector
                    // when the option is None.
                    vec_acc.extend(defaults.#name.clone().unwrap_or_default());
                    if let Some(f) = &file_fig {
                        if let Ok(v) = f.extract_inner::<Vec<#ty>>(stringify!(#name)) { vec_acc.extend(v); }
                    }
                    if let Ok(v) = env_figment.extract_inner::<Vec<#ty>>(stringify!(#name)) { vec_acc.extend(v); }
                    if let Ok(v) = cli_figment.extract_inner::<Vec<#ty>>(stringify!(#name)) { vec_acc.extend(v); }
                    if !vec_acc.is_empty() {
                        overrides.#name = Some(vec_acc);
                    }
                }
            }
        })
        .collect()
}

pub(super) fn build_map_state_tokens(strategies: &CollectionStrategies) -> Option<TokenStream> {
    if strategies.map_replace.is_empty() {
        return None;
    }

    let fields = strategies.map_replace.iter().map(|(name, ty)| {
        quote! { #name: Option<#ty>, }
    });
    let init = strategies.map_replace.iter().map(|(name, _)| {
        quote! { #name: None }
    });
    Some(quote! {
        struct ReplaceState { #( #fields )* }
        let mut replace = ReplaceState { #( #init, )* };
    })
}

pub(super) struct ExtractContext<'a> {
    pub name: &'a Ident,
    pub ty: &'a Type,
    pub state_field: TokenStream,
}

pub(super) fn build_inner_extract(
    figment_expr: &TokenStream,
    context: &ExtractContext<'_>,
) -> TokenStream {
    let name = context.name;
    let ty = context.ty;
    let state_field = &context.state_field;
    quote! {
        if let Ok(v) = #figment_expr.extract_inner::<#ty>(stringify!(#name)) {
            #state_field = Some(v);
        }
    }
}

pub(super) fn build_figment_extract(
    figment_binding: &TokenStream,
    context: &ExtractContext<'_>,
    is_optional: bool,
) -> TokenStream {
    if is_optional {
        let inner = build_inner_extract(&quote! { f }, context);
        quote! {
            if let Some(f) = #figment_binding {
                #inner
            }
        }
    } else {
        build_inner_extract(figment_binding, context)
    }
}

pub(super) fn build_map_merge_blocks(strategies: &CollectionStrategies) -> Vec<TokenStream> {
    strategies
        .map_replace
        .iter()
        .map(|(name, ty)| {
            let state_field = quote! { replace.#name };
            let context = ExtractContext {
                name,
                ty,
                state_field,
            };
            let file_binding = quote! { &file_fig };
            let env_binding = quote! { env_figment };
            let cli_binding = quote! { cli_figment };
            let file_extract = build_figment_extract(&file_binding, &context, true);
            let env_extract = build_figment_extract(&env_binding, &context, false);
            let cli_extract = build_figment_extract(&cli_binding, &context, false);
            quote! {
                #file_extract
                #env_extract
                #cli_extract
            }
        })
        .collect()
}

pub(super) fn build_map_assignment_blocks(strategies: &CollectionStrategies) -> Vec<TokenStream> {
    strategies
        .map_replace
        .iter()
        .map(|(name, _ty)| {
            quote! {
                if let Some(value) = replace.#name.take() {
                    cfg.#name = value;
                }
            }
        })
        .collect()
}

pub(super) fn build_pre_merge_tokens(
    strategies: &CollectionStrategies,
    cli_binding: &TokenStream,
) -> TokenStream {
    let append_logic = build_append_blocks(strategies);
    let map_state = build_map_state_tokens(strategies);
    let map_logic = build_map_merge_blocks(strategies);
    let map_state_ts = map_state.unwrap_or_default();
    quote! {
        #[allow(unused_mut, reason = "mutation depends on generated append/merge blocks")]
        let mut cli_figment = ortho_config::figment::Figment::new();
        if let Some(cli) = #cli_binding {
            cli_figment = ortho_config::figment::Figment::from(
                ortho_config::figment::providers::Serialized::defaults(cli),
            );
        }
        #map_state_ts
        #( #append_logic )*
        #( #map_logic )*
    }
}

pub(super) fn build_post_extract_tokens(strategies: &CollectionStrategies) -> TokenStream {
    let map_assignments = build_map_assignment_blocks(strategies);
    quote! { #( #map_assignments )* }
}
